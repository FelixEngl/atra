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

/// A trait for marking a context trait
pub trait BaseContext {}

macro_rules! create_context_trait {
    ($($name: ident),+ $(,)?) => {
        /// The context for a crawl context, collecting all needed taits in one.
        pub trait Context: BaseContext + AsyncContext $(+$name)+
        {}

        impl<T> Context for T where T: ContextDelegate + AsyncContext $( + $name)+ {}
    };
}

create_context_trait! {
    SupportsLinkState,
    SupportsUrlGuarding,
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
    SupportsLinkSeeding,
    SupportsPolling,
    SupportsWorkerId,
}

pub mod traits {
    use crate::blacklist::lists::BlackList;
    use crate::config::Configs;
    use crate::contexts::BaseContext;
    use crate::crawl::CrawlResult;
    use crate::crawl::SlimCrawlResult;
    use crate::extraction::ExtractedLink;
    use crate::gdbr::identifier::GdbrRegistry;
    use crate::io::fs::AtraFS;
    use crate::link_state::{LinkState, LinkStateType};
    use crate::queue::polling::UrlQueuePollResult;
    use crate::robots::RobotsManager;
    use crate::runtime::ShutdownReceiver;
    use crate::seed::BasicSeed;
    use crate::url::guard::UrlGuardian;
    use crate::url::queue::UrlQueue;
    use crate::url::{UrlWithDepth, UrlWithGuard};
    use crate::web_graph::WebGraphManager;
    use std::collections::HashSet;
    use std::error::Error;
    use std::time::Duration;
    use text_processing::stopword_registry::StopWordRegistry;

    /// A marker interface for applying the context trait iff appropriate
    pub trait ContextDelegate {}

    impl<T> BaseContext for T where T: ContextDelegate {}

    /// A context marking a context as compatible with async tasks.
    /// Can basically do nothing alone and is only a helper interface for the
    /// required interfaces.
    pub trait AsyncContext: BaseContext + Send + Sync + 'static {}

    pub trait SupportsLinkSeeding: BaseContext {
        type Error: std::error::Error + Send + Sync;

        /// Registers a seed in the context as beeing crawled.
        async fn register_seed<S: BasicSeed>(&self, seed: &S) -> Result<(), Self::Error>;

        /// Register outgoing & data links.
        /// Also returns a list of all urls existing on the seed, that can be registered.
        async fn handle_links(
            &self,
            from: &UrlWithDepth,
            links: &HashSet<ExtractedLink>,
        ) -> Result<Vec<UrlWithDepth>, Self::Error>;
    }

    /// Used when some kind of link management happens
    pub trait SupportsLinkState: BaseContext {
        type Error: std::error::Error + Send + Sync;

        /// The number of crawled websites
        fn crawled_websites(&self) -> Result<u64, Self::Error>;

        /// Sets the state of the link
        async fn update_link_state(
            &self,
            url: &UrlWithDepth,
            state: LinkStateType,
        ) -> Result<(), Self::Error>;

        /// Sets the state of the link with a payload
        async fn update_link_state_with_payload(
            &self,
            url: &UrlWithDepth,
            state: LinkStateType,
            payload: Vec<u8>,
        ) -> Result<(), Self::Error>;

        /// Gets the state of the current url
        async fn get_link_state(
            &self,
            url: &UrlWithDepth,
        ) -> Result<Option<LinkState>, Self::Error>;

        /// Checks if there are any crawable links. [max_age] denotes the maximum amount of time since
        /// the last search
        async fn check_if_there_are_any_crawlable_links(&self, max_age: Duration) -> bool;
    }

    /// Used when a host manager is provided
    pub trait SupportsUrlGuarding: BaseContext {
        /// The domain manager used by this
        type Guardian: UrlGuardian;

        /// Returns a reference to a [GuardedDomainManager]
        fn get_guardian(&self) -> &Self::Guardian;
    }

    pub trait SupportsRobotsManager: BaseContext {
        /// The used robots manager
        type RobotsManager: RobotsManager;

        /// Get an instance of the robots manager.
        async fn get_robots_instance(&self) -> Self::RobotsManager;
    }

    pub trait SupportsBlackList: BaseContext {
        type BlackList: BlackList;

        /// Get some kind of blacklist
        async fn get_blacklist(&self) -> Self::BlackList;
    }

    pub trait SupportsMetaInfo: BaseContext {
        /// When did the crawl officially start?
        fn crawl_started_at(&self) -> time::OffsetDateTime;

        /// The amount of discovered websites.
        fn discovered_websites(&self) -> usize;
    }

    pub trait SupportsConfigs: BaseContext {
        /// Returns a reference to the config
        fn configs(&self) -> &Configs;
    }

    pub trait SupportsUrlQueue: BaseContext {
        /// The url queue used by this
        type UrlQueue: UrlQueue;

        // type Error: Error;

        /// Returns true if poll possible
        async fn can_poll(&self) -> bool;

        /// Get the instance of the url queue.
        fn url_queue(&self) -> &Self::UrlQueue;

        // Retrieves the next seed if possible.
        // fn poll_next_seed(&self, shutdown_handle: impl ShutdownReceiver, max_miss: Option<u64>) -> UrlQueuePollResult<>
    }

    pub trait SupportsFileSystemAccess: BaseContext {
        type FileSystem: AtraFS;

        /// Provides access to the filesystem
        fn fs(&self) -> &Self::FileSystem;
    }

    /// The context supports webgraphs
    pub trait SupportsWebGraph: BaseContext {
        /// The manager for the link net
        type WebGraphManager: WebGraphManager;

        /// Returns the link net manager
        fn web_graph_manager(&self) -> &Self::WebGraphManager;
    }

    /// The context needed for tokenizing to work
    pub trait SupportsStopwordsRegistry: BaseContext {
        /// Returns the sopword registry
        fn stopword_registry(&self) -> Option<&StopWordRegistry>;
    }

    pub trait SupportsGdbrRegistry: BaseContext {
        type Registry: GdbrRegistry;

        /// Gdbr Registry
        fn gdbr_registry(&self) -> Option<&Self::Registry>;
    }

    pub trait SupportsSlimCrawlResults: BaseContext {
        type Error: std::error::Error + Send + Sync;

        /// Retrieve a single crawled website but without the body
        async fn retrieve_slim_crawled_website(
            &self,
            url: &UrlWithDepth,
        ) -> Result<Option<SlimCrawlResult>, Self::Error>;

        /// Store a crawl result
        async fn store_slim_crawled_website(
            &self,
            result: SlimCrawlResult,
        ) -> Result<(), Self::Error>;
    }

    pub trait SupportsCrawlResults: BaseContext {
        type Error: std::error::Error + Send + Sync;

        /// Store a crawl result
        async fn store_crawled_website(&self, result: &CrawlResult) -> Result<(), Self::Error>;

        /// Get the complete crawled website
        async fn retrieve_crawled_website(
            &self,
            url: &UrlWithDepth,
        ) -> Result<Option<CrawlResult>, Self::Error>;
    }

    /// A trait that allows polling for a context that satisfies the basic
    /// requirements for it.
    pub trait SupportsPolling: BaseContext {
        type Guardian: UrlGuardian;

        type Error: Error;

        /// Tries to poll the next free url.
        async fn poll_next_free_url<'a>(
            &'a self,
            shutdown_handle: impl ShutdownReceiver,
            max_miss: Option<u64>,
        ) -> UrlQueuePollResult<UrlWithGuard<'a, Self::Guardian>, Self::Error>;
    }

    /// A trait for a context that supports worker ID
    pub trait SupportsWorkerId: BaseContext {
        fn worker_id(&self) -> usize;
    }
}
