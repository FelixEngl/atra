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

use crate::blacklist::{
    create_managed_blacklist, Blacklist, BlacklistError, BlacklistManager, BlacklistType,
    ManagedBlacklist, ManagedBlacklistSender, PolyBlackList, RegexBlackList,
};
use crate::client::{build_classic_client, ClientWithUserAgent};
use crate::config::Configs;
use crate::contexts::local::LinkHandlingError;
use crate::contexts::traits::*;
use crate::contexts::{BaseContext, Context};
use crate::crawl::{CrawlResult, CrawlTask, SlimCrawlResult, StoredDataHint};
use crate::data::RawVecData;
use crate::database::DatabaseError;
use crate::extraction::ExtractedLink;
use crate::gdbr::identifier::GdbrIdentifierRegistry;
use crate::io::fs::FileSystemAccess;
use crate::link_state::{LinkState, LinkStateDBError, LinkStateKind, LinkStateManager};
use crate::queue::{PollWaiter, PollWaiterFactory, QueueError};
use crate::queue::{EnqueueCalled, UrlQueue, UrlQueueElement};
use crate::robots::InMemoryRobotsManager;
use crate::seed::BasicSeed;
use crate::url::guard::InMemoryUrlGuardian;
use crate::url::UrlWithDepth;
use crate::url::{AtraOriginProvider, AtraUri};
use crate::web_graph::{LinkNetError, WebGraphEntry, WebGraphManager};
use indexmap::IndexSet;
use itertools::Itertools;
use liblinear::solver::L2R_L2LOSS_SVR;
use rand::distributions::Alphanumeric;
use rand::Rng;
use std::cmp::min;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use text_processing::stopword_registry::StopWordRegistry;
use text_processing::tf_idf::{Idf, Tf};
use time::OffsetDateTime;
use tokio::sync::broadcast::Receiver;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct TestContext {
    ct_crawled_websites: AtomicUsize,
    ct_found_websites: AtomicUsize,
    link_state_manager: InMemoryLinkStateManager,
    robots_manager: InMemoryRobotsManager,
    blacklist_manager: TestBlacklistManager,
    crawled_websites: tokio::sync::RwLock<HashMap<AtraUri, SlimCrawlResult>>,
    data_urls: Mutex<Vec<(UrlWithDepth, UrlWithDepth)>>,
    configs: Configs,
    host_manager: InMemoryUrlGuardian,
    started_at: OffsetDateTime,
    links_queue: TestUrlQueue,
    link_net_manager: TestLinkNetManager,
    stop_word_registry: StopWordRegistry,
    gdbr_registry: Option<GdbrIdentifierRegistry<Tf, Idf, L2R_L2LOSS_SVR>>,
}

impl TestContext {
    pub fn new(configs: Configs) -> Self {
        Self {
            ct_crawled_websites: AtomicUsize::new(0),
            ct_found_websites: AtomicUsize::new(0),
            robots_manager: InMemoryRobotsManager::new(),
            blacklist_manager: TestBlacklistManager::new(Default::default()),
            crawled_websites: tokio::sync::RwLock::new(HashMap::new()),
            link_state_manager: InMemoryLinkStateManager::new(),
            links_queue: TestUrlQueue::default(),
            data_urls: Default::default(),
            stop_word_registry: StopWordRegistry::default(),
            configs,
            host_manager: Default::default(),
            started_at: OffsetDateTime::now_utc(),
            link_net_manager: TestLinkNetManager::default(),
            gdbr_registry: None,
        }
    }

    pub fn with_blacklist(configs: Configs, blacklist: Option<Vec<String>>) -> Self {
        Self {
            blacklist_manager: TestBlacklistManager::new(blacklist),
            ..Self::new(configs)
        }
    }

    pub fn get_all_crawled_websites(
        self,
    ) -> (HashMap<AtraUri, CrawlResult>, HashMap<AtraUri, LinkState>) {
        let data = self
            .crawled_websites
            .into_inner()
            .into_iter()
            .map(|value| (value.0, value.1.inflate(None)))
            .collect();
        let found = self.link_state_manager.state.into_inner().unwrap();
        (data, found)
    }
}

impl Default for TestContext {
    fn default() -> Self {
        Self::new(Configs::default())
    }
}

impl BaseContext for TestContext {}

impl AsyncContext for TestContext {}

impl SupportsWorkerId for TestContext {
    fn worker_id(&self) -> usize {
        0
    }
}

impl SupportsCrawling for TestContext {
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

impl Context for TestContext {}

impl SupportsLinkSeeding for TestContext {
    type Error = LinkHandlingError;

    async fn register_seed<S: BasicSeed>(&self, seed: &S) -> Result<(), LinkHandlingError> {
        self.link_net_manager
            .add(WebGraphEntry::create_seed(seed))
            .await?;
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
            self.ct_found_websites.fetch_add(1, Ordering::Relaxed);
            match link {
                ExtractedLink::OnSeed { url, .. } => {
                    self.link_net_manager
                        .add(WebGraphEntry::create_link(from, url))
                        .await
                        .unwrap();
                    for_insert.push(url.clone());
                }
                ExtractedLink::Outgoing { url, .. } => {
                    self.link_net_manager
                        .add(WebGraphEntry::create_link(from, url))
                        .await
                        .unwrap();
                    if self.link_state_manager.get_link_state(url).await?.is_none() {
                        self.link_state_manager
                            .update_link_state(url, LinkStateKind::Discovered)
                            .await?;
                        if let Some(origin) = url.atra_origin() {
                            if self
                                .configs
                                .crawl
                                .budget
                                .get_budget_for(&origin)
                                .is_in_budget(url)
                            {
                                for_queue.push(UrlQueueElement::new(false, 0, false, url.clone()));
                            }
                        }
                    }
                }
                ExtractedLink::Data { base, url, .. } => self
                    .data_urls
                    .lock()
                    .await
                    .push((base.clone(), url.clone())),
            }
        }
        if !for_queue.is_empty() {
            self.links_queue.enqueue_all(for_queue).await?;
        }
        Ok(for_insert)
    }
}

impl SupportsLinkState for TestContext {
    type LinkStateManager = InMemoryLinkStateManager;
    fn get_link_state_manager(&self) -> &Self::LinkStateManager {
        &self.link_state_manager
    }
}

impl SupportsUrlGuarding for TestContext {
    type Guardian = InMemoryUrlGuardian;

    fn get_guardian(&self) -> &InMemoryUrlGuardian {
        &self.host_manager
    }
}

impl SupportsRobotsManager for TestContext {
    type RobotsManager = InMemoryRobotsManager;

    fn get_robots_manager(&self) -> &Self::RobotsManager {
        &self.robots_manager
    }
}

impl SupportsBlackList for TestContext {
    type BlacklistManager = TestBlacklistManager;
    fn get_blacklist_manager(&self) -> &Self::BlacklistManager {
        &self.blacklist_manager
    }
}

impl SupportsMetaInfo for TestContext {
    fn crawl_started_at(&self) -> OffsetDateTime {
        self.started_at
    }

    fn discovered_websites(&self) -> usize {
        self.ct_found_websites.load(Ordering::Relaxed)
    }
}

impl SupportsConfigs for TestContext {
    fn configs(&self) -> &Configs {
        &self.configs
    }
}

impl SupportsUrlQueue for TestContext {
    type UrlQueue = TestUrlQueue;

    async fn can_poll(&self) -> bool {
        !self.links_queue.is_empty().await
    }

    fn url_queue(&self) -> &Self::UrlQueue {
        &self.links_queue
    }
}

impl SupportsFileSystemAccess for TestContext {
    type FileSystem = FileSystemAccess;

    fn fs(&self) -> &FileSystemAccess {
        panic!("Not supported by in memory actions!")
    }
}

impl SupportsWebGraph for TestContext {
    type WebGraphManager = TestLinkNetManager;

    fn web_graph_manager(&self) -> &Self::WebGraphManager {
        &self.link_net_manager
    }
}

impl SupportsStopwordsRegistry for TestContext {
    fn stopword_registry(&self) -> Option<&StopWordRegistry> {
        Some(&self.stop_word_registry)
    }
}

impl SupportsGdbrRegistry for TestContext {
    type Registry = GdbrIdentifierRegistry<Tf, Idf, L2R_L2LOSS_SVR>;

    fn gdbr_registry(&self) -> Option<&Self::Registry> {
        self.gdbr_registry.as_ref()
    }
}

impl SupportsSlimCrawlResults for TestContext {
    type Error = DatabaseError;

    async fn retrieve_slim_crawled_website(
        &self,
        url: &UrlWithDepth,
    ) -> Result<Option<SlimCrawlResult>, DatabaseError> {
        let crawled = self.crawled_websites.read().await;
        if let Some(found) = crawled.get(url.url()) {
            Ok(Some(found.clone()))
        } else {
            Ok(None)
        }
    }

    async fn store_slim_crawled_website(
        &self,
        result: SlimCrawlResult,
    ) -> Result<(), DatabaseError> {
        self.ct_crawled_websites.fetch_add(1, Ordering::Relaxed);
        let mut crawled = self.crawled_websites.write().await;
        crawled.insert(result.meta.url.url().clone(), result);
        Ok(())
    }
}

impl SupportsCrawlResults for TestContext {
    type Error = DatabaseError;

    async fn store_crawled_website(&self, result: &CrawlResult) -> Result<(), DatabaseError> {
        let hint = match &result.content {
            RawVecData::None => StoredDataHint::None,
            RawVecData::InMemory { data } => StoredDataHint::InMemory(data.clone()),
            RawVecData::ExternalFile { file } => StoredDataHint::External(file.clone()),
        };
        let slim = SlimCrawlResult::new(result, hint);
        self.store_slim_crawled_website(slim).await?;
        Ok(())
    }

    async fn retrieve_crawled_website(
        &self,
        url: &UrlWithDepth,
    ) -> Result<Option<CrawlResult>, DatabaseError> {
        self.retrieve_slim_crawled_website(url)
            .await
            .map(|value| value.map(|value| value.inflate(None)))
    }
}

#[derive(Debug)]
pub struct TestBlacklistManager {
    managed: ManagedBlacklist<PolyBlackList>,
    sender: ManagedBlacklistSender<PolyBlackList>,
    version: AtomicU64,
    entries: tokio::sync::RwLock<IndexSet<String>>,
}

impl TestBlacklistManager {
    pub fn new(entries: Option<Vec<String>>) -> Self {
        let blacklist = if let Some(value) = entries.clone() {
            PolyBlackList::new(value.len() as u64, value).unwrap()
        } else {
            PolyBlackList::default()
        };

        let (new, sender) = create_managed_blacklist(blacklist);

        Self {
            managed: new,
            sender,
            version: AtomicU64::default(),
            entries: tokio::sync::RwLock::new(IndexSet::from_iter(entries.unwrap_or_default())),
        }
    }
}

impl BlacklistManager for TestBlacklistManager {
    type Blacklist = PolyBlackList;

    async fn current_version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }

    async fn add(&self, value: String) -> Result<bool, BlacklistError> {
        let mut entries = self.entries.write().await;
        if !entries.insert(value) {
            return Ok(false);
        }
        let entries = entries.downgrade();
        let v = self.managed.version();
        self.sender.update(PolyBlackList::Regex(
            RegexBlackList::new(v + 1, entries.deref().clone())
                .expect("The regex blacklist should compile!"),
        ));
        Ok(true)
    }

    async fn apply_patch<I: IntoIterator<Item = String>>(&self, patch: I) {
        let mut entries = self.entries.write().await;
        let old = entries.len();
        entries.extend(patch);
        if old == entries.len() {
            return;
        }
        let v = self.managed.version();
        self.sender.update(PolyBlackList::Regex(
            RegexBlackList::new(v + 1, entries.deref().clone())
                .expect("The regex blacklist should compile!"),
        ));
    }

    async fn get_patch(&self, since_version: u64) -> Option<Vec<String>> {
        if self.current_version().await <= since_version {
            None
        } else {
            let entries = self.entries.read().await;
            Some(
                entries
                    .iter()
                    .dropping(since_version as usize)
                    .cloned()
                    .collect(),
            )
        }
    }

    async fn is_empty(&self) -> bool {
        self.entries.read().await.is_empty()
    }

    async fn get_blacklist(&self) -> ManagedBlacklist<PolyBlackList> {
        self.managed.clone()
    }
}

#[derive(Debug, Default, Clone)]
pub struct TestLinkNetManager {
    link_net: Arc<Mutex<Vec<WebGraphEntry>>>,
}

impl WebGraphManager for TestLinkNetManager {
    async fn add(&self, link_net_entry: WebGraphEntry) -> Result<(), LinkNetError> {
        self.link_net.lock().await.push(link_net_entry);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct TestUrlQueue {
    links_queue: Arc<Mutex<VecDeque<UrlQueueElement<UrlWithDepth>>>>,
    broadcast: tokio::sync::broadcast::Sender<EnqueueCalled>,
    factory: PollWaiterFactory
}

impl Default for TestUrlQueue {
    fn default() -> Self {
        Self {
            links_queue: Default::default(),
            broadcast: tokio::sync::broadcast::Sender::new(1),
            factory: PollWaiterFactory::new()
        }
    }
}

impl UrlQueue for TestUrlQueue {
    async fn enqueue_seed(&self, url: &str) -> Result<(), QueueError> {
        self.enqueue(UrlQueueElement::new(
            true,
            0,
            false,
            UrlWithDepth::from_seed(url)?,
        ))
        .await
    }

    /// Enqueues all [urls] at distance 0
    async fn enqueue_seeds(
        &self,
        urls: impl IntoIterator<Item = impl AsRef<str>> + Clone,
    ) -> Result<(), QueueError> {
        self.enqueue_all(
            urls.into_iter()
                .map(|s| {
                    UrlWithDepth::from_seed(s.as_ref())
                        .map(|value| UrlQueueElement::new(true, 0, false, value))
                })
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
        )
        .await
    }

    async fn enqueue(&self, entry: UrlQueueElement) -> Result<(), QueueError> {
        let mut lock = self.links_queue.lock().await;
        lock.push_back(UrlQueueElement::new(
            entry.is_seed,
            entry.age + 1,
            entry.host_was_in_use,
            entry.target.clone(),
        ));
        Ok(())
    }

    #[cfg(test)]
    async fn enqueue_borrowed<'a>(
        &self,
        entry: UrlQueueElement<&'a UrlWithDepth>,
    ) -> Result<(), QueueError> {
        self.enqueue(entry.map(|value| value.clone())).await
    }

    async fn enqueue_all(
        &self,
        entries: impl IntoIterator<Item = UrlQueueElement<UrlWithDepth>>,
    ) -> Result<(), QueueError> {
        let mut lock = self.links_queue.lock().await;
        lock.extend(entries.into_iter().map(|value| value.into()));
        Ok(())
    }

    async fn dequeue(&self) -> Result<Option<UrlQueueElement>, QueueError> {
        let mut lock = self.links_queue.lock().await;
        Ok(lock.pop_front())
    }

    #[cfg(test)]
    async fn dequeue_n(&self, n: usize) -> Result<Vec<UrlQueueElement>, QueueError> {
        let mut lock = self.links_queue.lock().await;
        let len = lock.len();
        Ok(lock.drain(0..min(len, n)).collect_vec())
    }

    async fn len(&self) -> usize {
        let lock = self.links_queue.lock().await;
        lock.len()
    }

    async fn is_empty(&self) -> bool {
        let lock = self.links_queue.lock().await;
        lock.is_empty()
    }

    fn subscribe_to_change(&self) -> Receiver<EnqueueCalled> {
        self.broadcast.subscribe()
    }

    fn start_polling(&self) -> PollWaiter {
        self.factory.create()
    }
}

#[derive(Debug)]
pub struct InMemoryLinkStateManager {
    state: std::sync::RwLock<HashMap<AtraUri, LinkState>>,
}

impl InMemoryLinkStateManager {
    pub fn new() -> Self {
        Self {
            state: Default::default(),
        }
    }
}

impl LinkStateManager for InMemoryLinkStateManager {
    type Error = LinkStateDBError;

    fn crawled_websites(&self) -> Result<u64, LinkStateDBError> {
        Ok(self.state.read().unwrap().len() as u64)
    }

    async fn update_link_state(
        &self,
        url: &UrlWithDepth,
        state: LinkStateKind,
    ) -> Result<(), LinkStateDBError> {
        let mut lock = self.state.write().unwrap();
        let raw_url = url.url();
        if let Some(target) = lock.get_mut(raw_url) {
            target.update_in_place(state.into_update(url, None));
        } else {
            lock.insert(raw_url.clone(), state.into_update(url, None));
        }
        Ok(())
    }

    async fn update_link_state_with_payload(
        &self,
        url: &UrlWithDepth,
        state: LinkStateKind,
        payload: Vec<u8>,
    ) -> Result<(), LinkStateDBError> {
        let mut lock = self.state.write().unwrap();
        let raw_url = url.url();
        if let Some(target) = lock.get_mut(raw_url) {
            target.update_in_place(state.into_update(url, Some(payload)));
        } else {
            lock.insert(raw_url.clone(), state.into_update(url, Some(payload)));
        }
        Ok(())
    }

    async fn get_link_state(
        &self,
        url: &UrlWithDepth,
    ) -> Result<Option<LinkState>, LinkStateDBError> {
        let lock = self.state.read().unwrap();
        Ok(lock.get(url.url()).map(|value| value.clone()))
    }

    async fn check_if_there_are_any_crawlable_links(&self, max_age: std::time::Duration) -> bool {
        let lock = self.state.read().unwrap();
        lock.iter().any(|value| {
            value.1.kind < LinkStateKind::ProcessedAndStored
                || OffsetDateTime::now_utc() - value.1.timestamp > max_age
        })
    }
}
