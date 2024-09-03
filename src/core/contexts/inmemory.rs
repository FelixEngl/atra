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

use std::cell::LazyCell;
use std::cmp::min;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use itertools::Itertools;
use time::{Duration, OffsetDateTime};
use tokio::sync::{Mutex};
use tokio::sync::broadcast::Receiver;
use crate::core::blacklist::PolyBlackList;
use crate::core::config::Configs;
use crate::core::contexts::{Context, RecoveryCommand, SlimCrawlTaskContext};
use crate::core::crawl::result::CrawlResult;
use crate::core::crawl::seed::CrawlSeed;
use crate::core::crawl::slim::{SlimCrawlResult, StoredDataHint};
use crate::core::extraction::ExtractedLink;
use crate::core::link_state::{LinkState, LinkStateDBError, LinkStateType};
use crate::core::database_error::DatabaseError;
use crate::core::io::fs::FileSystemAccess;
use crate::core::web_graph::{WebGraphEntry, LinkNetError, WebGraphManager};
use crate::core::queue::QueueError;
use crate::core::robots::{InMemoryRobotsManager, ShareableRobotsManager};
use crate::core::url::queue::{EnqueueCalled, UrlQueue, UrlQueueElement, UrlQueueElementWeak};
use crate::core::{UrlWithDepth, VecDataHolder};
use crate::core::contexts::errors::{LinkHandlingError, RecoveryError};
use crate::core::origin::AtraOriginProvider;
use crate::core::origin::managers::InMemoryOriginManager;
use crate::core::url::atra_uri::AtraUri;
use crate::features::tokenizing::stopwords::StopWordRegistry;

#[derive(Debug)]
pub struct InMemoryContext {
    ct_crawled_websites: AtomicUsize,
    ct_found_websites: AtomicUsize,
    robots_manager: ShareableRobotsManager,
    blacklist: PolyBlackList,
    crawled_websites: tokio::sync::RwLock<HashMap<AtraUri, SlimCrawlResult>>,
    state: tokio::sync::RwLock<HashMap<AtraUri, LinkState>>,
    data_urls: Mutex<Vec<(UrlWithDepth, UrlWithDepth)>>,
    configs: Configs,
    host_manager: InMemoryOriginManager,
    started_at: OffsetDateTime,
    links_queue: InMemoryLinkQueue,
    link_net_manager: InMemoryLinkNetManager,
    stop_word_registry: StopWordRegistry
}



impl InMemoryContext {
    pub fn new(configs: Configs) -> Self {
        Self {
            ct_crawled_websites: AtomicUsize::new(0),
            ct_found_websites: AtomicUsize::new(0),
            robots_manager: InMemoryRobotsManager::new().into(),
            blacklist: PolyBlackList::default(),
            crawled_websites: tokio::sync::RwLock::new(HashMap::new()),
            state: tokio::sync::RwLock::new(HashMap::new()),
            links_queue: InMemoryLinkQueue::default(),
            data_urls: Default::default(),
            stop_word_registry: StopWordRegistry::default(),
            configs,
            host_manager: Default::default(),
            started_at: OffsetDateTime::now_utc(),
            link_net_manager: InMemoryLinkNetManager::default(),
        }
    }

    pub fn with_blacklist(configs: Configs, blacklist: PolyBlackList) -> Self {
        Self {
            blacklist,
            ..Self::new(configs)
        }
    }

    pub fn get_all_crawled_websites(self) -> (HashMap<AtraUri, CrawlResult>, HashMap<AtraUri, LinkState>) {
        let data = self.crawled_websites.into_inner().into_iter().map(|value| (value.0, value.1.inflate(None))).collect();
        let found = self.state.into_inner();
        (data, found)
    }

}

impl Default for InMemoryContext {
    fn default() -> Self {
        Self::new(Configs::default())
    }
}



impl super::Context for InMemoryContext {
    type RobotsManager = ShareableRobotsManager;
    type UrlQueue = InMemoryLinkQueue;
    type HostManager = InMemoryOriginManager;
    type WebGraphManager = InMemoryLinkNetManager;

    async fn can_poll(&self) -> bool {
        !self.links_queue.is_empty().await
    }


    fn fs(&self) -> &FileSystemAccess {
        panic!("Not supported by in memory actions!")
    }

    fn crawled_websites(&self) -> Result<u64, LinkStateDBError> {
        Ok(self.ct_crawled_websites.load(Ordering::Relaxed) as u64)
    }

    fn discovered_websites(&self) -> usize {
        self.ct_found_websites.load(Ordering::Relaxed)
    }

    fn url_queue(&self) -> &Self::UrlQueue {
        &self.links_queue
    }

    fn configs(&self) -> &Configs {
        &self.configs
    }

    fn crawl_started_at(&self) -> OffsetDateTime {
        self.started_at
    }

    fn web_graph_manager(&self) -> &Self::WebGraphManager {
        &self.link_net_manager
    }

    async fn get_blacklist(&self) -> PolyBlackList {
        self.blacklist.clone()
    }

    async fn get_robots_instance(&self) -> Self::RobotsManager {
        self.robots_manager.clone()
    }

    fn get_host_manager(&self) -> &InMemoryOriginManager {
        &self.host_manager
    }



    async fn retrieve_slim_crawled_website(&self, url: &UrlWithDepth) -> Result<Option<SlimCrawlResult>, DatabaseError> {
        let crawled = self.crawled_websites.read().await;
        if let Some(found) = crawled.get(url.url()) {
            Ok(Some(found.clone()))
        } else {
            Ok(None)
        }
    }

    async fn register_seed(&self, seed: &impl CrawlSeed) -> Result<(), LinkHandlingError> {
        self.link_net_manager.add(
            WebGraphEntry::create_seed(seed)
        ).await?;
        Ok(())
    }


    async fn handle_links(&self, from: &UrlWithDepth, links: &HashSet<ExtractedLink>) -> Result<Vec<UrlWithDepth>, LinkHandlingError> {

        let mut for_queue = Vec::with_capacity(links.len() / 2);
        let mut for_insert = Vec::with_capacity(links.len() / 2);
        for link in links {
            self.ct_found_websites.fetch_add(1, Ordering::Relaxed);
            match link {
                ExtractedLink::OnSeed{url, ..} => {
                    self.link_net_manager.add(WebGraphEntry::create_link(from, url)).await.unwrap();
                    for_insert.push(url.clone());
                }
                ExtractedLink::Outgoing{url, ..} => {
                    self.link_net_manager.add(WebGraphEntry::create_link(from, url)).await.unwrap();
                    if self.get_link_state(url).await?.is_none() {
                        self.update_link_state(url, LinkStateType::Discovered).await?;
                        if let Some(origin) = url.atra_origin() {
                            if self.configs.crawl().budget.get_budget_for(&origin).is_in_budget(url) {
                                for_queue.push(UrlQueueElement::new(false, 0, false, url.clone()));
                            }
                        }
                    }
                }
                ExtractedLink::Data{ base, url, ..} => {
                    self.data_urls.lock().await.push((base.clone(), url.clone()))
                }
            }
        }
        if !for_queue.is_empty() {
            self.links_queue.enqueue_all(for_queue).await?;
        }
        Ok(for_insert)
    }


    async fn update_link_state(&self, url: &UrlWithDepth, state: LinkStateType) -> Result<(), LinkStateDBError> {
        let mut lock = self.state.write().await;
        let raw_url = url.url();
        if let Some(target) = lock.get_mut(raw_url) {
            target.update_in_place(state.into_update(url, None));
        } else {
            lock.insert(raw_url.clone(), state.into_update(url, None));
        }
        Ok(())
    }

    async fn update_link_state_with_payload(&self, url: &UrlWithDepth, state: LinkStateType, payload: Vec<u8>) -> Result<(), LinkStateDBError> {
        let mut lock = self.state.write().await;
        let raw_url = url.url();
        if let Some(target) = lock.get_mut(raw_url) {
            target.update_in_place(state.into_update(url, Some(payload)));
        } else {
            lock.insert(raw_url.clone(), state.into_update(url, Some(payload)));
        }
        Ok(())
    }

    async fn get_link_state(&self, url: &UrlWithDepth) -> Result<Option<LinkState>, LinkStateDBError> {
        let lock = self.state.read().await;
        Ok(lock.get(url.url()).map(|value| value.clone()))
    }

    async fn check_if_there_are_any_crawlable_links(&self, max_age: Duration) -> bool {
        let lock = self.state.read().await;
        lock.iter().any(|value | value.1.typ < LinkStateType::ProcessedAndStored || OffsetDateTime::now_utc() - value.1.timestamp > max_age)
    }

    async fn recover<'a>(&self, _: RecoveryCommand<'a>) -> Result<(), RecoveryError> {
        todo!("Recovery not supported in this version.")
    }


}

impl crate::features::tokenizing::StopwordContext for InMemoryContext {
    fn stopword_registry(&self) -> &StopWordRegistry {
        &self.stop_word_registry
    }
}

impl super::SlimCrawlTaskContext for InMemoryContext {
    async fn store_slim_crawled_website(&self, result: SlimCrawlResult) -> Result<(), DatabaseError> {
        self.ct_crawled_websites.fetch_add(1, Ordering::Relaxed);
        let mut crawled = self.crawled_websites.write().await;
        crawled.insert(result.meta.url.url().clone(), result);
        Ok(())
    }
}

impl super::CrawlTaskContext for InMemoryContext {

    async fn retrieve_crawled_website(&self, url: &UrlWithDepth) -> Result<Option<CrawlResult>, DatabaseError> {
        self.retrieve_slim_crawled_website(url).await.map(|value| value.map(|value| value.inflate(None)))
    }

    async fn store_crawled_website(&self, result: &CrawlResult) -> Result<(), DatabaseError> {
        let hint = match &result.content {
            VecDataHolder::None => {StoredDataHint::None}
            VecDataHolder::InMemory { data } => {StoredDataHint::InMemory(data.clone())}
            VecDataHolder::ExternalFile { file } => {StoredDataHint::External(file.clone())}
        };
        let slim = SlimCrawlResult::new(result, hint);
        self.store_slim_crawled_website(slim).await?;
        Ok(())
    }

}

#[derive(Debug, Default, Clone)]
pub struct InMemoryLinkNetManager {
    link_net: Arc<Mutex<Vec<WebGraphEntry>>>
}

// impl InMemoryLinkNetManager {
//     pub fn new() -> Self {
//         Self {
//             link_net: Arc::new(Mutex::new(Vec::new()))
//         }
//     }
// }

impl WebGraphManager for InMemoryLinkNetManager {
    async fn add(&self, link_net_entry: WebGraphEntry) -> Result<(), LinkNetError> {
        self.link_net.lock().await.push(link_net_entry);
        Ok(())
    }
}


#[derive(Debug, Clone)]
pub struct InMemoryLinkQueue {
    links_queue: Arc<Mutex<VecDeque<UrlQueueElement>>>,
    broadcast: tokio::sync::broadcast::Sender<EnqueueCalled>
}

impl Default for InMemoryLinkQueue {
    fn default() -> Self {
        Self {
            links_queue: Arc::default(),
            broadcast: tokio::sync::broadcast::Sender::new(1)
        }
    }
}


impl UrlQueue for InMemoryLinkQueue {

    async fn enqueue_seed(&self, url: &str) -> Result<(), QueueError> {
        self.enqueue(UrlQueueElementWeak::new(true, 0, false, &UrlWithDepth::from_seed(url).unwrap())).await
    }

    /// Enqueues all [urls] at distance 0
    async fn enqueue_seeds(&self, urls: impl IntoIterator<Item = impl AsRef<str>> + Clone) -> Result<(), QueueError> {
        self.enqueue_all(
            urls.into_iter()
                .map(|s| UrlWithDepth::from_seed(s.as_ref()).map(|value| UrlQueueElement::new(true, 0, false, value)))
                .collect::<Result<Vec<_>, _>>().unwrap()
        ).await
    }

    async fn enqueue<'a>(&self, entry: UrlQueueElementWeak<'a>) -> Result<(), QueueError> {
        let mut lock = self.links_queue.lock().await;
        lock.push_back(UrlQueueElement::new(entry.is_seed, entry.age + 1, entry.host_was_in_use, entry.target.clone()));
        Ok(())
    }

    async fn enqueue_all<E: Into<UrlQueueElement>>(&self, entries: impl IntoIterator<Item=E> + Clone) -> Result<(), QueueError> {
        let mut lock = self.links_queue.lock().await;
        lock.extend(entries.into_iter().map(|value| value.into()));
        Ok(())
    }

    async fn dequeue(&self) -> Result<Option<UrlQueueElement>, QueueError> {
        let mut lock = self.links_queue.lock().await;
        Ok(lock.pop_front())
    }

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
}



