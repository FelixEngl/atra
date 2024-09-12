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
use crate::io::templating::{file_name_template, FileNameTemplate, FileNameTemplateArgs};
use crate::io::unique_path_provider::{UniquePathProvider, UniquePathProviderWithTemplate};
use crate::stores::warc::WarcFilePathProvider;
use camino::{Utf8Path, Utf8PathBuf};
use data_encoding::BASE64URL_NOPAD;
use std::fmt::Debug;
use std::io;
use std::io::ErrorKind;
use tokio::sync::Mutex;

pub trait AtraFS {
    /// Creates a unique path to a fresh data file.
    fn create_unique_path_for_dat_file(&self, url: &str) -> Utf8PathBuf;

    /// Builds the path to the data-file with a given name
    fn get_unique_path_for_data_file(&self, name: impl AsRef<Utf8Path>) -> Utf8PathBuf;

    /// Deletes a datafile
    fn cleanup_data_file(&self, name: impl AsRef<Utf8Path> + Debug) -> io::Result<()>;

    async fn create_worker_file_provider(
        &self,
        worker_id: usize,
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

        let path_provider_big_file = UniquePathProvider::new(big_file_folder).with_template(
            file_name_template!(arg!@"url64" _ timestamp64 _ serial ".dat").unwrap(),
        );

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
        args.insert("url64", BASE64URL_NOPAD.encode(url.as_bytes()));
        return self.big_file.provide_path_with_args(&args).unwrap();
    }

    /// Builds the path to the data-file with a given name
    fn get_unique_path_for_data_file(&self, name: impl AsRef<Utf8Path>) -> Utf8PathBuf {
        self.big_file.root().join(name)
    }

    /// Deletes a datafile
    fn cleanup_data_file(&self, name: impl AsRef<Utf8Path> + Debug) -> io::Result<()> {
        log::debug!("Delete the file {name:?}");
        let path = self.big_file.root().join(name);
        std::fs::remove_file(path)
    }

    async fn create_worker_file_provider(
        &self,
        worker_id: usize,
    ) -> Result<WorkerFileSystemAccess, ErrorWithPath> {
        let _ = self.filesystem_lock.lock().await;
        WorkerFileSystemAccess::new(
            self.collection_root.clone(),
            self.worker_base.clone(),
            worker_id,
        )
    }
}

/// A worker bound access for writing warcs
#[derive(Debug)]
pub struct WorkerFileSystemAccess {
    provider: UniquePathProviderWithTemplate,
}

impl WorkerFileSystemAccess {
    pub fn new(
        collection_root: Utf8PathBuf,
        worker_base: FileNameTemplate,
        worker_id: usize,
    ) -> Result<Self, ErrorWithPath> {
        let worker_root = collection_root.join(format!("worker_{worker_id}"));
        if !worker_root.exists() {
            std::fs::create_dir_all(&worker_root).to_error_with_path(&worker_root)?;
        }
        let provider = UniquePathProvider::new(&worker_root).with_template(
            file_name_template!(ref worker_base _ worker_id _ timestamp64 _ serial ".warc")
                .unwrap(),
        );
        Ok(Self {
            // worker_root,
            // worker_base,
            provider,
        })
    }
}

impl WarcFilePathProvider for WorkerFileSystemAccess {
    fn create_new_warc_file_path(&self) -> Result<Utf8PathBuf, ErrorWithPath> {
        let mut last: Option<Utf8PathBuf> = None;
        loop {
            let result = self.provider.provide_path_no_args().unwrap();
            if !result.exists() {
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