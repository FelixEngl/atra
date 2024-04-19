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

use std::cmp::min;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom};
use std::sync::{Arc, };
use camino::{Utf8Path};
use data_encoding::BASE64;
use itertools::{Itertools, Position};
use tokio::sync::RwLock;
use thiserror::Error;
use time::Duration;
use ubyte::ByteUnit;
use crate::core::blacklist::PolyBlackList;
use crate::core::config::Configs;
use crate::core::contexts::{Context, CrawlTaskContext, LinkHandlingError, RecoveryCommand, RecoveryError, SlimCrawlTaskContext};
use crate::core::crawl::result::CrawlResult;
use crate::core::crawl::seed::CrawlSeed;
use crate::core::crawl::slim::{SlimCrawlResult, StoredDataHint};
use crate::core::database_error::DatabaseError;
use crate::core::extraction::ExtractedLink;
use crate::core::io::fs::{FSAError, ToFSAError, WorkerFileProvider};
use crate::core::io::paths::DataFilePathBuf;
use crate::core::link_state::{LinkState, LinkStateDBError, LinkStateType};
use crate::core::{UrlWithDepth, VecDataHolder};
use crate::core::warc::{SpecialWarcWriter, WarcSkipInstruction, write_warc};
use crate::core::warc::writer::{WarcSkipPointer, WarcSkipPointerWithOffsets};
use crate::warc::header::{WarcHeader};
use crate::warc::writer::{WarcWriter, WarcWriterError};

/// A context for a specific worker
#[derive(Debug)]
pub struct WorkerContext<T: SlimCrawlTaskContext> {
    worker_id: usize,
    inner: Arc<T>,
    worker_file_provider: Arc<WorkerFileProvider>,
    worker_warc_writer: WorkerWarcWriter
}

impl<T: SlimCrawlTaskContext> Clone for WorkerContext<T> {
    fn clone(&self) -> Self {
        Self {
            worker_id: self.worker_id,
            inner: self.inner.clone(),
            worker_file_provider: self.worker_file_provider.clone(),
            worker_warc_writer: self.worker_warc_writer.clone()
        }
    }
}

impl<T: SlimCrawlTaskContext> WorkerContext<T> {
    pub fn worker_id(&self) -> usize {
        self.worker_id
    }

    pub async fn create(worker_id: usize, inner: Arc<T>) -> Result<Self, FSAError> {
        let worker_warc_system = inner.fs().create_worker_file_provider(worker_id).await?;
        Ok(Self::new(worker_id, inner, worker_warc_system)?)
    }

    pub fn new(worker_id: usize, inner: Arc<T>, worker_warc_system: WorkerFileProvider) -> Result<Self, FSAError> {
        let worker_file_provider = Arc::new(worker_warc_system);
        let worker_warc_writer = WorkerWarcWriter::new(worker_file_provider.clone())?;
        Ok(
            Self {
                worker_id,
                inner,
                worker_file_provider,
                worker_warc_writer
            }
        )
    }

    async fn read_body(&self, pointer: &WarcSkipPointerWithOffsets, header_octet_count: u32) -> Result<Option<Vec<u8>>, DatabaseError> {
        self.worker_warc_writer.make_sure_is_not_write_target(pointer.file()).await?;
        let mut file = File::options().read(true).open(pointer.file())?;
        let header_octet_count = header_octet_count as u64;
        file.seek(SeekFrom::Start(pointer.position() + pointer.warc_header_offset() as u64 + header_octet_count))?;
        let mut to_read = (pointer.body_octet_count() - header_octet_count) as usize;
        if to_read == 0 {
            return Ok(None)
        }

        let mut data = Vec::new();
        const BUF_SIZE: usize = ByteUnit::Megabyte(2).as_u64() as usize;
        let buffer = &mut [0u8; BUF_SIZE];
        while data.len() < to_read {
            file.read(&mut buffer[..min(BUF_SIZE, to_read)])?;
            data.extend_from_slice(&buffer[..min(BUF_SIZE, to_read)]);
            to_read = to_read.saturating_sub(BUF_SIZE);
        }
        return Ok(Some(data));
    }
}

impl<T: SlimCrawlTaskContext> Context for WorkerContext<T> {
    type RobotsManager = T::RobotsManager;
    type UrlQueue = T::UrlQueue;
    type DomainManager = T::DomainManager;
    type WebGraphManager = T::WebGraphManager;

    delegate::delegate! {
        to self.inner {
            async fn can_poll(&self) -> bool;

            /// Provides access to the filesystem
            fn fs(&self) -> &crate::core::io::fs::FileSystemAccess;

            /// The number of crawled websites
            fn crawled_websites(&self) -> Result<u64, LinkStateDBError>;

            /// The amount of discovered websites.
            fn discovered_websites(&self) -> usize;

            /// Get the instance of the url queue.
            fn url_queue(&self) -> &Self::UrlQueue;

            /// Returns a reference to the config
            fn configs(&self) -> &Configs;

            /// When did the crawl officially start?
            fn crawl_started_at(&self) -> time::OffsetDateTime;

            /// Returns the link net manager
            fn web_graph_manager(&self) -> &Self::WebGraphManager;

            /// Get some kind of blacklist
            async fn get_blacklist(&self) -> PolyBlackList;

            /// Get an instance of the robots manager.
            async fn get_robots_instance(&self) -> Self::RobotsManager;

            /// Returns a reference to a [GuardedDomainManager]
            fn get_domain_manager(&self) -> &Self::DomainManager;

            /// Retrieve a single crawled website but without the body
            async fn retrieve_slim_crawled_website(&self, url: &UrlWithDepth) -> Result<Option<SlimCrawlResult>, DatabaseError>;

            /// Registers a seed in the context as beeing crawled.
            async fn register_seed(&self, seed: &impl CrawlSeed) -> Result<(), LinkHandlingError>;

            /// Register outgoing & data links.
            /// Also returns a list of all urls existing on the seed, that can be registered.
            async fn handle_links(&self, from: &UrlWithDepth, links: &HashSet<ExtractedLink>) -> Result<Vec<UrlWithDepth>, LinkHandlingError>;

            /// Sets the state of the link
            async fn update_link_state(&self, url: &UrlWithDepth, state: LinkStateType) -> Result<(), LinkStateDBError>;

            /// Sets the state of the link with a payload
            async fn update_link_state_with_payload(&self, url: &UrlWithDepth, state: LinkStateType, payload: Vec<u8>) -> Result<(), LinkStateDBError>;

            /// Gets the state of the current url
            async fn get_link_state(&self, url: &UrlWithDepth) -> Result<Option<LinkState>, LinkStateDBError>;

            /// Checks if there are any crawable links. [max_age] denotes the maximum amount of time since
            /// the last search
            async fn check_if_there_are_any_crawlable_links(&self, max_age: Duration) -> bool;

            /// Recover the
            async fn recover<'a>(&self, command: RecoveryCommand<'a>) -> Result<(), RecoveryError>;
        }
    }
}

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error(transparent)]
    Database(#[from] DatabaseError)
}

impl<T: SlimCrawlTaskContext> SlimCrawlTaskContext for WorkerContext<T> {
    async fn store_slim_crawled_website(&self, slim: SlimCrawlResult) -> Result<(), DatabaseError> {
        self.inner.store_slim_crawled_website(slim).await
    }
}

impl<T: SlimCrawlTaskContext> CrawlTaskContext for WorkerContext<T> {

    async fn store_crawled_website(&self, result: &CrawlResult) -> Result<(), DatabaseError> {
        let hint = match &result.content {
            VecDataHolder::None => {StoredDataHint::None}
            VecDataHolder::InMemory { .. } => {
                log::debug!("Store in warc: {}", result.url);
                StoredDataHint::Warc(self.worker_warc_writer.execute_on_writer(|value| {
                    log::debug!("WARC-Writer start:");
                    write_warc(result, value)
                }).await?)
            }
            VecDataHolder::ExternalFile { file } => {
                log::debug!("Store external");
                StoredDataHint::External(file.clone())
            }
        };
        log::debug!("Store slim: {}", result.url);
        self.store_slim_crawled_website(SlimCrawlResult::new(result, hint)).await
    }



    async fn retrieve_crawled_website(&self, url: &UrlWithDepth) -> Result<Option<CrawlResult>, DatabaseError> {
        if let Some(found) = self.retrieve_slim_crawled_website(url).await? {
            match &found.stored_data_hint {
                StoredDataHint::External(_) | StoredDataHint::None | StoredDataHint::InMemory(_) => {
                    Ok(Some(found.inflate(None)))
                }
                StoredDataHint::Warc(pointers) => {
                    match pointers {
                        WarcSkipInstruction::Single{pointer, header_signature_octet_count: header_octet_count, is_base64 } => {
                            let data = self.read_body(pointer, *header_octet_count).await?;
                            let data = if *is_base64 {
                                if let Some(value) = data {
                                    Some(BASE64.decode(&value)?)
                                } else {
                                    None
                                }
                            } else {
                                data
                            };
                            Ok(Some(found.inflate(data)))
                        },
                        WarcSkipInstruction::Multiple{ pointers, header_signature_octet_count: header_octet_count, is_base64 } => {
                            let mut collected_data = Vec::new();
                            for (pos, value) in pointers.iter().with_position() {
                                match pos {
                                    Position::First | Position::Only => {
                                        match self.read_body(value, *header_octet_count).await? {
                                            None => {}
                                            Some(value) => {
                                                collected_data.extend(value)
                                            }
                                        }
                                    }
                                    _ => {
                                        match self.read_body(value, 0).await? {
                                            None => {}
                                            Some(value) => {
                                                collected_data.extend(value)
                                            }
                                        }
                                    }
                                }

                            }
                            if collected_data.is_empty() {
                                Ok(Some(found.inflate(None)))
                            } else {
                                let collected_data = if *is_base64 {
                                    BASE64.decode(&collected_data)?
                                } else {
                                    collected_data
                                };
                                Ok(Some(found.inflate(Some(collected_data))))
                            }

                        }
                    }
                }
                StoredDataHint::Associated => unreachable!()
            }
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug)]
pub struct WorkerWarcWriter {
    writer: Arc<RwLock<RawWorkerWarcWriter>>
}

impl WorkerWarcWriter {
    pub fn new(fp: Arc<WorkerFileProvider>) -> Result<Self, FSAError> {
        let (file, path) = fp.create_fresh_warc_file(None::<&str>)?;
        Ok(
            Self {
                writer: Arc::new(RwLock::new(RawWorkerWarcWriter::new(
                    fp,
                    WarcWriter::new(BufWriter::new(file)),
                    DataFilePathBuf::new(path)
                ))),
            }
        )
    }

    #[allow(dead_code)]
    pub async fn current_file(&self) -> DataFilePathBuf {
        let writer = self.writer.read().await;
        writer.path.clone()
    }

    #[allow(dead_code)]
    pub async fn flush(&self) -> Result<(), FSAError> {
        let mut writer = self.writer.write().await;
        writer.flush()
    }

    pub async fn execute_on_writer<R, E, F: FnOnce(&mut RawWorkerWarcWriter) -> Result<R, E>>(&self, to_execute: F) -> Result<R, E> {
        log::trace!("Get WARC-Write lock");
        let mut writer = self.writer.write().await;
        log::trace!("Get WARC-Write lock - success");
        to_execute(&mut writer)
    }

    pub async fn make_sure_is_not_write_target(&self, path: &Utf8Path) -> Result<(), FSAError> {
        let writer = self.writer.read().await;
        if writer.path.as_path().eq(path) {
            drop(writer);
            let mut writer = self.writer.write().await;
            let _ = writer.forward_if_filesize(0)?;
            Ok(())
        } else {
            Ok(())
        }
    }
}

impl Clone for WorkerWarcWriter {
    fn clone(&self) -> Self {
        Self {
            writer: self.writer.clone()
        }
    }
}


#[derive(Debug)]
pub struct RawWorkerWarcWriter {
    fp: Arc<WorkerFileProvider>,
    writer: WarcWriter<BufWriter<File>>,
    path: DataFilePathBuf
}

impl RawWorkerWarcWriter {

    pub fn new(
        fp: Arc<WorkerFileProvider>,
        writer: WarcWriter<BufWriter<File>>,
        path: DataFilePathBuf
    ) -> Self {
        Self {
            fp,
            writer,
            path
        }
    }

    #[allow(dead_code)]
    fn flush(&mut self) -> Result<(), FSAError> {
        self.writer.flush().to_fsa_error(|| self.path.to_string())
    }

    fn replace_writer(&mut self, writer: WarcWriter<BufWriter<File>>, path: DataFilePathBuf) -> (WarcWriter<BufWriter<File>>, DataFilePathBuf) {
        (std::mem::replace(&mut self.writer, writer), std::mem::replace(&mut self.path, path))
    }
}

impl SpecialWarcWriter for RawWorkerWarcWriter {
    fn get_skip_pointer(&self) -> Result<WarcSkipPointer, WarcWriterError> {
        self.writer.check_if_state(crate::warc::states::State::ExpectHeader)?;
        Ok(
            WarcSkipPointer::new(
                self.path.clone(),
                self.writer.bytes_written() as u64
            )
        )
    }

    unsafe fn get_skip_pointer_unchecked(&self) -> WarcSkipPointer {
        WarcSkipPointer::new(
            self.path.clone(),
            self.writer.bytes_written() as u64
        )
    }


    #[inline] fn bytes_written(&self) -> usize {
        self.writer.bytes_written()
    }

    #[inline] fn write_header(&mut self, header: WarcHeader) -> Result<usize, WarcWriterError> {
        self.writer.write_header(&header)
    }

    #[inline] fn write_body_complete(&mut self, buf: &[u8]) -> Result<usize, WarcWriterError> {
        self.writer.write_complete_body(buf)
    }


    #[inline] fn write_body<R: Read>(&mut self, body: &mut R) -> Result<usize, WarcWriterError> {
        self.writer.write_body(body)
    }

    #[inline] fn write_empty_body(&mut self) -> Result<usize, WarcWriterError> {
        self.writer.write_complete_body(&[])
    }

    fn forward(&mut self) -> Result<DataFilePathBuf, FSAError> {
        let (file, path) = self.fp.create_fresh_warc_file(None::<&str>)?;
        let (mut old_writer, path) = self.replace_writer(
            WarcWriter::new(BufWriter::new(file)),
            DataFilePathBuf::new(path)
        );
        old_writer.flush().map_err(|value| FSAError::IOError(path.as_path().to_string(), value))?;
        Ok(path)
    }
}


#[cfg(test)]
pub(crate) mod test {
    use std::path::Path;
    use std::sync::Arc;
    use camino::Utf8PathBuf;
    use ubyte::ByteUnit;
    use crate::core::config::Configs;
    use crate::core::contexts::{Context, CrawlTaskContext, LocalContext};
    use crate::core::contexts::worker_context::{WorkerContext, WorkerWarcWriter};
    use crate::core::crawl::result::test::{create_test_data, create_test_data_unknown, create_testdata_with_on_seed};
    use crate::core::io::fs::{FileSystemAccess};
    use crate::core::{UrlWithDepth, VecDataHolder};
    use crate::core::warc::SpecialWarcWriter;
    use crate::util::RuntimeContext;
    use crate::warc::parser::test::create_test_header;


    pub async fn create_writers() -> (FileSystemAccess, WorkerWarcWriter) {
        let x = Utf8PathBuf::from("test\\data");
        if x.exists() {
            std::fs::remove_dir_all(x).unwrap();
        }


        let fs = FileSystemAccess::new(
            "test_service".to_string(),
            "test_collection".to_string(),
            0,
            Utf8PathBuf::from("test\\data"),
            Utf8PathBuf::from("test\\data\\blobs"),
        );

        let wwr = WorkerWarcWriter::new(
            Arc::new(
                fs.create_worker_file_provider(0).await.unwrap()
            )
        ).unwrap();

        (fs, wwr)
    }

    #[tokio::test]
    async fn writer_test(){
        let (_, wwr) = create_writers().await;

        const DATA1: &[u8] = b"TEXT1.....bla";
        const DATA2: &[u8] = b"TEXT2.....bla";
        const DATA3: &[u8] = b"TEXT3.....bla";

        wwr.execute_on_writer::<(), anyhow::Error, _>(|writer| {
            writer.write_header(create_test_header("google", DATA1.len() as u64))?;
            writer.write_body_complete(DATA1)?;
            writer.write_header(create_test_header("amazon", DATA2.len() as u64))?;
            writer.write_body_complete(DATA2)?;
            writer.write_header(create_test_header("catsanddogs", DATA2.len() as u64))?;
            writer.write_empty_body()?;
            let _ = writer.forward_if_filesize(0)?;
            writer.write_header(create_test_header("ebay", DATA3.len() as u64))?;
            writer.write_body_complete(&DATA3)?;
            Ok(())
        }).await.unwrap();


    }

    #[tokio::test]
    async fn test_context() {

        if Path::new("test").exists() {
            std::fs::remove_dir_all("test").unwrap();
        }


        let mut cfg = Configs::default();
        cfg.paths.root_folder = "test".to_string();

        let local = Arc::new(LocalContext::new(cfg, RuntimeContext::unbound()).await.unwrap());

        let worker = WorkerContext::create(0, local.clone()).await.unwrap();
        let test_data1 = create_testdata_with_on_seed(None);
        const BIG_DATA: [u8; ByteUnit::Gigabyte(1).as_u64() as usize - 20] = [b'a'; {ByteUnit::Gigabyte(1).as_u64() as usize - 20}];

        let test_data2 = create_test_data(UrlWithDepth::from_seed("https://www.oofsize.de/").unwrap(), Some(VecDataHolder::from_vec(BIG_DATA.to_vec())));
        let test_data3 = create_test_data(UrlWithDepth::from_seed("https://www.catsanddogs.de/").unwrap(), None);
        worker.store_crawled_website(&test_data1).await.unwrap();
        worker.store_crawled_website(&test_data2).await.unwrap();
        worker.store_crawled_website(&test_data3).await.unwrap();

        let x = UrlWithDepth::from_seed("https://www.oofsize.de/").unwrap();

        let found = worker.retrieve_slim_crawled_website(&x).await.unwrap();
        println!("{:?}", found);

        let retrieved = worker.retrieve_crawled_website(&test_data1.url).await.expect("This should work").expect("Expected to exist!");
        println!("->{}<-", String::from_utf8(test_data1.content.as_in_memory().unwrap().clone()).unwrap());
        println!("->{}<-", String::from_utf8(retrieved.content.as_in_memory().unwrap().clone()).unwrap());
        assert_eq!(retrieved, test_data1);
    }

    #[tokio::test]
    async fn test_context2() {

        if Path::new("test").exists() {
            std::fs::remove_dir_all("test").unwrap();
        }


        let mut cfg = Configs::default();
        cfg.paths.root_folder = "test".to_string();

        let local = Arc::new(LocalContext::new(cfg, RuntimeContext::unbound()).await.unwrap());

        let worker = WorkerContext::create(0, local.clone()).await.unwrap();
        let test_data1 = create_testdata_with_on_seed(None);
        const BIG_DATA: [u8; ByteUnit::Gigabyte(1).as_u64() as usize - 20] = [b'a'; {ByteUnit::Gigabyte(1).as_u64() as usize - 20}];

        let test_data2 = create_test_data_unknown(UrlWithDepth::from_seed("https://www.oofsize.de/").unwrap(), VecDataHolder::from_vec(BIG_DATA.to_vec()));
        let test_data3 = create_test_data(UrlWithDepth::from_seed("https://www.catsanddogs.de/").unwrap(), None);
        worker.store_crawled_website(&test_data1).await.unwrap();
        worker.store_crawled_website(&test_data2).await.unwrap();
        worker.store_crawled_website(&test_data3).await.unwrap();

        let x = UrlWithDepth::from_seed("https://www.oofsize.de/").unwrap();

        let found = worker.retrieve_crawled_website(&x).await.expect("This should work").expect("Expected to exist!");
        assert_eq!(test_data2, found, "Failed to compare:\n\nA: {:?}\n\nB: {:?}", test_data2, found);

        let retrieved = worker.retrieve_crawled_website(&test_data1.url).await.expect("This should work").expect("Expected to exist!");
        assert_eq!(test_data1, retrieved);
    }
}