// Copyright 2024 Felix Engl
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::io::errors::{ErrorWithPath, ToErrorWithPath};
use crate::io::serial::{SerialProvider, SerialProviderKind, SerialValue};
use crate::io::templating::{
    file_name_template, FileNameTemplate, FileNameTemplateArgs, RecoverInstruction,
};
use crate::io::unique_path_provider::{UniquePathProvider, UniquePathProviderWithTemplate};
use crate::stores::warc::WarcFilePathProvider;
use byteorder::WriteBytesExt;
use camino::{Utf8Path, Utf8PathBuf};
use regex::Regex;
use std::cmp::max;
use std::fmt::Debug;
use std::fs::File;
use std::hash::Hash;
use std::io;
use std::io::{BufRead, BufReader, BufWriter, ErrorKind};
use std::sync::Mutex;
use std::sync::{Arc, LazyLock};
use twox_hash::xxh3::HasherExt;

pub trait AtraFS {
    /// Creates a unique path to a fresh data file.
    fn create_unique_path_for_dat_file(&self, url: &str) -> Utf8PathBuf;

    /// Builds the path to the data-file with a given name
    fn get_unique_path_for_data_file(&self, path: impl AsRef<Utf8Path>) -> Utf8PathBuf;

    /// Deletes a datafile
    fn cleanup_data_file(&self, path: impl AsRef<Utf8Path>) -> io::Result<()>;

    fn create_worker_file_provider(
        &self,
        worker_id: usize,
        recrawl_iteration: usize,
    ) -> Result<WorkerFileSystemAccess, ErrorWithPath>;
}

/// Provides the paths in the application
#[derive(Debug)]
pub struct FileSystemAccess {
    collection_root: Utf8PathBuf,
    worker_base: FileNameTemplate,
    big_file: UniquePathProviderWithTemplate,
    filesystem_lock: Mutex<()>,
}

impl FileSystemAccess {
    pub fn new(
        service: String,
        collection: String,
        crawl_job_id: u64,
        output_folder: Utf8PathBuf,
        big_file_folder: Utf8PathBuf,
    ) -> Result<Self, ErrorWithPath> {
        let collection_root = output_folder.join(&collection);
        if !collection_root.exists() {
            std::fs::create_dir_all(&collection_root).to_error_with_path(&collection_root)?;
        }

        let template_base = file_name_template!(service _ crawl_job_id).unwrap();

        if !big_file_folder.exists() {
            std::fs::create_dir_all(&big_file_folder).to_error_with_path(&collection_root)?;
        }

        let path_provider_big_file = UniquePathProvider::new(big_file_folder, Default::default())
            .with_template(file_name_template!(arg!@"url" _ timestamp64 _ serial ".dat").unwrap());

        Ok(Self {
            collection_root,
            worker_base: template_base,
            big_file: path_provider_big_file,
            filesystem_lock: Mutex::new(()),
        })
    }
}

impl AtraFS for FileSystemAccess {
    /// Creates a unique path to a fresh data file.
    fn create_unique_path_for_dat_file(&self, url: &str) -> Utf8PathBuf {
        let mut args = FileNameTemplateArgs::with_capacity(1);
        let mut hasher = twox_hash::xxh3::Hash128::default();
        url.hash(&mut hasher);
        args.insert("url", hasher.finish_ext().to_string());
        return self.big_file.provide_path_with_args(&args).unwrap();
    }

    /// Builds the path to the data-file with a given name
    fn get_unique_path_for_data_file(&self, name: impl AsRef<Utf8Path>) -> Utf8PathBuf {
        self.big_file.root().join(name)
    }

    /// Deletes a datafile
    fn cleanup_data_file(&self, name: impl AsRef<Utf8Path>) -> io::Result<()> {
        log::debug!("Delete the file {}", name.as_ref().to_string());
        let path = self.big_file.root().join(name);
        std::fs::remove_file(path)
    }

    fn create_worker_file_provider(
        &self,
        worker_id: usize,
        recrawl_iteration: usize,
    ) -> Result<WorkerFileSystemAccess, ErrorWithPath> {
        let _unused = self.filesystem_lock.lock();
        WorkerFileSystemAccess::new(
            self.collection_root.clone(),
            self.worker_base.clone(),
            worker_id,
            recrawl_iteration,
        )
    }
}

/// A worker bound access for writing warcs
#[derive(Debug)]
pub struct WorkerFileSystemAccess {
    root: Utf8PathBuf,
    provider: Arc<UniquePathProviderWithTemplate>,
    journal: Arc<Mutex<BufWriter<File>>>,
}

static FILE_NAME_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("rc_(\\d+)_(\\d+)\\.warc").unwrap());

impl WorkerFileSystemAccess {
    pub fn new(
        collection_root: Utf8PathBuf,
        worker_base: FileNameTemplate,
        worker_id: usize,
        recrawl_iteration: usize,
    ) -> Result<Self, ErrorWithPath> {
        let worker_root = collection_root.join(format!("worker_{worker_id}"));
        let path_to_journal = worker_root.join("warc.journal");

        let recover_instruction = if path_to_journal.exists() {
            let reader = File::options()
                .read(true)
                .open(&path_to_journal)
                .to_error_with_path(&path_to_journal)?;
            let last_pos = BufReader::new(reader)
                .lines()
                .filter_map(|value| {
                    value
                        .ok()
                        .and_then(|value| (!value.is_empty()).then_some(value))
                })
                .last();
            if let Some(last) = last_pos {
                serde_json::from_str::<RecoverInstruction>(&last).ok()
            } else {
                None
            }
        } else {
            None
        };

        let (provider, recover) = if !worker_root.exists()
            || !worker_root
                .read_dir_utf8()
                .is_ok_and(|mut value| value.next().is_some())
        {
            std::fs::create_dir_all(&worker_root).to_error_with_path(&worker_root)?;
            (UniquePathProvider::new(&worker_root, SerialProviderKind::Long.into()).with_template(
                    file_name_template!(ref worker_base _ worker_id _ "rc" _ recrawl_iteration _ serial ".warc")
                        .unwrap(),
                ), false)
        } else if let Some(recover) = recover_instruction {
            let mut provider = UniquePathProvider::new(&worker_root, SerialProviderKind::Long.into()).with_template(
                    file_name_template!(ref worker_base _ worker_id _ "rc" _ recrawl_iteration _ serial ".warc")
                        .unwrap(),
                );
            provider.recover(recover);
            let _ = provider.provide_path_no_args();
            (provider, true)
        } else {
            log::warn!("Failed to find recover information. Start with fallback recovery mode.");
            let regex = FILE_NAME_REGEX.clone();
            let mut last_serial = 0;
            for file in worker_root
                .read_dir_utf8()
                .to_error_with_path(&worker_root)?
            {
                if let Ok(file) = file {
                    let ft = file.file_type().to_error_with_path(&worker_root)?;
                    if ft.is_file() {
                        if let Some(cap) = regex.captures(file.file_name()) {
                            let recrawl_read: u64 = if let Ok(value) = cap[1].parse() {
                                value
                            } else {
                                continue;
                            };
                            if recrawl_read as usize != recrawl_iteration {
                                continue;
                            }
                            let serial_read: u64 = if let Ok(value) = cap[2].parse::<u64>() {
                                value + 1
                            } else {
                                continue;
                            };
                            last_serial = max(last_serial, serial_read as usize);
                        }
                    }
                }
            }
            (UniquePathProvider::new(&worker_root, SerialProvider::with_initial_state(
                    SerialValue::Long(last_serial as u64)
                )).with_template(
                    file_name_template!(ref worker_base _ worker_id _ "rc" _ recrawl_iteration _ serial ".warc")
                        .unwrap(),
                ), true)
        };

        if recover {
            loop {
                let result = provider
                    .current_path_no_args()
                    .expect("This should never fail!");
                if !result.exists() {
                    break;
                }
                let _ = provider
                    .provide_path_no_args()
                    .expect("This should never fail!");
            }
        }

        let journal = BufWriter::new(
            File::options()
                .write(true)
                .create(true)
                .append(true)
                .open(&path_to_journal)
                .to_error_with_path(&path_to_journal)?,
        );

        Ok(Self {
            root: worker_root,
            provider: Arc::new(provider),
            journal: Arc::new(Mutex::new(journal)),
        })
    }

    fn update_journal(&self) {
        let recover = self.provider.get_recover_information();
        let mut w = self.journal.lock().unwrap();
        if serde_json::to_writer(w.get_mut(), &recover).is_err() {
            log::warn!(
                "Failed to write to warc journey for {}",
                self.root.join("warc.journal").to_string()
            )
        }
        let _ = w.get_mut().write_u8(b'\n');
    }
}

impl WarcFilePathProvider for WorkerFileSystemAccess {
    fn create_new_warc_file_path(&self) -> Result<Utf8PathBuf, ErrorWithPath> {
        let mut last: Option<Utf8PathBuf> = None;
        loop {
            let result = self.provider.provide_path_no_args().unwrap();
            if !result.exists() {
                self.update_journal();
                break Ok(result);
            }
            match last {
                None => {
                    last = Some(result);
                }
                Some(value) => {
                    if result == value {
                        return Err(
                            ErrorWithPath::new(
                                result,
                                io::Error::new(ErrorKind::AlreadyExists, "The path was already generated once, this should not be happening!")
                            )
                        );
                    } else {
                        last = Some(result);
                    }
                }
            }
        }
    }
}

impl Drop for WorkerFileSystemAccess {
    fn drop(&mut self) {
        self.update_journal();
    }
}

#[cfg(test)]
mod test {
    use crate::io::fs::FILE_NAME_REGEX;

    #[test]
    fn can_properly_parse() {
        const TEST: &str = "atra_0_0_rc_42_123.warc";
        let cap = FILE_NAME_REGEX.captures(TEST).expect("Can read fn");
        println!("{}", &cap[1]);
        println!("{}", &cap[2]);
        let recrawl_read: u64 = (&cap[1]).parse().expect("Expected a read recrawl info");
        assert_eq!(42, recrawl_read, "Failed recrawl read: {recrawl_read}");
        let serial_read: u64 = (&cap[2]).parse().expect("Expected a serial info");
        assert_eq!(123, serial_read, "Failed serial read: {recrawl_read}");
    }
}
