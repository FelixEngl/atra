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

use crate::blacklist::{InMemoryBlacklistManager, PolyBlackList};
use crate::client::{build_classic_client, ClientWithUserAgent};
use crate::config::configs::Config;
use crate::contexts::local::errors::LinkHandlingError;
use crate::contexts::local::LocalContextInitError;
use crate::contexts::traits::*;
use crate::contexts::BaseContext;
use crate::crawl::db::CrawlDB;
use crate::crawl::{CrawlTask, SlimCrawlResult};
use crate::database::open_db;
use crate::database::DatabaseError;
use crate::extraction::ExtractedLink;
use crate::gdbr::identifier::{GdbrIdentifierRegistry, InitHelper};
use crate::io::fs::FileSystemAccess;
use crate::link_state::{
    DatabaseLinkStateManager, IsSeedYesNo, LinkStateKind, LinkStateManager, LinkStateRockDB,
    RecrawlYesNo,
};
use crate::queue::{RawAgingQueueFile, UrlQueue, UrlQueueElement, UrlQueueWrapper};
use crate::recrawl_management::DomainLastCrawledDatabaseManager;
use crate::robots::OffMemoryRobotsManager;
use crate::runtime::{GracefulShutdownGuard, GracefulShutdownWithGuard, RuntimeContext};
use crate::seed::BasicSeed;
use crate::url::guard::InMemoryUrlGuardian;
use crate::url::{AtraOriginProvider, UrlWithDepth};
use crate::web_graph::{QueuingWebGraphManager, WebGraphEntry, WebGraphManager};
use liblinear::solver::L2R_L2LOSS_SVR;
use rand::distributions::Alphanumeric;
use rand::Rng;
use rocksdb::DB;
use std::collections::HashSet;
use std::fs::File;
use std::io::BufWriter;
use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use text_processing::stopword_registry::StopWordRegistry;
use text_processing::tf_idf::{Idf, Tf};
use time::OffsetDateTime;

/// The state of the app
#[derive(Debug)]
pub struct LocalContext {
    started_at: OffsetDateTime,
    _db: Arc<DB>,
    file_provider: Arc<FileSystemAccess>,
    url_queue: UrlQueueWrapper<RawAgingQueueFile>,
    link_state_manager: DatabaseLinkStateManager<LinkStateRockDB>,
    blacklist: InMemoryBlacklistManager<PolyBlackList>,
    robots: OffMemoryRobotsManager,
    crawled_data: CrawlDB,
    host_manager: InMemoryUrlGuardian,
    configs: Config,
    web_graph_manager: Option<Arc<QueuingWebGraphManager>>,
    ct_discovered_websites: AtomicUsize,
    stop_word_registry: Option<StopWordRegistry>,
    gdbr_filer_registry: Option<GdbrIdentifierRegistry<Tf, Idf, L2R_L2LOSS_SVR>>,
    domain_manager: DomainLastCrawledDatabaseManager,
    _guard: GracefulShutdownGuard,
}

impl LocalContext {
    pub fn new_without_runtime(config: Config) -> Result<Self, LocalContextInitError> {
        let other = RuntimeContext::new(GracefulShutdownWithGuard::new(), None);
        Self::new(config, &other)
    }

    /// Creates the state for Atra.
    pub fn new(
        configs: Config,
        runtime_context: &RuntimeContext,
    ) -> Result<Self, LocalContextInitError> {
        let output_path = configs.paths.root_path();
        if !output_path.exists() {
            std::fs::create_dir_all(output_path)?;
        }

        serde_json::to_writer_pretty(
            BufWriter::new(
                File::options()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(output_path.join("config.json"))?,
            ),
            &configs,
        )?;

        log::info!("Init file system.");
        let file_provider = Arc::new(FileSystemAccess::new(
            configs.session.service.clone(),
            configs.session.collection.clone(),
            configs.session.crawl_job_id,
            configs.paths.root_path().to_path_buf(),
            configs.paths.dir_big_files(),
        )?);

        log::info!("Init internal database.");
        let db = Arc::new(open_db(configs.paths.dir_database())?);

        log::info!("Init link states database.");
        let link_state_manager = DatabaseLinkStateManager::new(db.clone());
        log::info!("Init crawled information database.");
        let crawled_data = CrawlDB::new(db.clone(), &configs)?;
        log::info!("Init robots manager.");
        let robots = OffMemoryRobotsManager::new(db.clone(), configs.system.robots_cache_size);
        log::info!("Init web graph writer.");

        let web_graph_manager = configs
            .crawl
            .generate_web_graph
            .then(|| {
                QueuingWebGraphManager::new(
                    configs.system.web_graph_cache_size,
                    configs.paths.file_web_graph(),
                    &runtime_context,
                )
                .map(Arc::new)
            })
            .transpose()?;

        log::info!("Init stopword registry.");
        let stop_word_registry = configs
            .crawl
            .stopword_registry
            .as_ref()
            .map(StopWordRegistry::initialize)
            .transpose()?;
        log::info!("Init url queue.");
        let url_queue = UrlQueueWrapper::open(configs.paths.file_queue())?;
        log::info!("Init blacklist manager.");
        let blacklist = InMemoryBlacklistManager::open(
            configs.paths.file_blacklist(),
            runtime_context.shutdown_guard().clone(),
        )?;

        let gdbr_filer_registry = if let Some(ref cfg) = configs.crawl.gbdr {
            let helper = InitHelper {
                gdbr_config: Some(cfg),
                stop_word_registry: stop_word_registry.as_ref(),
            };
            log::info!("Init gdbr identifier.");
            GdbrIdentifierRegistry::new_from_config(&helper)?
        } else {
            log::info!("No gdbr identifier initialized.");
            None
        };

        let domain_manager = DomainLastCrawledDatabaseManager::new(db.clone());

        Ok(LocalContext {
            _db: db,
            url_queue,
            link_state_manager,
            blacklist,
            file_provider,
            crawled_data,
            robots,
            configs,
            host_manager: InMemoryUrlGuardian::default(),
            started_at: OffsetDateTime::now_utc(),
            ct_discovered_websites: AtomicUsize::new(0),
            web_graph_manager,
            stop_word_registry,
            gdbr_filer_registry,
            domain_manager,
            _guard: runtime_context.shutdown_guard().guard(),
        })
    }

    pub fn crawl_db(&self) -> &CrawlDB {
        &self.crawled_data
    }
}

unsafe impl Send for LocalContext {}
unsafe impl Sync for LocalContext {}

impl BaseContext for LocalContext {}

impl SupportsStopwordsRegistry for LocalContext {
    fn stopword_registry(&self) -> Option<&StopWordRegistry> {
        self.stop_word_registry.as_ref()
    }
}
impl AsyncContext for LocalContext {}

impl SupportsDomainHandling for LocalContext {
    type DomainHandler = DomainLastCrawledDatabaseManager;

    fn get_domain_manager(&self) -> &Self::DomainHandler {
        &self.domain_manager
    }
}

impl SupportsLinkSeeding for LocalContext {
    type Error = LinkHandlingError;

    async fn register_seed<S: BasicSeed>(&self, seed: &S) -> Result<(), LinkHandlingError> {
        if let Some(ref manager) = self.web_graph_manager {
            manager.add(WebGraphEntry::create_seed(seed)).await?;
        }
        Ok(())
    }

    async fn handle_links(
        &self,
        from: &UrlWithDepth,
        links: &HashSet<ExtractedLink>,
    ) -> Result<Vec<UrlWithDepth>, LinkHandlingError> {
        let mut for_queue = Vec::with_capacity(links.len() / 2);
        let mut for_insert = Vec::with_capacity(links.len() / 2);
        for link in links {
            match link {
                ExtractedLink::OnSeed { url, .. } => {
                    if let Some(ref manager) = self.web_graph_manager {
                        manager.add(WebGraphEntry::create_link(from, url)).await?;
                    }
                    for_insert.push(url.clone());
                }
                ExtractedLink::Outgoing { url, .. } => {
                    if let Some(ref manager) = self.web_graph_manager {
                        manager.add(WebGraphEntry::create_link(from, url)).await?;
                    }
                    if self.link_state_manager.get_link_state(url).await?.is_none() {
                        let recrawl: Option<RecrawlYesNo> = if let Some(origin) = url.atra_origin()
                        {
                            let budget = self.configs.crawl.budget.get_budget_for(&origin);
                            if budget.is_in_budget(url) {
                                for_queue.push(UrlQueueElement::new(false, 0, false, url.clone()));
                            }
                            Some(budget.get_recrawl_interval().is_some().into())
                        } else {
                            None
                        };

                        self.link_state_manager
                            .update_link_state_no_payload(
                                url,
                                LinkStateKind::Discovered,
                                Some(IsSeedYesNo::No),
                                recrawl,
                            )
                            .await?;
                    }
                }
                ExtractedLink::Data { .. } => {
                    // let parsed = data_url::DataUrl::process(&url.url.as_str())?;
                    //
                    // /// TODO: this is expensive. But mime does not provide a better API
                    // let mime_type = MimeType::new_single(parsed.mime_type().to_string().parse()?);

                    // todo data-urls: How to handle?
                    log::warn!("data-urls are at the moment unsupported.")
                }
            }
        }
        self.ct_discovered_websites
            .fetch_add(for_queue.len() + for_insert.len(), Ordering::Relaxed);
        if !for_queue.is_empty() {
            self.url_queue.enqueue_all(for_queue).await?;
        }
        Ok(for_insert)
    }
}

impl SupportsLinkState for LocalContext {
    type LinkStateManager = DatabaseLinkStateManager<LinkStateRockDB>;

    #[inline]
    fn get_link_state_manager(&self) -> &Self::LinkStateManager {
        &self.link_state_manager
    }
}
impl SupportsUrlGuarding for LocalContext {
    type Guardian = InMemoryUrlGuardian;

    fn get_guardian(&self) -> &Self::Guardian {
        &self.host_manager
    }
}
impl SupportsMetaInfo for LocalContext {
    fn crawl_started_at(&self) -> OffsetDateTime {
        self.started_at
    }

    fn discovered_websites(&self) -> usize {
        self.ct_discovered_websites.load(Ordering::Relaxed)
    }
}
impl SupportsConfigs for LocalContext {
    fn configs(&self) -> &Config {
        &self.configs
    }
}
impl SupportsWebGraph for LocalContext {
    type WebGraphManager = QueuingWebGraphManager;

    fn web_graph_manager(&self) -> Option<&Self::WebGraphManager> {
        if let Some(ref value) = self.web_graph_manager {
            Some(value.deref())
        } else {
            None
        }
    }
}
impl SupportsBlackList for LocalContext {
    type BlacklistManager = InMemoryBlacklistManager<PolyBlackList>;

    fn get_blacklist_manager(&self) -> &Self::BlacklistManager {
        &self.blacklist
    }
}
impl SupportsUrlQueue for LocalContext {
    type UrlQueue = UrlQueueWrapper<RawAgingQueueFile>;

    async fn can_poll(&self) -> bool {
        !self.url_queue.is_empty().await
    }

    fn url_queue(&self) -> &Self::UrlQueue {
        &self.url_queue
    }
}

impl SupportsGdbrRegistry for LocalContext {
    type Registry = GdbrIdentifierRegistry<Tf, Idf, L2R_L2LOSS_SVR>;

    fn gdbr_registry(&self) -> Option<&Self::Registry> {
        self.gdbr_filer_registry.as_ref()
    }
}

impl SupportsRobotsManager for LocalContext {
    type RobotsManager = OffMemoryRobotsManager;

    fn get_robots_manager(&self) -> &OffMemoryRobotsManager {
        &self.robots
    }
}

impl SupportsFileSystemAccess for LocalContext {
    type FileSystem = FileSystemAccess;
    fn fs(&self) -> &FileSystemAccess {
        &self.file_provider
    }
}

impl SupportsSlimCrawlResults for LocalContext {
    type Error = DatabaseError;

    async fn retrieve_slim_crawled_website(
        &self,
        url: &UrlWithDepth,
    ) -> Result<Option<SlimCrawlResult>, DatabaseError> {
        match self.crawled_data.get(url) {
            Err(DatabaseError::RecoverableFailure { .. }) => self.crawled_data.get(url),
            pipe => pipe,
        }
    }

    async fn store_slim_crawled_website(&self, slim: SlimCrawlResult) -> Result<(), DatabaseError> {
        match self.crawled_data.add(&slim) {
            Err(DatabaseError::RecoverableFailure { .. }) => self.crawled_data.add(&slim),
            pipe => pipe,
        }
    }
}

impl SupportsCrawling for LocalContext {
    type Client = ClientWithUserAgent;
    type Error = reqwest::Error;

    fn create_crawl_task<S>(&self, seed: S) -> Result<CrawlTask<S, Self::Client>, Self::Error>
    where
        S: BasicSeed,
    {
        let useragent = self.configs.crawl.user_agent.get_user_agent().to_string();
        let client = build_classic_client(self, &seed, &useragent)?;
        let client = ClientWithUserAgent::new(useragent, client);
        Ok(CrawlTask::new(seed, client))
    }

    fn create_crawl_id(&self) -> String {
        let mut result: String = "crawl".to_string();
        result.reserve(15 + 2 + 22);
        result.push('-');
        result.push_str(
            &data_encoding::BASE64URL_NOPAD.encode(
                &OffsetDateTime::now_utc()
                    .unix_timestamp_nanos()
                    .to_be_bytes(),
            ),
        );
        result.push('-');
        result.push_str(
            &rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(15)
                .map(char::from)
                .collect::<String>(),
        );
        result
    }
}

#[cfg(test)]
mod test {
    use data_encoding::BASE64URL_NOPAD;

    #[test]
    fn read() {
        println!("{}", BASE64URL_NOPAD.encode(&i128::MIN.to_be_bytes()))
    }
}
