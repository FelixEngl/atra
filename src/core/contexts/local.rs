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

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use rocksdb::DB;
use time::{Duration, OffsetDateTime};
use tokio::sync::{RwLock};
use crate::core::blacklist::{PolyBlackList, BlacklistManager};
use crate::core::link_state::{LinkStateManager, LinkState, LinkStateDB, LinkStateDBError, LinkStateType};
use crate::core::config::configs::Configs;
use crate::core::contexts::{RecoveryCommand};
use crate::core::contexts::errors::{LinkHandlingError, RecoveryError};
use crate::core::crawl::db::{CrawlDB};
use crate::core::crawl::seed::CrawlSeed;
use crate::core::crawl::slim::{SlimCrawlResult};
use crate::core::database_error::DatabaseError;
use crate::core::origin::managers::InMemoryOriginManager;
use crate::core::extraction::ExtractedLink;
use crate::core::robots::{OffMemoryRobotsManager, ShareableRobotsManager};
use crate::core::rocksdb_ext::{open_db};
use crate::core::io::fs::FileSystemAccess;
use crate::core::origin::AtraOriginProvider;
use crate::core::web_graph::{WebGraphEntry, QueuingWebGraphManager, WebGraphManager};
use crate::core::queue::file::RawAgingQueueFile;
use crate::core::shutdown::UnsafeShutdownGuard;
use crate::core::url::queue::{UrlQueue, UrlQueueElement, UrlQueueWrapper};
use crate::core::UrlWithDepth;
use crate::util::RuntimeContext;


/// The state of the app
#[derive(Debug)]
pub struct LocalContext {
    started_at: OffsetDateTime,
    _db: Arc<DB>,
    file_provider: Arc<FileSystemAccess>,
    url_queue: UrlQueueWrapper<RawAgingQueueFile>,
    link_states: LinkStateDB,
    blacklist: BlacklistManager,
    robots: ShareableRobotsManager,
    crawled_data: CrawlDB,
    host_manager: InMemoryOriginManager,
    configs: Configs,
    links_net_manager: Arc<QueuingWebGraphManager>,
    // Internal states
    last_scan_over_link_states: RwLock<Option<(bool, OffsetDateTime)>>,
    ct_discovered_websites: AtomicUsize,
    _graceful_shutdown_guard: UnsafeShutdownGuard
}

impl LocalContext {

    /// Creates the state for Atra.
    pub async fn new(
        configs: Configs,
        runtime_context: RuntimeContext
    ) -> anyhow::Result<Self> {

        let output_path = configs.paths().root_path();
        if !output_path.exists() {
            std::fs::create_dir_all(output_path)?;
        }
        let file_provider = Arc::new(FileSystemAccess::new(
            configs.session.service.clone(),
            configs.session.collection.clone(),
            configs.session.crawl_job_id,
            configs.paths.root_path().to_path_buf(),
            configs.paths.dir_big_files()
        )?);

        let db = Arc::new(open_db(configs.paths().dir_database())?);
        let link_states = LinkStateDB::new(db.clone())?;
        let crawled_data = CrawlDB::new(db.clone(), &configs)?;
        let robots = OffMemoryRobotsManager::new(db.clone(), configs.system().robots_cache_size)?.into();
        let web_graph_manager = QueuingWebGraphManager::new(
            configs.system().web_graph_cache_size,
            configs.paths().file_web_graph(),
            &runtime_context
        )?;

        let url_queue = UrlQueueWrapper::open(configs.paths().file_queue())?;
        let blacklist = BlacklistManager::open(
            configs.paths().file_blacklist(),
            runtime_context.shutdown_guard().clone()
        )?;

        Ok(
            LocalContext {
                _db: db,
                url_queue,
                link_states,
                blacklist,
                file_provider,
                crawled_data,
                robots,
                configs,
                host_manager: InMemoryOriginManager::default(),
                started_at: OffsetDateTime::now_utc(),
                last_scan_over_link_states: RwLock::new(None),
                ct_discovered_websites: AtomicUsize::new(0),
                links_net_manager: Arc::new(web_graph_manager),
                _graceful_shutdown_guard: runtime_context.shutdown_guard().clone()
            }
        )
    }


    #[allow(dead_code)]
    pub fn crawl_db(&self) -> &CrawlDB {
        &self.crawled_data
    }
}



impl super::Context for LocalContext {
    type RobotsManager = ShareableRobotsManager;
    type UrlQueue = UrlQueueWrapper<RawAgingQueueFile>;
    type HostManager = InMemoryOriginManager;
    type WebGraphManager = QueuingWebGraphManager;

    async fn can_poll(&self) -> bool {
        !self.url_queue.is_empty().await
    }

    fn fs(&self) -> &FileSystemAccess {
        &self.file_provider
    }

    fn crawled_websites(&self) -> Result<u64, LinkStateDBError> {
        self.link_states.count_state(LinkStateType::ProcessedAndStored)
    }

    fn discovered_websites(&self) -> usize {
        self.ct_discovered_websites.load(Ordering::Relaxed)
    }

    fn url_queue(&self) -> &Self::UrlQueue {
        &self.url_queue
    }

    fn configs(&self) -> &Configs {
        &self.configs
    }

    fn crawl_started_at(&self) -> OffsetDateTime {
        self.started_at
    }

    fn web_graph_manager(&self) -> &Self::WebGraphManager {
        &self.links_net_manager
    }

    async fn get_blacklist(&self) -> PolyBlackList {
        self.blacklist.create_current_blacklist().await.unwrap_or_default()
    }

    async fn get_robots_instance(&self) -> ShareableRobotsManager {
        self.robots.clone()
    }

    fn get_host_manager(&self) -> &Self::HostManager {
        &self.host_manager
    }

    async fn retrieve_slim_crawled_website(&self, url: &UrlWithDepth) -> Result<Option<SlimCrawlResult>, DatabaseError> {
        match self.crawled_data.get(url) {
            Err(DatabaseError::RecoverableFailure{..}) => self.crawled_data.get(url),
            pipe => pipe
        }
    }

    async fn register_seed(&self, seed: &impl CrawlSeed) -> Result<(), LinkHandlingError> {
        self.links_net_manager.add(WebGraphEntry::create_seed(seed)).await?;
        Ok(())
    }

    async fn handle_links(&self, from: &UrlWithDepth, links: &HashSet<ExtractedLink>) -> Result<Vec<UrlWithDepth>, LinkHandlingError> {
        let mut for_queue = Vec::with_capacity(links.len() / 2);
        let mut for_insert = Vec::with_capacity(links.len() / 2);
        for link in links {
            match link {
                ExtractedLink::OnSeed{url,..} => {
                    self.links_net_manager.add(WebGraphEntry::create_link(from, url)).await?;
                    for_insert.push(url.clone());
                }
                ExtractedLink::Outgoing{url,..} => {
                    self.links_net_manager.add(WebGraphEntry::create_link(from, url)).await?;
                    if self.get_link_state(url).await?.is_none() {
                        self.update_link_state(url, LinkStateType::Discovered).await?;
                        if let Some(origin) = url.atra_origin() {
                            if self.configs.crawl().budget.get_budget_for(&origin).is_in_budget(url) {
                                for_queue.push(UrlQueueElement::new(false, 0, false, url.clone()));
                            }
                        }
                    }
                }
                ExtractedLink::Data{ .. } => {
                    // todo data-urls: How to handle?
                    log::warn!("data-urls are at the moment unsupported.")
                }
            }
        }
        self.ct_discovered_websites.fetch_add(for_queue.len() + for_insert.len(), Ordering::Relaxed);
        if !for_queue.is_empty() {
            self.url_queue.enqueue_all(for_queue).await?;
        }
        Ok(for_insert)
    }

    /// Sets the state of the link
    async fn update_link_state(&self, url: &UrlWithDepth, state: LinkStateType) -> Result<(), LinkStateDBError> {
        match self.link_states.update_state(url, state) {
            Err(LinkStateDBError::Database(DatabaseError::RecoverableFailure{..})) => {
                self.link_states.update_state(url, state)
            }
            escalate => escalate
        }
    }

    /// Sets the state of the link with a payload
    async fn update_link_state_with_payload(&self, url: &UrlWithDepth, state: LinkStateType, payload: Vec<u8>) -> Result<(), LinkStateDBError> {
        let linkstate = state.into_update(
            url,
            Some(payload)
        );
        match self.link_states.upsert_state(url, &linkstate) {
            Err(LinkStateDBError::Database(DatabaseError::RecoverableFailure{..})) => {
                self.link_states.upsert_state(url, &linkstate)
            }
            escalate => escalate
        }
    }

    /// Gets the state of the current url
    async fn get_link_state(&self, url: &UrlWithDepth) -> Result<Option<LinkState>, LinkStateDBError> {
        match self.link_states.get_state(url) {
            Err(LinkStateDBError::Database(DatabaseError::RecoverableFailure{..})) => {
                self.link_states.get_state(url)
            }
            escalate => escalate
        }
    }

    async fn check_if_there_are_any_crawlable_links(&self, max_age: Duration) -> bool {
        let lock = self.last_scan_over_link_states.read().await;
        if let Some(value) = lock.as_ref() {
            if OffsetDateTime::now_utc() - value.1 <= max_age {
                return value.0
            }
        }
        drop(lock);
        let mut lock = self.last_scan_over_link_states.write().await;
        if let Some(value) = lock.as_ref() {
            if OffsetDateTime::now_utc() - value.1 <= max_age {
                return value.0
            }
        }
        let found = self.link_states.scan_for_any_link_state(LinkStateType::Discovered..=LinkStateType::Crawled).await;
        lock.replace((found, OffsetDateTime::now_utc()));
        found
    }

    async fn recover<'a>(&self, recovery_command: RecoveryCommand<'a>) -> Result<(), RecoveryError> {
        match recovery_command {
            RecoveryCommand::All => {todo!("Not supported in this version.")}
            RecoveryCommand::UpdateLinkState(_, _) => {
                return Err(RecoveryError::UnknownReason)
            }
        }
    }
}

impl super::SlimCrawlTaskContext for LocalContext {
    async fn store_slim_crawled_website(&self, slim: SlimCrawlResult) -> Result<(), DatabaseError> {
        match self.crawled_data.add(&slim) {
            Err(DatabaseError::RecoverableFailure{..}) => self.crawled_data.add(&slim),
            pipe => pipe
        }
    }
}

unsafe impl Send for LocalContext {}
unsafe impl Sync for LocalContext {}
