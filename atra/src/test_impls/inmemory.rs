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
use crate::client::traits::{AtraClient, AtraResponse};
use crate::config::Config;
use crate::contexts::local::LinkHandlingError;
use crate::contexts::traits::*;
use crate::contexts::{BaseContext, Context};
use crate::crawl::{CrawlResult, CrawlTask, SlimCrawlResult, StoredDataHint};
use crate::data::RawVecData;
use crate::database::DatabaseError;
use crate::extraction::ExtractedLink;
use crate::gdbr::identifier::GdbrIdentifierRegistry;
use crate::io::fs::{AtraFS, WorkerFileSystemAccess};
use crate::link_state::{
    IsSeedYesNo, LinkStateDBError, LinkStateKind, LinkStateLike, LinkStateManager, RawLinkState,
    RecrawlYesNo,
};
use crate::queue::{EnqueueCalled, UrlQueue, UrlQueueElement};
use crate::queue::{QueueError, SupportsForcedQueueElement, UrlQueueElementRef};
use crate::recrawl_management::DomainLastCrawledManager;
use crate::robots::{CachedRobots, RobotsError, RobotsManager};
use crate::seed::{BasicSeed, UnguardedSeed};
use crate::test_impls::providers::{ClientProvider, DefaultAtraProvider};
use crate::url::guard::InMemoryUrlGuardian;
use crate::url::{AtraOriginProvider, AtraUri};
use crate::url::{AtraUrlOrigin, UrlWithDepth};
use crate::web_graph::{WebGraphEntry, WebGraphError, WebGraphManager};
use indexmap::IndexSet;
use itertools::Itertools;
use liblinear::solver::L2R_L2LOSS_SVR;
use rand::distributions::Alphanumeric;
use rand::Rng;
use std::cmp::min;
use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::fmt::Debug;
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use camino::{Utf8Path, Utf8PathBuf};
use camino_tempfile::Utf8TempDir;
use text_processing::stopword_registry::StopWordRegistry;
use text_processing::tf_idf::{Idf, Tf};
use texting_robots::{get_robots_url, Robot};
use time::{Duration, OffsetDateTime};
use tokio::sync::watch::Receiver;
use tokio::sync::Mutex;
use crate::io::errors::ErrorWithPath;
use crate::io::serial::SerialProvider;

#[derive(Debug)]
pub struct TestContext<Provider = DefaultAtraProvider> {
    pub ct_crawled_websites: AtomicUsize,
    pub ct_found_websites: AtomicUsize,
    pub link_state_manager: InMemoryLinkStateManager,
    pub robots_manager: InMemoryRobotsManager,
    pub blacklist_manager: TestBlacklistManager,
    pub crawled_websites: std::sync::RwLock<HashMap<AtraUri, SlimCrawlResult>>,
    pub data_urls: Mutex<Vec<(UrlWithDepth, UrlWithDepth)>>,
    pub configs: Config,
    pub host_manager: InMemoryUrlGuardian,
    pub started_at: OffsetDateTime,
    pub links_queue: TestUrlQueue,
    pub link_net_manager: TestLinkNetManager,
    pub stop_word_registry: StopWordRegistry,
    pub gdbr_registry: Option<GdbrIdentifierRegistry<Tf, Idf, L2R_L2LOSS_SVR>>,
    pub fs: Arc<TestFS>,
    pub provider: Provider,
    pub domain_manager: InMemoryDomainManager,
}

impl<Provider> TestContext<Provider>
where
    Provider: Send + Sync + 'static,
{
    pub fn new(configs: Config, provider: Provider) -> Self {
        Self {
            ct_crawled_websites: AtomicUsize::new(0),
            ct_found_websites: AtomicUsize::new(0),
            robots_manager: InMemoryRobotsManager::new(),
            blacklist_manager: TestBlacklistManager::new(Default::default()),
            crawled_websites: RwLock::new(HashMap::new()),
            link_state_manager: InMemoryLinkStateManager::new(),
            links_queue: TestUrlQueue::default(),
            data_urls: Default::default(),
            stop_word_registry: StopWordRegistry::default(),
            configs,
            host_manager: Default::default(),
            fs: Arc::new(TestFS::new()),
            started_at: OffsetDateTime::now_utc(),
            link_net_manager: TestLinkNetManager::default(),
            gdbr_registry: None,
            domain_manager: Default::default(),
            provider,
        }
    }

    pub fn with_blacklist(
        configs: Config,
        provider: Provider,
        blacklist: Option<Vec<String>>,
    ) -> Self {
        Self {
            blacklist_manager: TestBlacklistManager::new(blacklist),
            ..Self::new(configs, provider)
        }
    }

    /// Returns the crawled websites on the left the results, on the right the data.
    pub fn get_all_crawled_websites(
        &self,
    ) -> (HashMap<AtraUri, CrawlResult>, HashMap<AtraUri, Vec<u8>>) {
        let data = self
            .crawled_websites
            .read()
            .unwrap()
            .iter()
            .map(|value| (value.0.clone(), unsafe{value.1.clone().inflate_unchecked().unwrap()}))
            .collect();
        let found = self.link_state_manager.state.read().unwrap().clone();
        (data, found)
    }

    pub fn provider(&self) -> &Provider {
        &self.provider
    }
}

impl Default for TestContext<DefaultAtraProvider> {
    fn default() -> Self {
        Self::new(Config::default(), DefaultAtraProvider::default())
    }
}

impl<Provider> BaseContext for TestContext<Provider> where Provider: Send + Sync + 'static {}

impl<Provider> AsyncContext for TestContext<Provider> where Provider: Send + Sync + 'static {}

impl<Provider> SupportsWorkerId for TestContext<Provider>
where
    Provider: Send + Sync + 'static,
{
    fn worker_id(&self) -> usize {
        0
    }
}

impl<Provider> SupportsCrawling for TestContext<Provider>
where
    Provider: Send + Sync + 'static + ClientProvider,
{
    type Client = Provider::Client;
    type Error = Provider::Error;

    fn create_crawl_task<S>(&self, seed: S) -> Result<CrawlTask<S, Self::Client>, Self::Error>
    where
        S: BasicSeed,
    {
        let seed2 = UnguardedSeed::new(
            seed.url().clone(),
            seed.origin().clone(),
            seed.is_original_seed(),
        )
        .unwrap();
        let provider = self.provider.provide(self, &seed2)?;
        Ok(CrawlTask::new(seed, provider))
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

impl<Provider> Context for TestContext<Provider> where
    Provider: Send + Sync + 'static + ClientProvider
{
}

impl<Provider> SupportsLinkSeeding for TestContext<Provider>
where
    Provider: Send + Sync + 'static,
{
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

impl<Provider> SupportsDomainHandling for TestContext<Provider>
where
    Provider: Send + Sync + 'static,
{
    type DomainHandler = InMemoryDomainManager;
    fn get_domain_manager(&self) -> &InMemoryDomainManager {
        &self.domain_manager
    }
}

impl<Provider> SupportsLinkState for TestContext<Provider>
where
    Provider: Send + Sync + 'static,
{
    type LinkStateManager = InMemoryLinkStateManager;
    fn get_link_state_manager(&self) -> &Self::LinkStateManager {
        &self.link_state_manager
    }
}

impl<Provider> SupportsUrlGuarding for TestContext<Provider>
where
    Provider: Send + Sync + 'static,
{
    type Guardian = InMemoryUrlGuardian;

    fn get_guardian(&self) -> &InMemoryUrlGuardian {
        &self.host_manager
    }
}

impl<Provider> SupportsRobotsManager for TestContext<Provider>
where
    Provider: Send + Sync + 'static,
{
    type RobotsManager = InMemoryRobotsManager;

    fn get_robots_manager(&self) -> &Self::RobotsManager {
        &self.robots_manager
    }
}

impl<Provider> SupportsBlackList for TestContext<Provider>
where
    Provider: Send + Sync + 'static,
{
    type BlacklistManager = TestBlacklistManager;
    fn get_blacklist_manager(&self) -> &Self::BlacklistManager {
        &self.blacklist_manager
    }
}

impl<Provider> SupportsMetaInfo for TestContext<Provider>
where
    Provider: Send + Sync + 'static,
{
    fn crawl_started_at(&self) -> OffsetDateTime {
        self.started_at
    }

    fn discovered_websites(&self) -> usize {
        self.ct_found_websites.load(Ordering::Relaxed)
    }
}

impl<Provider> SupportsConfigs for TestContext<Provider>
where
    Provider: Send + Sync + 'static,
{
    fn configs(&self) -> &Config {
        &self.configs
    }
}

impl<Provider> SupportsUrlQueue for TestContext<Provider>
where
    Provider: Send + Sync + 'static,
{
    type UrlQueue = TestUrlQueue;

    async fn can_poll(&self) -> bool {
        !self.links_queue.is_empty().await
    }

    fn url_queue(&self) -> &Self::UrlQueue {
        &self.links_queue
    }
}

impl<Provider> SupportsFileSystemAccess for TestContext<Provider>
where
    Provider: Send + Sync + 'static,
{
    type FileSystem = TestFS;

    fn fs(&self) -> &TestFS {
        &self.fs
    }
}

#[derive(Debug)]
pub struct TestFS {
    temp_dir: Utf8TempDir,
    id_prov: SerialProvider
}

impl TestFS {
    pub fn new() -> Self {
        Self { temp_dir: Utf8TempDir::new().unwrap(), id_prov: SerialProvider::default() }
    }
}

impl AtraFS for TestFS {
    fn create_unique_path_for_dat_file(&self, _url: &str) -> Utf8PathBuf {
        self.temp_dir.path().join(format!("dat_{}.tmp", self.id_prov.provide_serial().unwrap().to_string())).to_path_buf()
    }

    fn get_unique_path_for_data_file(&self, _path: impl AsRef<Utf8Path>) -> Utf8PathBuf {
        todo!()
    }

    fn cleanup_data_file(&self, path: impl AsRef<Utf8Path>) -> std::io::Result<()> {
        std::fs::remove_file(path.as_ref())
    }

    fn create_worker_file_provider(&self, _worker_id: usize, _recrawl_iteration: usize) -> Result<WorkerFileSystemAccess, ErrorWithPath> {
        todo!()
    }
}

impl<Provider> SupportsWebGraph for TestContext<Provider>
where
    Provider: Send + Sync + 'static,
{
    type WebGraphManager = TestLinkNetManager;

    fn web_graph_manager(&self) -> Option<&Self::WebGraphManager> {
        if self.configs.crawl.generate_web_graph {
            Some(&self.link_net_manager)
        } else {
            None
        }
    }
}

impl<Provider> SupportsStopwordsRegistry for TestContext<Provider>
where
    Provider: Send + Sync + 'static,
{
    fn stopword_registry(&self) -> Option<&StopWordRegistry> {
        Some(&self.stop_word_registry)
    }
}

impl<Provider> SupportsGdbrRegistry for TestContext<Provider>
where
    Provider: Send + Sync + 'static,
{
    type Registry = GdbrIdentifierRegistry<Tf, Idf, L2R_L2LOSS_SVR>;

    fn gdbr_registry(&self) -> Option<&Self::Registry> {
        self.gdbr_registry.as_ref()
    }
}

impl<Provider> SupportsSlimCrawlResults for TestContext<Provider>
where
    Provider: Send + Sync + 'static,
{
    type Error = DatabaseError;

    async fn retrieve_slim_crawled_website(
        &self,
        url: &UrlWithDepth,
    ) -> Result<Option<SlimCrawlResult>, DatabaseError> {
        let crawled = self.crawled_websites.read().unwrap();
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
        let mut crawled = self.crawled_websites.write().unwrap();
        crawled.insert(result.meta.url.url().clone(), result);
        Ok(())
    }
}

impl<Provider> SupportsCrawlResults for TestContext<Provider>
where
    Provider: Send + Sync + 'static,
{
    type Error = DatabaseError;

    async fn store_crawled_website(&self, result: &CrawlResult) -> Result<(), DatabaseError> {
        let hint = match &result.content {
            RawVecData::None => StoredDataHint::None,
            RawVecData::InMemory { data } => StoredDataHint::InMemory(data.clone()),
            RawVecData::ExternalFile { path } => {
                assert!(path.exists());
                StoredDataHint::External(path.clone())
            },
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
            .map(|value| value.map(|value| unsafe{value.inflate_unchecked().unwrap()}))
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
    async fn add(&self, link_net_entry: WebGraphEntry) -> Result<(), WebGraphError> {
        self.link_net.lock().await.push(link_net_entry);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct TestUrlQueue {
    links_queue: Arc<std::sync::Mutex<VecDeque<UrlQueueElement<UrlWithDepth>>>>,
    broadcast: tokio::sync::watch::Sender<EnqueueCalled>,
    counter: crate::queue::UrlQueueElementRefCounter,
}

unsafe impl Send for TestUrlQueue {}
unsafe impl Sync for TestUrlQueue {}

impl TestUrlQueue {
    fn wrap(&self, value: UrlQueueElement<UrlWithDepth>) -> UrlQueueElementRef<UrlWithDepth> {
        let no = self.counter.create_drop_notifyer();
        UrlQueueElementRef::new(value, self, no)
    }
}

impl SupportsForcedQueueElement<UrlWithDepth> for TestUrlQueue {
    fn force_enqueue(&self, entry: UrlQueueElement<UrlWithDepth>) -> Result<(), QueueError> {
        Ok(self.links_queue.lock().unwrap().push_back(entry))
    }
}

impl Default for TestUrlQueue {
    fn default() -> Self {
        Self {
            links_queue: Default::default(),
            broadcast: tokio::sync::watch::Sender::new(EnqueueCalled),
            counter: crate::queue::UrlQueueElementRefCounter::new(),
        }
    }
}

impl UrlQueue<UrlWithDepth> for TestUrlQueue {
    async fn enqueue(&self, entry: UrlQueueElement) -> Result<(), QueueError> {
        let mut lock = self.links_queue.lock().unwrap();
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
        self.force_enqueue(entry.map(|value| value.clone()))
    }

    async fn enqueue_all(
        &self,
        entries: impl IntoIterator<Item = UrlQueueElement<UrlWithDepth>>,
    ) -> Result<(), QueueError> {
        let mut lock = self.links_queue.lock().unwrap();
        lock.extend(entries.into_iter().map(|value| value.into()));
        Ok(())
    }

    async fn dequeue<'a>(
        &'a self,
    ) -> Result<Option<UrlQueueElementRef<'a, UrlWithDepth>>, QueueError> {
        let mut lock = self.links_queue.lock().unwrap();
        Ok(lock.pop_front().map(|value| self.wrap(value)))
    }

    #[cfg(test)]
    async fn dequeue_n<'a>(
        &'a self,
        n: usize,
    ) -> Result<Vec<UrlQueueElementRef<'a, UrlWithDepth>>, QueueError> {
        let mut lock = self.links_queue.lock().unwrap();
        let len = lock.len();
        Ok(lock
            .drain(0..min(len, n))
            .map(|value| self.wrap(value))
            .collect_vec())
    }

    async fn len(&self) -> usize {
        let lock = self.links_queue.lock().unwrap();
        lock.len() + self.counter.get_count()
    }

    async fn is_empty(&self) -> bool {
        let lock = self.links_queue.lock().unwrap();
        lock.is_empty()
    }

    fn has_floating_urls(&self) -> bool {
        self.counter.awaits_drops()
    }

    fn floating_url_count(&self) -> usize {
        self.counter.get_count()
    }

    fn subscribe_to_change(&self) -> Receiver<EnqueueCalled> {
        self.broadcast.subscribe()
    }
}

#[derive(Debug)]
pub struct InMemoryLinkStateManager {
    state: std::sync::RwLock<HashMap<AtraUri, Vec<u8>>>,
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

    async fn update_link_state<P>(
        &self,
        url: &UrlWithDepth,
        state: LinkStateKind,
        is_seed: Option<IsSeedYesNo>,
        recrawl: Option<RecrawlYesNo>,
        payload: Option<Option<&P>>,
    ) -> Result<(), Self::Error>
    where
        P: ?Sized + AsRef<[u8]>,
    {
        let mut lock = self.state.write().unwrap();
        let raw_url = url.url();
        let upsert = RawLinkState::new_preconfigured_upsert(url, state, is_seed, recrawl, payload);
        if let Some(target) = lock.get_mut(raw_url) {
            RawLinkState::fold_merge_linkstate_test(target, url.as_bytes(), &upsert)
        } else {
            lock.insert(raw_url.clone(), upsert.deref().to_vec());
        }
        Ok(())
    }

    fn get_link_state_sync(&self, url: &UrlWithDepth) -> Result<Option<RawLinkState>, Self::Error> {
        let lock = self.state.read().unwrap();
        Ok(lock
            .get(url.url())
            .map(|value| unsafe { RawLinkState::from_slice_unchecked(&value) }))
    }

    async fn get_link_state(
        &self,
        url: &UrlWithDepth,
    ) -> Result<Option<RawLinkState>, LinkStateDBError> {
        let lock = self.state.read().unwrap();
        Ok(lock
            .get(url.url())
            .map(|value| unsafe { RawLinkState::from_slice_unchecked(&value) }))
    }

    async fn check_if_there_are_any_crawlable_links(&self, max_age: std::time::Duration) -> bool {
        let lock = self.state.read().unwrap();
        lock.iter().any(|value| {
            RawLinkState::read_kind(&value.1).unwrap() < LinkStateKind::ProcessedAndStored
                || OffsetDateTime::now_utc() - RawLinkState::read_timestamp(&value.1).unwrap()
                    > max_age
        })
    }

    async fn check_if_there_are_any_recrawlable_links(&self) -> bool {
        let lock = self.state.read().unwrap();
        lock.iter()
            .any(|value| RawLinkState::read_recrawl(&value.1).unwrap().is_yes())
    }

    async fn collect_recrawlable_links<F: Fn(IsSeedYesNo, UrlWithDepth) -> ()>(
        &self,
        collector: F,
    ) {
        let lock = self.state.read().unwrap();
        for (k, v) in lock.iter() {
            let raw = RawLinkState::from_slice(v.as_ref()).unwrap();
            if raw.recrawl().is_yes() {
                collector(raw.is_seed(), UrlWithDepth::new(k.clone(), raw.depth()))
            }
        }
    }

    async fn collect_all_links<F: Fn(IsSeedYesNo, UrlWithDepth) -> ()>(&self, collector: F) {
        let lock = self.state.read().unwrap();
        for (k, v) in lock.iter() {
            let raw = RawLinkState::from_slice(v.as_ref()).unwrap();
            collector(raw.is_seed(), UrlWithDepth::new(k.clone(), raw.depth()))
        }
    }
}

#[derive(Clone, Default, Debug)]
pub struct InMemoryDomainManager {
    inner: Arc<RwLock<HashMap<AtraUrlOrigin, OffsetDateTime>>>,
}

impl DomainLastCrawledManager for InMemoryDomainManager {
    async fn register_access(&self, domain: &AtraUrlOrigin) {
        self.inner
            .write()
            .unwrap()
            .insert(domain.clone(), OffsetDateTime::now_utc());
    }

    async fn get_last_access(&self, domain: &AtraUrlOrigin) -> Option<OffsetDateTime> {
        self.inner.read().unwrap().get(domain).cloned()
    }
}

/// An in memory variant of a robots.txt manager
/// Ideal for smaller crawls
#[derive(Debug, Default)]
pub struct InMemoryRobotsManager {
    cache: tokio::sync::RwLock<HashMap<AtraUrlOrigin, Arc<CachedRobots>>>,
}

impl InMemoryRobotsManager {
    #[cfg(test)]
    pub fn new() -> Self {
        Self {
            cache: tokio::sync::RwLock::new(HashMap::new()),
        }
    }
}

impl RobotsManager for InMemoryRobotsManager {
    async fn get<E: Error>(
        &self,
        _: &str,
        url: &UrlWithDepth,
        max_age: Option<&Duration>,
    ) -> Result<Option<Arc<CachedRobots>>, RobotsError<E>> {
        let domain = url.atra_origin().ok_or(RobotsError::NoDomainForUrl)?;
        let cache = self.cache.read().await;
        let found = if let Some(found) = cache.get(&domain) {
            if let Some(max_age) = max_age {
                if (OffsetDateTime::now_utc() - found.retrieved_at()).le(max_age) {
                    Some(found.clone())
                } else {
                    drop(cache);
                    let mut cache = self.cache.write().await;
                    cache.remove(&domain);
                    None
                }
            } else {
                Some(found.clone())
            }
        } else {
            None
        };
        Ok(found)
    }

    async fn get_or_retrieve<C: AtraClient>(
        &self,
        client: &C,
        agent: &str,
        url: &UrlWithDepth,
        max_age: Option<&Duration>,
    ) -> Result<Arc<CachedRobots>, RobotsError<C::Error>> {
        if let Some(found) = self.get(agent, url, max_age).await? {
            return Ok(found);
        }
        // Later used but cheaper than downloading and then recognizing invalidity for manager.
        let origin = url.atra_origin().ok_or(RobotsError::NoDomainForUrl)?;
        let result = client
            .get(&get_robots_url(&url.try_as_str())?)
            .await
            .map_err(RobotsError::ClientWasNotAbleToSend)?;
        let retrieved_at = OffsetDateTime::now_utc();
        let status_code = result.status();
        let result = result.bytes().await;

        let retrieved = if let Ok(result) = result {
            if status_code.is_client_error() || status_code.is_server_error() {
                CachedRobots::NoRobots {
                    retrieved_at,
                    _status_code: status_code,
                }
            } else {
                let robot =
                    Robot::new(agent, result.as_ref()).map_err(RobotsError::InvalidRobotsTxt)?;
                CachedRobots::HasRobots {
                    robot,
                    retrieved_at,
                }
            }
        } else {
            CachedRobots::NoRobots {
                retrieved_at,
                _status_code: status_code,
            }
        };

        let retrieved = Arc::new(retrieved);
        let mut cache = self.cache.write().await;
        let retrieved = if let Some(found) = cache.remove(&origin) {
            if found.retrieved_at() < retrieved.retrieved_at() {
                cache.insert(origin, retrieved.clone());
                retrieved
            } else {
                cache.insert(origin, found.clone());
                found
            }
        } else {
            cache.insert(origin, retrieved.clone());
            retrieved
        };
        drop(cache);
        Ok(retrieved)
    }
}
