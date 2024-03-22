//Copyright 2024 Felix Engl
//
//Licensed under the Apache License, Version 2.0 (the "License");
//you may not use this file except in compliance with the License.
//You may obtain a copy of the License at
//
//    http://www.apache.org/licenses/LICENSE-2.0
//
//Unless required by applicable law or agreed to in writing, software
//distributed under the License is distributed on an "AS IS" BASIS,
//WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//See the License for the specific language governing permissions and
//limitations under the License.

use std::borrow::Cow;
use std::fmt::{Debug, Display, Formatter};
use std::fs::{File};
use std::io;
use camino::{Utf8PathBuf as PathBuf, Utf8Path as Path, Utf8PathBuf, Utf8Path};
use std::sync::atomic::{AtomicU16, AtomicU64, Ordering};
use std::time::SystemTime;
use data_encoding::{BASE64URL_NOPAD};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;
use tokio::sync::Mutex;
use crate::core::config::configs::Configs;
use crate::core::io::{AtraPathBuf};
use crate::core::io::paths::{DataFilePathBuf};
use crate::warc::reader::WarcCursor;


/// The errors when working with the FileSystemAccess
#[derive(Debug, Error)]
pub enum FSAError {
    #[error("IOError at {0:?}\n{1}")]
    IOError(String, #[source] io::Error),
    // #[error("The path {0:?} already exists but it was expected to be fresh!")]
    // AlreadyExits(String),
}

/// Helper trait to convert Result-enums to Result-enums with FSAError
pub trait ToFSAError<T> {

    fn to_fsa_error<F: FnOnce() -> String>(self, path_provider: F) -> Result<T, FSAError>;
}

impl<T> ToFSAError<T> for Result<T, io::Error> {
    #[inline]
    fn to_fsa_error<F: FnOnce() -> String>(self, path_provider: F) -> Result<T, FSAError> {
        self.map_err(|err| FSAError::IOError(path_provider(), err))
    }
}


/// Provides the paths in the application
#[derive(Debug)]
pub struct FileSystemAccess {
    service: String,
    collection: String,
    crawl_job_id: u64,
    data_serial: AtomicU64,
    output_folder: PathBuf,
    big_file_folder: PathBuf,
    filesystem_lock: Mutex<()>,
}

impl FileSystemAccess {

    pub fn new(
        service: String,
        collection: String,
        crawl_job_id: u64,
        output_folder: PathBuf,
        big_file_folder: PathBuf,
    ) -> Self {
        let output_folder =  PathBuf::from(output_folder);
        let ouput_path = Path::new(&output_folder);
        if !ouput_path.exists() {
            std::fs::create_dir_all(ouput_path).expect("Can not create necessary directories.");
        }

        if !big_file_folder.exists() {
            std::fs::create_dir_all(big_file_folder.clone()).expect("Can not create necessary directories.");
        }

        Self {
            service,
            collection,
            crawl_job_id,
            output_folder,
            big_file_folder,
            data_serial: AtomicU64::default(),
            filesystem_lock: Mutex::new(()),
        }
    }


    /// Creates a unique path to a fresh data file.
    pub fn create_unique_path_for_dat_file(&self, url: &str) -> DataFilePathBuf {
        let part1 = BASE64URL_NOPAD.encode(url.as_bytes());
        let part2 = BASE64URL_NOPAD.encode(&OffsetDateTime::now_utc().unix_timestamp_nanos().to_be_bytes());
        let part3 = BASE64URL_NOPAD.encode(&self.data_serial.fetch_add(1, Ordering::SeqCst).to_be_bytes());
        let name = format!("{part1}_{part2}_{part3}.dat");
        self.get_unique_path_for_data_file(&name)
    }

    /// Builds the path to the data-file with a given name
    pub fn get_unique_path_for_data_file(&self, name: impl AsRef<Path>) -> DataFilePathBuf {
        AtraPathBuf::new(PathBuf::from(&self.big_file_folder).join(name))
    }



    /// Deletes a datafile
    pub fn cleanup_data_file(&self, name: impl AsRef<Path> + Debug) -> std::io::Result<()> {
        log::debug!("Delete the file {name:?}");
        let path = self.big_file_folder.join(name);
        std::fs::remove_file(path)
    }

    pub async fn create_worker_file_provider(&self, worker_id: usize) -> Result<WorkerFileProvider, FSAError> {
        let _ = self.filesystem_lock.lock().await;
        return WorkerFileProvider::build(worker_id, &self)
    }
}

/// A worker bound access for writing warcs
#[derive(Debug)]
pub struct WorkerFileProvider {
    warc_writer_serial: AtomicU16,
    warc_dir: PathBuf,
    service: String,
    collection: String,
    crawl_job_id: u64,
}

impl WorkerFileProvider {

    /// Creates the new WorkerWarcFileProvider
    fn build(
        worker_id: usize,
        base: &FileSystemAccess
    ) -> Result<Self, FSAError> {
        let worker_dir = base.output_folder.join(format!("worker_{}", worker_id));
        if !worker_dir.exists() {
            std::fs::create_dir_all(&worker_dir).to_fsa_error(|| worker_dir.to_string())?;
        }
        let warc_dir = worker_dir.join("warc");
        if !warc_dir.exists() {
            std::fs::create_dir_all(&warc_dir).to_fsa_error(|| warc_dir.to_string())?;
        }
        Ok(
            Self {
                service: base.service.clone(),
                collection: base.collection.clone(),
                crawl_job_id: base.crawl_job_id,
                warc_dir,
                warc_writer_serial: AtomicU16::default()
            }
        )
    }

    /// Increment the serial
    fn get_serial(serial: &AtomicU16) -> u16 {
        unsafe {
            serial.fetch_update(
                Ordering::SeqCst,
                Ordering::Relaxed,
                |next| Some(next.overflowing_add(1).0)
            ).unwrap_unchecked()
        }
    }

    /// Creates a file in the dir of a specific worker.
    /// Also makes sure, that the returned file is new IFF [NEW] is set.
    fn create_worker_file<const NEW: bool>(&self, path: Utf8PathBuf, serial: u16, extension: &str) -> Result<(File, Utf8PathBuf), FSAError> {
        let context = Template::new(
            &self.service,
            &self.collection,
            self.crawl_job_id,
            serial,
            extension
        );
        let final_path = path.join(context.to_string());
        let file = if NEW {
            File::options().read(true).write(true).create_new(true).open(&final_path).to_fsa_error(|| final_path.clone().into_string())?
        } else {
            File::create(&final_path).to_fsa_error(|| final_path.clone().into_string())?
        };
        Ok((file, final_path))
    }

    /// Build the proper extension for a file
    fn build_extension(base: &'static str, others: Option<impl AsRef<str>>) -> Cow<str> {
        if let Some(extensions) = others {
            let value = extensions.as_ref();
            if value.starts_with('.') {
                Cow::Owned(format!("{base}{value}"))
            } else {
                Cow::Owned(format!("{base}.{value}"))
            }
        } else {
            Cow::Borrowed(base)
        }
    }

    /// Creates a fresh warc file
    pub fn create_fresh_warc_file<T: AsRef<str>>(&self, additional_extension: Option<T>) -> Result<(File, Utf8PathBuf), FSAError> {
        let serial = Self::get_serial(&self.warc_writer_serial);
        let extension = Self::build_extension("warc", additional_extension);
        self.create_worker_file::<true>(self.warc_dir.clone(), serial, extension.as_ref())
    }

    #[allow(dead_code)]
    pub fn warc_file_reader<P: AsRef<Utf8Path>>(&self, path: P) -> Result<WarcCursor<File>, FSAError> {
        let path = path.as_ref();
        let file = if path.exists() {
            File::options().read(true).open(path).to_fsa_error(|| path.to_string())?
        } else {
            let new = self.warc_dir.clone().join(path);
            File::options().read(true).open(new).to_fsa_error(|| path.to_string())?
        };
        Ok(WarcCursor::new(file))
    }
}

impl From<&Configs> for FileSystemAccess {
    fn from(value: &Configs) -> Self {
        Self::new(
            value.session().service_name.clone(),
            value.session().collection_name.clone(),
            value.session().crawl_job_id,
            value.paths().root_path(),
            value.paths().dir_big_files(),
        )
    }
}

/// A template used to generate a specific file names
#[derive(Debug, Clone, Deserialize, Serialize)]
struct Template<'a> {
    service: &'a str,
    collection: &'a str,
    crawl_job_id: u64,
    timestamp: OffsetDateTime,
    serial: u16,
    extensions: &'a str
}
impl<'a> Template<'a> {
    pub fn new(
        service: &'a str,
        collection: &'a str,
        crawl_job_id: u64,
        serial: u16,
        extensions: &'a str
    ) -> Self {
        Self {
            service,
            collection,
            crawl_job_id,
            timestamp: SystemTime::now().into(),
            serial,
            extensions
        }
    }
}
impl Display for Template<'_> {

    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}-{}-{}-{}-{}.{}",
            self.service,
            self.collection,
            self.crawl_job_id,
            self.timestamp.unix_timestamp_nanos(),
            self.serial,
            self.extensions
        )
    }
}
