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

use crate::config::Config;
use crate::contexts::traits::*;
use crate::contexts::worker::error::CrawlWriteError;
use crate::crawl::StoredDataHint;
use crate::crawl::{CrawlResult, CrawlTask, SlimCrawlResult};
use crate::data::RawVecData;
use crate::extraction::ExtractedLink;
use crate::io::errors::{ErrorWithPath};
use crate::io::fs::{AtraFS, WorkerFileSystemAccess};
use crate::seed::BasicSeed;
use crate::stores::warc::ThreadsafeMultiFileWarcWriter;
use crate::url::UrlWithDepth;
use crate::warc_ext::write_warc;
use std::collections::HashSet;
use std::sync::Arc;
use text_processing::stopword_registry::StopWordRegistry;

/// A context for a specific worker
#[derive(Debug)]
pub struct WorkerContext<T> {
    worker_id: usize,
    inner: Arc<T>,
    worker_file_provider: Arc<WorkerFileSystemAccess>,
    worker_warc_writer: ThreadsafeMultiFileWarcWriter,
}

impl<T> AsyncContext for WorkerContext<T> where T: AsyncContext {}
impl<T> ContextDelegate for WorkerContext<T> {}

impl<T> SupportsWorkerId for WorkerContext<T> {
    fn worker_id(&self) -> usize {
        self.worker_id
    }
}

impl<T> WorkerContext<T>
where
    T: SupportsFileSystemAccess,
{
    pub fn create(
        worker_id: usize,
        recrawl_number: usize,
        inner: Arc<T>,
    ) -> Result<Self, ErrorWithPath> {
        let worker_warc_system = inner
            .fs()
            .create_worker_file_provider(worker_id, recrawl_number)?;
        Ok(Self::new(worker_id, inner, worker_warc_system)?)
    }

    pub fn new(
        worker_id: usize,
        inner: Arc<T>,
        worker_warc_system: WorkerFileSystemAccess,
    ) -> Result<Self, ErrorWithPath> {
        let worker_file_provider = Arc::new(worker_warc_system);
        let worker_warc_writer =
            ThreadsafeMultiFileWarcWriter::new_for_worker(worker_file_provider.clone())?;
        Ok(Self {
            worker_id,
            inner,
            worker_file_provider,
            worker_warc_writer,
        })
    }
}

impl<T> Clone for WorkerContext<T> {
    fn clone(&self) -> Self {
        Self {
            worker_id: self.worker_id,
            inner: self.inner.clone(),
            worker_file_provider: self.worker_file_provider.clone(),
            worker_warc_writer: self.worker_warc_writer.clone(),
        }
    }
}

impl<T> SupportsLinkSeeding for WorkerContext<T>
where
    T: SupportsLinkSeeding,
{
    type Error = T::Error;

    delegate::delegate! {
        to self.inner {
            async fn register_seed<S: BasicSeed>(&self, seed: &S) -> Result<(), Self::Error>;

            async fn handle_links(&self, from: &UrlWithDepth, links: &HashSet<ExtractedLink>) -> Result<Vec<UrlWithDepth>, Self::Error>;
        }
    }
}

impl<T> SupportsLinkState for WorkerContext<T>
where
    T: SupportsLinkState,
{
    type LinkStateManager = T::LinkStateManager;

    delegate::delegate! {
        to self.inner {
            fn get_link_state_manager(&self) -> &Self::LinkStateManager;
        }
    }
}

impl<T> SupportsUrlGuarding for WorkerContext<T>
where
    T: SupportsUrlGuarding,
{
    type Guardian = T::Guardian;

    delegate::delegate! {
        to self.inner {
            fn get_guardian(&self) -> &Self::Guardian;
        }
    }
}

impl<T> SupportsRobotsManager for WorkerContext<T>
where
    T: SupportsRobotsManager,
{
    type RobotsManager = T::RobotsManager;

    delegate::delegate! {
        to self.inner {
            fn get_robots_manager(&self) -> &Self::RobotsManager;
        }
    }
}

impl<T> SupportsBlackList for WorkerContext<T>
where
    T: SupportsBlackList,
{
    type BlacklistManager = T::BlacklistManager;

    delegate::delegate! {
        to self.inner {
            fn get_blacklist_manager(&self) -> &Self::BlacklistManager;
        }
    }
}

impl<T> SupportsMetaInfo for WorkerContext<T>
where
    T: SupportsMetaInfo,
{
    delegate::delegate! {
        to self.inner {
            fn crawl_started_at(&self) -> time::OffsetDateTime;

            fn discovered_websites(&self) -> usize;
        }
    }
}

impl<T> SupportsConfigs for WorkerContext<T>
where
    T: SupportsConfigs,
{
    delegate::delegate! {
        to self.inner {
            fn configs(&self) -> &Config;
        }
    }
}

impl<T> SupportsUrlQueue for WorkerContext<T>
where
    T: SupportsUrlQueue,
{
    type UrlQueue = T::UrlQueue;

    delegate::delegate! {
        to self.inner {
            async fn can_poll(&self) -> bool;

            fn url_queue(&self) -> &Self::UrlQueue;
        }
    }
}

impl<T> SupportsFileSystemAccess for WorkerContext<T>
where
    T: SupportsFileSystemAccess,
{
    type FileSystem = T::FileSystem;

    delegate::delegate! {
        to self.inner {
            fn fs(&self) -> &Self::FileSystem;
        }
    }
}

impl<T> SupportsWebGraph for WorkerContext<T>
where
    T: SupportsWebGraph,
{
    type WebGraphManager = T::WebGraphManager;

    delegate::delegate! {
        to self.inner {
            fn web_graph_manager(&self) -> &Self::WebGraphManager;
        }
    }
}

impl<T> SupportsStopwordsRegistry for WorkerContext<T>
where
    T: SupportsStopwordsRegistry,
{
    delegate::delegate! {
        to self.inner {
            fn stopword_registry(&self) -> Option<&StopWordRegistry>;
        }
    }
}

impl<T> SupportsGdbrRegistry for WorkerContext<T>
where
    T: SupportsGdbrRegistry,
{
    type Registry = T::Registry;

    delegate::delegate! {
        to self.inner {
           fn gdbr_registry(&self) -> Option<&Self::Registry>;
        }
    }
}
impl<T> SupportsSlimCrawlResults for WorkerContext<T>
where
    T: SupportsSlimCrawlResults,
{
    type Error = T::Error;
    delegate::delegate! {
        to self.inner {
            async fn retrieve_slim_crawled_website(&self, url: &UrlWithDepth) -> Result<Option<SlimCrawlResult>, Self::Error>;

            async fn store_slim_crawled_website(&self, result: SlimCrawlResult) -> Result<(), Self::Error>;
        }
    }
}

impl<T> SupportsDomainHandling for WorkerContext<T>
where
    T: SupportsDomainHandling,
{
    type DomainHandler = T::DomainHandler;
    delegate::delegate! {
        to self.inner {
            fn get_domain_manager(&self) -> &Self::DomainHandler;
        }
    }
}

impl<T> SupportsCrawlResults for WorkerContext<T>
where
    T: AsyncContext + SupportsSlimCrawlResults + SupportsConfigs,
{
    type Error = CrawlWriteError<T::Error>;

    async fn store_crawled_website(&self, result: &CrawlResult) -> Result<(), Self::Error> {
        let hint = match &result.content {
            RawVecData::None => StoredDataHint::None,
            RawVecData::InMemory { .. } => {
                log::debug!("Store in warc: {}", result.meta.url);
                StoredDataHint::Warc(
                    self.worker_warc_writer
                        .execute_on_writer(|value| {
                            log::debug!("WARC-Writer start:");
                            write_warc(value, result)
                        })
                        .await?,
                )
            }
            RawVecData::ExternalFile { file } => {
                log::debug!("Store external");
                if self.configs().crawl.store_big_file_hints_in_warc {
                    self.worker_warc_writer
                        .execute_on_writer(|value| write_warc(value, result))
                        .await?;
                }
                StoredDataHint::External(file.clone())
            }
        };
        log::debug!("Store slim: {}", result.meta.url);
        self.store_slim_crawled_website(SlimCrawlResult::new(result, hint))
            .await
            .map_err(CrawlWriteError::SlimError)
    }

    async fn retrieve_crawled_website(
        &self,
        url: &UrlWithDepth,
    ) -> Result<Option<CrawlResult>, Self::Error> {
        if let Some(found) = self
            .retrieve_slim_crawled_website(url)
            .await
            .map_err(CrawlWriteError::SlimError)?
        {
            match &found.stored_data_hint {
                StoredDataHint::External(_)
                | StoredDataHint::None
                | StoredDataHint::InMemory(_) => {
                    return Ok(Some(found.inflate(None)));
                }
                StoredDataHint::Warc(pointers) => {
                    let read = pointers.read_in_context(Some(&self.worker_warc_writer)).await?;
                    return Ok(Some(found.inflate(read)));
                }
                StoredDataHint::Associated => unreachable!(),
            }
        } else {
            Ok(None)
        }
    }
}

impl<T> SupportsCrawling for WorkerContext<T>
where
    T: SupportsCrawling,
{
    type Client = T::Client;
    type Error = T::Error;

    delegate::delegate! {
        to self.inner {
            fn create_crawl_task<S>(&self, seed: S) -> Result<CrawlTask<S, Self::Client>, Self::Error>
            where
                S: BasicSeed;

            fn create_crawl_id(&self) -> String;
        }
    }
}

#[cfg(test)]
pub mod test {
    use crate::config::Config;
    use crate::contexts::local::LocalContext;
    use crate::contexts::traits::{SupportsCrawlResults, SupportsSlimCrawlResults};
    use crate::contexts::worker::context::WorkerContext;
    use crate::crawl::test::{
        create_test_data, create_test_data_unknown, create_testdata_with_on_seed,
    };
    use crate::data::RawVecData;
    use crate::io::fs::{AtraFS, FileSystemAccess};
    use crate::runtime::RuntimeContext;
    use crate::stores::warc::ThreadsafeMultiFileWarcWriter;
    use crate::url::UrlWithDepth;
    use crate::warc_ext::SpecialWarcWriter;
    use camino::Utf8PathBuf;
    use encoding_rs::UTF_8;
    use std::net::{IpAddr, Ipv4Addr};
    use std::path::Path;
    use std::sync::Arc;
    use time::OffsetDateTime;
    use ubyte::ByteUnit;
    use warc::field::UriLikeFieldValue;
    use warc::header::WarcHeader;
    use warc::media_type::parse_media_type;
    use warc::record_type::WarcRecordType;
    use warc::truncated_reason::TruncatedReason;

    pub fn create_test_header(id_base: &str, content_length: u64) -> WarcHeader {
        fn create_uri_num(id_base: &str, ct: u64) -> UriLikeFieldValue {
            UriLikeFieldValue::new(format!("https://www.{id_base}.com/{ct}").parse().unwrap())
                .unwrap()
        }

        let mut data = WarcHeader::new();
        let mut uri_ct = 0;
        data.warc_record_id(create_uri_num(id_base, {
            let x = uri_ct;
            uri_ct += 1;
            x
        }))
        .unwrap();
        data.concurrent_to(create_uri_num(id_base, {
            let x = uri_ct;
            uri_ct += 1;
            x
        }))
        .unwrap();
        data.refers_to(create_uri_num(id_base, {
            let x = uri_ct;
            uri_ct += 1;
            x
        }))
        .unwrap();
        data.refers_to_target(create_uri_num(id_base, {
            let x = uri_ct;
            uri_ct += 1;
            x
        }))
        .unwrap();
        data.target_uri(create_uri_num(id_base, {
            let x = uri_ct;
            uri_ct += 1;
            x
        }))
        .unwrap();
        data.info_id(create_uri_num(id_base, {
            let x = uri_ct;
            uri_ct += 1;
            x
        }))
        .unwrap();
        data.profile(create_uri_num(id_base, {
            let x = uri_ct;
            uri_ct += 1;
            x
        }))
        .unwrap();
        data.segment_origin_id(create_uri_num(id_base, uri_ct))
            .unwrap();

        data.warc_type(WarcRecordType::Response).unwrap();

        data.atra_content_encoding(UTF_8).unwrap();

        data.date(OffsetDateTime::now_utc()).unwrap();
        data.referes_to_date(OffsetDateTime::now_utc()).unwrap();

        data.content_length(content_length).unwrap();
        data.segment_number(1234).unwrap();
        data.segment_total_length(12345).unwrap();

        data.content_type(
            parse_media_type::<true>(b"text/html;charset=UTF-8")
                .unwrap()
                .1,
        )
        .unwrap();
        data.indentified_payload_type(parse_media_type::<true>(b"text/xml").unwrap().1)
            .unwrap();

        data.truncated_reason(TruncatedReason::Length).unwrap();

        data.ip_address(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)))
            .unwrap();

        data.block_digest_string("sha1:bla").unwrap();
        data.payload_digest_string("sha1:bla").unwrap();

        data.file_name_string("lolwut.txt").unwrap();

        data
    }

    pub async fn create_writers() -> (FileSystemAccess, ThreadsafeMultiFileWarcWriter) {
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
        )
        .unwrap();

        let wwr = ThreadsafeMultiFileWarcWriter::new_for_worker(Arc::new(
            fs.create_worker_file_provider(0, 0).unwrap(),
        ))
        .unwrap();

        (fs, wwr)
    }

    #[tokio::test]
    async fn writer_test() {
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
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_context() {
        if Path::new("test").exists() {
            std::fs::remove_dir_all("test").unwrap();
        }

        let mut cfg = Config::default();
        cfg.paths.root = "test".parse().unwrap();

        let local = Arc::new(LocalContext::new(cfg, &RuntimeContext::unbound()).unwrap());

        let worker = WorkerContext::create(0, 0, local.clone()).unwrap();
        let test_data1 = create_testdata_with_on_seed(None);
        const BIG_DATA: [u8; ByteUnit::Gigabyte(1).as_u64() as usize - 20] =
            [b'a'; { ByteUnit::Gigabyte(1).as_u64() as usize - 20 }];

        let test_data2 = create_test_data(
            UrlWithDepth::from_url("https://www.oofsize.de/").unwrap(),
            Some(RawVecData::from_vec(BIG_DATA.to_vec())),
        );
        let test_data3 = create_test_data(
            UrlWithDepth::from_url("https://www.catsanddogs.de/").unwrap(),
            None,
        );
        worker.store_crawled_website(&test_data1).await.unwrap();
        worker.store_crawled_website(&test_data2).await.unwrap();
        worker.store_crawled_website(&test_data3).await.unwrap();

        let x = UrlWithDepth::from_url("https://www.oofsize.de/").unwrap();

        let found = worker.retrieve_slim_crawled_website(&x).await.unwrap();
        println!("{:?}", found);

        let retrieved = worker
            .retrieve_crawled_website(&test_data1.meta.url)
            .await
            .expect("This should work")
            .expect("Expected to exist!");
        println!(
            "->{}<-",
            String::from_utf8(test_data1.content.as_in_memory().unwrap().clone()).unwrap()
        );
        println!(
            "->{}<-",
            String::from_utf8(retrieved.content.as_in_memory().unwrap().clone()).unwrap()
        );
        assert_eq!(retrieved, test_data1);
    }

    #[tokio::test]
    async fn test_context2() {
        if Path::new("test").exists() {
            std::fs::remove_dir_all("test").unwrap();
        }

        let mut cfg = Config::default();
        cfg.paths.root = "test".parse().unwrap();

        let local = Arc::new(LocalContext::new(cfg, &RuntimeContext::unbound()).unwrap());

        let worker = WorkerContext::create(0, 0, local.clone()).unwrap();
        let test_data1 = create_testdata_with_on_seed(None);
        const BIG_DATA: [u8; ByteUnit::Gigabyte(1).as_u64() as usize - 20] =
            [b'a'; { ByteUnit::Gigabyte(1).as_u64() as usize - 20 }];

        let test_data2 = create_test_data_unknown(
            UrlWithDepth::from_url("https://www.oofsize.de/").unwrap(),
            RawVecData::from_vec(BIG_DATA.to_vec()),
        );
        let test_data3 = create_test_data(
            UrlWithDepth::from_url("https://www.catsanddogs.de/").unwrap(),
            None,
        );
        worker.store_crawled_website(&test_data1).await.unwrap();
        worker.store_crawled_website(&test_data2).await.unwrap();
        worker.store_crawled_website(&test_data3).await.unwrap();

        let x = UrlWithDepth::from_url("https://www.oofsize.de/").unwrap();

        let found = worker
            .retrieve_crawled_website(&x)
            .await
            .expect("This should work")
            .expect("Expected to exist!");
        assert_eq!(
            test_data2, found,
            "Failed to compare:\n\nA: {:?}\n\nB: {:?}",
            test_data2, found
        );

        let retrieved = worker
            .retrieve_crawled_website(&test_data1.meta.url)
            .await
            .expect("This should work")
            .expect("Expected to exist!");
        assert_eq!(test_data1, retrieved);
    }
}
