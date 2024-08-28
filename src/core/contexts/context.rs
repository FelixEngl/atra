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
use std::fmt::Debug;
use time::Duration;
use crate::core::blacklist::PolyBlackList;
use crate::core::config::Configs;
use crate::core::contexts::errors::{LinkHandlingError, RecoveryError};
use crate::core::crawl::result::CrawlResult;
use crate::core::crawl::seed::CrawlSeed;
use crate::core::crawl::slim::{SlimCrawlResult};
use crate::core::database_error::DatabaseError;
use crate::core::origin::OriginManager;
use crate::core::extraction::ExtractedLink;
use crate::core::web_graph::{WebGraphManager};
use crate::core::robots::RobotsManager;
use crate::core::link_state::{LinkState, LinkStateDBError, LinkStateType};
use crate::core::url::queue::UrlQueue;
use crate::core::UrlWithDepth;



/// What do you want to recover?
#[allow(dead_code)]
pub enum RecoveryCommand<'a> {
    All,
    UpdateLinkState(
        &'a UrlWithDepth,
        LinkStateType
    )
}



/// The context for a crawl
pub trait Context: Debug +  Send + Sync + 'static {
    /// The used robots manager
    type RobotsManager: RobotsManager;

    /// The url queue used by this
    type UrlQueue: UrlQueue;

    /// The domain manager used by this
    type HostManager: OriginManager;

    /// The manager for the link net
    type WebGraphManager: WebGraphManager;

    /// Returns true if poll possible
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
    fn get_host_manager(&self) -> &Self::HostManager;

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

pub trait SlimCrawlTaskContext: Context {
    /// Store a crawl result
    async fn store_slim_crawled_website(&self, result: SlimCrawlResult) -> Result<(), DatabaseError>;
}

/// A context that in addition to normal context actions allows to store and retrieve cralwed besites as a whole.
pub trait CrawlTaskContext: SlimCrawlTaskContext {
    /// Store a crawl result
    async fn store_crawled_website(&self, result: &CrawlResult) -> Result<(), DatabaseError>;

    /// Get the complete crawled website
    async fn retrieve_crawled_website(&self, url: &UrlWithDepth) -> Result<Option<CrawlResult>, DatabaseError>;

}