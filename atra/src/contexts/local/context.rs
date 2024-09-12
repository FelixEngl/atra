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
use liblinear::solver::L2R_L2LOSS_SVR;
use rocksdb::DB;
use time::{OffsetDateTime};
use tokio::sync::{RwLock};
use crate::blacklist::{PolyBlackList, BlacklistManager};
use crate::link_state::{LinkStateManager, LinkState, LinkStateDB, LinkStateDBError, LinkStateType};
use crate::config::configs::Configs;
use crate::contexts::traits::*;
use crate::crawl::db::{CrawlDB};
use crate::seed::BasicSeed;
use crate::crawl::{SlimCrawlResult};
use crate::database::DatabaseError;
use crate::extraction::ExtractedLink;
use crate::robots::{OffMemoryRobotsManager, ShareableRobotsManager};
use crate::database::{open_db};
use crate::io::fs::FileSystemAccess;
use crate::web_graph::{WebGraphEntry, QueuingWebGraphManager, WebGraphManager};
use crate::queue::RawAgingQueueFile;
use crate::runtime::UnsafeShutdownGuard;
use crate::url::queue::{UrlQueue, UrlQueueElement, UrlQueueWrapper};
use crate::url::{AtraOriginProvider, UrlWithDepth};
use text_processing::tf_idf::{Idf, Tf};
use text_processing::stopword_registry::StopWordRegistry;
use crate::contexts::BaseContext;
use crate::contexts::local::errors::LinkHandlingError;
use crate::gdbr::identifier::{GdbrIdentifierRegistry, InitHelper};
use crate::runtime::RuntimeContext;
use crate::url::guard::InMemoryUrlGuardian;

/// The state of the app
#[derive(Debug)]
#[allow(dead_code)]
pub struct LocalContext {
    started_at: OffsetDateTime,
    _db: Arc<DB>,
    file_provider: Arc<FileSystemAccess>,
    url_queue: UrlQueueWrapper<RawAgingQueueFile>,
    link_states: LinkStateDB,
    blacklist: BlacklistManager,
    robots: ShareableRobotsManager,
    crawled_data: CrawlDB,
    host_manager: InMemoryUrlGuardian,
    configs: Configs,
    links_net_manager: Arc<QueuingWebGraphManager>,
    // Internal states
    last_scan_over_link_states: RwLock<Option<(bool, OffsetDateTime)>>,
    ct_discovered_websites: AtomicUsize,
    stop_word_registry: Option<StopWordRegistry>,
    gdbr_filer_registry: Option<GdbrIdentifierRegistry<Tf, Idf, L2R_L2LOSS_SVR>>,
    _graceful_shutdown_guard: UnsafeShutdownGuard,
}

impl LocalContext {
    /// Creates the state for Atra.
    pub async fn new(
        configs: Configs,
        runtime_context: RuntimeContext,
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
            configs.paths.dir_big_files(),
        )?);

        let db = Arc::new(open_db(configs.paths().dir_database())?);
        let link_states = LinkStateDB::new(db.clone())?;
        let crawled_data = CrawlDB::new(db.clone(), &configs)?;
        let robots = OffMemoryRobotsManager::new(db.clone(), configs.system().robots_cache_size)?.into();
        let web_graph_manager = QueuingWebGraphManager::new(
            configs.system().web_graph_cache_size,
            configs.paths().file_web_graph(),
            &runtime_context,
        )?;


        let stop_word_registry = configs.crawl.stopword_registry.as_ref().map(StopWordRegistry::initialize).transpose()?;

        let url_queue = UrlQueueWrapper::open(configs.paths().file_queue())?;
        let blacklist = BlacklistManager::open(
            configs.paths().file_blacklist(),
            runtime_context.shutdown_guard().clone(),
        )?;

        let gdbr_filer_registry = if let Some(ref cfg) = configs.crawl.gbdr {
            let helper = InitHelper {
                gdbr_config: Some(cfg),
                root: Some(&configs.paths.root_path()),
                stop_word_registry: stop_word_registry.as_ref(),
            };
            GdbrIdentifierRegistry::new_from_config(&helper)?
        } else {
            None
        };

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
                host_manager: InMemoryUrlGuardian::default(),
                started_at: OffsetDateTime::now_utc(),
                last_scan_over_link_states: RwLock::new(None),
                ct_discovered_websites: AtomicUsize::new(0),
                links_net_manager: Arc::new(web_graph_manager),
                stop_word_registry,
                gdbr_filer_registry,
                _graceful_shutdown_guard: runtime_context.shutdown_guard().clone(),
            }
        )
    }


    #[allow(dead_code)]
    pub fn crawl_db(&self) -> &CrawlDB {
        &self.crawled_data
    }
}

impl BaseContext for LocalContext {}

impl SupportsStopwordsRegistry for LocalContext {
    fn stopword_registry(&self) -> Option<&StopWordRegistry> {
        self.stop_word_registry.as_ref()
    }
}
impl AsyncContext for LocalContext {}

impl SupportsLinkSeeding for LocalContext {
    type Error = LinkHandlingError;

    async fn register_seed<S: BasicSeed>(&self, seed: &S) -> Result<(), LinkHandlingError> {
        self.links_net_manager.add(WebGraphEntry::create_seed(seed)).await?;
        Ok(())
    }

    async fn handle_links(&self, from: &UrlWithDepth, links: &HashSet<ExtractedLink>) -> Result<Vec<UrlWithDepth>, LinkHandlingError> {
        let mut for_queue = Vec::with_capacity(links.len() / 2);
        let mut for_insert = Vec::with_capacity(links.len() / 2);
        for link in links {
            match link {
                ExtractedLink::OnSeed { url, .. } => {
                    self.links_net_manager.add(WebGraphEntry::create_link(from, url)).await?;
                    for_insert.push(url.clone());
                }
                ExtractedLink::Outgoing { url, .. } => {
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
                ExtractedLink::Data { .. } => {
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
}

impl SupportsLinkState for LocalContext {
    type Error = LinkStateDBError;

    fn crawled_websites(&self) -> Result<u64, LinkStateDBError> {
        self.link_states.count_state(LinkStateType::ProcessedAndStored)
    }

    /// Sets the state of the link
    async fn update_link_state(&self, url: &UrlWithDepth, state: LinkStateType) -> Result<(), LinkStateDBError> {
        match self.link_states.update_state(url, state) {
            Err(LinkStateDBError::Database(DatabaseError::RecoverableFailure { .. })) => {
                self.link_states.update_state(url, state)
            }
            escalate => escalate
        }
    }


    /// Sets the state of the link with a payload
    async fn update_link_state_with_payload(&self, url: &UrlWithDepth, state: LinkStateType, payload: Vec<u8>) -> Result<(), LinkStateDBError> {
        let linkstate = state.into_update(
            url,
            Some(payload),
        );
        match self.link_states.upsert_state(url, &linkstate) {
            Err(LinkStateDBError::Database(DatabaseError::RecoverableFailure { .. })) => {
                self.link_states.upsert_state(url, &linkstate)
            }
            escalate => escalate
        }
    }

    /// Gets the state of the current url
    async fn get_link_state(&self, url: &UrlWithDepth) -> Result<Option<LinkState>, LinkStateDBError> {
        match self.link_states.get_state(url) {
            Err(LinkStateDBError::Database(DatabaseError::RecoverableFailure { .. })) => {
                self.link_states.get_state(url)
            }
            escalate => escalate
        }
    }

    async fn check_if_there_are_any_crawlable_links(&self, max_age: std::time::Duration) -> bool {
        let lock = self.last_scan_over_link_states.read().await;
        if let Some(value) = lock.as_ref() {
            if OffsetDateTime::now_utc() - value.1 <= max_age {
                return value.0;
            }
        }
        drop(lock);
        let mut lock = self.last_scan_over_link_states.write().await;
        if let Some(value) = lock.as_ref() {
            if OffsetDateTime::now_utc() - value.1 <= max_age {
                return value.0;
            }
        }
        let found = self.link_states.scan_for_any_link_state(LinkStateType::Discovered..=LinkStateType::Crawled).await;
        lock.replace((found, OffsetDateTime::now_utc()));
        found
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
    fn configs(&self) -> &Configs {
        &self.configs
    }
}
impl SupportsWebGraph for LocalContext {
    type WebGraphManager = QueuingWebGraphManager;

    fn web_graph_manager(&self) -> &Self::WebGraphManager {
        &self.links_net_manager
    }
}
impl SupportsBlackList for LocalContext {
    type BlackList = PolyBlackList;

    async fn get_blacklist(&self) -> PolyBlackList {
        self.blacklist.create_current_blacklist().await.unwrap_or_default()
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
    type RobotsManager = ShareableRobotsManager;

    async fn get_robots_instance(&self) -> ShareableRobotsManager {
        self.robots.clone()
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

    async fn retrieve_slim_crawled_website(&self, url: &UrlWithDepth) -> Result<Option<SlimCrawlResult>, DatabaseError> {
        match self.crawled_data.get(url) {
            Err(DatabaseError::RecoverableFailure { .. }) => self.crawled_data.get(url),
            pipe => pipe
        }
    }

    async fn store_slim_crawled_website(&self, slim: SlimCrawlResult) -> Result<(), DatabaseError> {
        match self.crawled_data.add(&slim) {
            Err(DatabaseError::RecoverableFailure { .. }) => self.crawled_data.add(&slim),
            pipe => pipe
        }
    }
}

unsafe impl Send for LocalContext {}
unsafe impl Sync for LocalContext {}

