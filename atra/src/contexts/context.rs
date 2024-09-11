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

use traits::*;

macro_rules! create_context_trait {
    ($($name: ident),+ $(,)?) => {
        /// The context for a crawl context, collecting all needed taits in one.
        pub trait Context: AsyncContext $(+$name)+
        {}

        impl<T> Context for T where T: ContextDelegate + AsyncContext $( + $name)+ {}
    };
}

create_context_trait! {
    SupportsLinkState,
    SupportsHostManagement,
    SupportsRobotsManager,
    SupportsBlackList,
    SupportsMetaInfo,
    SupportsConfigs,
    SupportsUrlQueue,
    SupportsFileSystemAccess,
    SupportsWebGraph,
    SupportsStopwordsRegistry,
    SupportsGdbrRegistry,
    SupportsSlimCrawlResults,
    SupportsCrawlResults,
}


#[allow(dead_code)]
pub mod traits {
    use std::collections::HashSet;
    use std::time::Duration;
    use liblinear::solver::traits::Solver;
    use crate::blacklist::PolyBlackList;
    use crate::config::Configs;
    use crate::contexts::errors::LinkHandlingError;
    use crate::crawl::result::CrawlResult;
    use crate::crawl::seed::CrawlSeed;
    use crate::crawl::slim::SlimCrawlResult;
    use crate::database_error::DatabaseError;
    use crate::extraction::ExtractedLink;
    use crate::link_state::{LinkState, LinkStateDBError, LinkStateType};
    use crate::origin::OriginManager;
    use crate::robots::RobotsManager;
    use crate::url::queue::UrlQueue;
    use crate::url::url_with_depth::UrlWithDepth;
    use crate::web_graph::WebGraphManager;
    use text_processing::tf_idf::{IdfAlgorithm, TfAlgorithm};
    use text_processing::stopword_registry::StopWordRegistry;
    use crate::gdbr::identifier::GdbrIdentifierRegistry;

    /// A marker interface for applying the context iff apropiate
    pub trait ContextDelegate{}

    /// A context marking a context as compatible with async tasks.
    /// Can basically do nothing alone and is only a helper interface for the
    /// required interfaces.
    pub trait AsyncContext: Send + Sync + 'static {}

    /// Used when some kind of link management happens
    pub trait SupportsLinkState {

        /// The number of crawled websites
        fn crawled_websites(&self) -> Result<u64, LinkStateDBError>;

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
    }

    /// Used when a host manager is provided
    pub trait SupportsHostManagement {
        /// The domain manager used by this
        type HostManager: OriginManager;

        /// Returns a reference to a [GuardedDomainManager]
        fn get_host_manager(&self) -> &Self::HostManager;
    }

    pub trait SupportsRobotsManager {
        /// The used robots manager
        type RobotsManager: RobotsManager;

        /// Get an instance of the robots manager.
        async fn get_robots_instance(&self) -> Self::RobotsManager;
    }

    pub trait SupportsBlackList {
        /// Get some kind of blacklist
        async fn get_blacklist(&self) -> PolyBlackList;
    }

    pub trait SupportsMetaInfo {
        /// When did the crawl officially start?
        fn crawl_started_at(&self) -> time::OffsetDateTime;

        /// The amount of discovered websites.
        fn discovered_websites(&self) -> usize;
    }

    pub trait SupportsConfigs {

        /// Returns a reference to the config
        fn configs(&self) -> &Configs;
    }

    pub trait SupportsUrlQueue {
        /// The url queue used by this
        type UrlQueue: UrlQueue;

        /// Returns true if poll possible
        async fn can_poll(&self) -> bool;

        /// Get the instance of the url queue.
        fn url_queue(&self) -> &Self::UrlQueue;
    }

    pub trait SupportsFileSystemAccess {
        /// Provides access to the filesystem
        fn fs(&self) -> &crate::io::fs::FileSystemAccess;
    }


    /// The context supports webgraphs
    pub trait SupportsWebGraph {
        /// The manager for the link net
        type WebGraphManager: WebGraphManager;


        /// Returns the link net manager
        fn web_graph_manager(&self) -> &Self::WebGraphManager;
    }


    /// The context needed for tokenizing to work
    pub trait SupportsStopwordsRegistry {

        /// Returns the sopword registry
        fn stopword_registry(&self) -> Option<&StopWordRegistry>;
    }


    pub trait SupportsGdbrRegistry {

        type Solver: Solver;

        type TF: TfAlgorithm;

        type IDF: IdfAlgorithm;

        /// Gdbr Registry
        fn gdbr_registry(&self) -> Option<&GdbrIdentifierRegistry<Self::TF, Self::IDF, Self::Solver>>;
    }

    pub trait SupportsSlimCrawlResults {
        /// Retrieve a single crawled website but without the body
        async fn retrieve_slim_crawled_website(&self, url: &UrlWithDepth) -> Result<Option<SlimCrawlResult>, DatabaseError>;

        /// Store a crawl result
        async fn store_slim_crawled_website(&self, result: SlimCrawlResult) -> Result<(), DatabaseError>;
    }

    pub trait SupportsCrawlResults {
        /// Store a crawl result
        async fn store_crawled_website(&self, result: &CrawlResult) -> Result<(), DatabaseError>;

        /// Get the complete crawled website
        async fn retrieve_crawled_website(&self, url: &UrlWithDepth) -> Result<Option<CrawlResult>, DatabaseError>;
    }
}
