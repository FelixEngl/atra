// Copyright 2024. Felix Engl
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

use traits::*;

/// A trait for marking a context trait
pub trait BaseContext {}

macro_rules! create_abstract_traits {
    ($($(#[$meta:meta])* $t_name: ident {$($name: ident),+ $(,)?}),+) => {
        $(
            $(#[$meta])*
            pub trait $t_name: BaseContext + AsyncContext $(+$name)+ {}

            impl<T> $t_name for T where T: ContextDelegate + AsyncContext $( + $name)+ {}
        )+
    };
}

create_abstract_traits! {
    #[doc = "The context for a crawl context, collecting all needed taits in one."]
    Context {
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
        SupportsCrawling,
        SupportsDomainHandling,
        SupportsBudgetManagement
    }
}

pub mod traits {
    use crate::blacklist::BlacklistManager;
    use crate::client::traits::AtraClient;
    use crate::config::Config;
    use crate::contexts::BaseContext;
    use crate::crawl::SlimCrawlResult;
    use crate::crawl::{CrawlResult, CrawlTask};
    use crate::extraction::ExtractedLink;
    use crate::gdbr::identifier::GdbrRegistry;
    use crate::io::fs::AtraFS;
    use crate::link_state::LinkStateManager;
    use crate::queue::{SupportsForcedQueueElement, UrlQueue, UrlQueuePollResult};
    use crate::recrawl_management::DomainLastCrawledManager;
    use crate::robots::RobotsManager;
    #[cfg(test)]
    use crate::runtime::ShutdownPhantom;
    use crate::runtime::ShutdownReceiver;
    use crate::seed::BasicSeed;
    use crate::url::guard::UrlGuardian;
    use crate::url::{UrlWithDepth, UrlWithGuard};
    use crate::web_graph::WebGraphManager;
    use std::collections::HashSet;
    use std::error::Error;
    use text_processing::stopword_registry::StopWordRegistry;
    use crate::budget::BudgetManager;
    use crate::cookies::CookieManager;

    /// A marker interface for applying the context trait iff appropriate
    pub trait ContextDelegate {}

    impl<T> BaseContext for T where T: ContextDelegate {}

    /// A context marking a context as compatible with async tasks.
    /// Can basically do nothing alone and is only a helper interface for the
    /// required interfaces.
    pub trait AsyncContext: BaseContext + Send + Sync + 'static {}

    pub trait SupportsLinkSeeding: BaseContext {
        type Error: Error + Send + Sync;

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
        type LinkStateManager: LinkStateManager;

        fn get_link_state_manager(&self) -> &Self::LinkStateManager;
    }

    /// Used when a host manager is provided
    pub trait SupportsUrlGuarding: BaseContext {
        /// The domain manager used by this
        type Guardian: UrlGuardian;

        /// Returns a reference to a [GuardedDomainManager]
        fn get_guardian(&self) -> &Self::Guardian;
    }

    pub trait SupportsBudgetManagement: BaseContext {
        type BudgetManager: BudgetManager;

        fn get_budget_manager(&self) -> &Self::BudgetManager;
    }

    pub trait SupportsCookieManagement: BaseContext {
        type CookieManager: CookieManager;

        fn get_cookie_manager(&self) -> &Self::CookieManager;
    }

    pub trait SupportsRobotsManager: BaseContext {
        /// The used robots manager
        type RobotsManager: RobotsManager;

        /// Get a reference to the robots manager.
        fn get_robots_manager(&self) -> &Self::RobotsManager;
    }

    pub trait SupportsBlackList: BaseContext {
        type BlacklistManager: BlacklistManager;

        fn get_blacklist_manager(&self) -> &Self::BlacklistManager;
    }

    pub trait SupportsMetaInfo: BaseContext {
        /// When did the crawl officially start?
        fn crawl_started_at(&self) -> time::OffsetDateTime;

        /// The amount of discovered websites.
        fn discovered_websites(&self) -> usize;
    }

    pub trait SupportsConfigs: BaseContext {
        /// Returns a reference to the config
        fn configs(&self) -> &Config;
    }

    pub trait SupportsUrlQueue: BaseContext + Send + Sync {
        /// The url queue used by this
        type UrlQueue: UrlQueue<UrlWithDepth>
            + SupportsForcedQueueElement<UrlWithDepth>
            + Send
            + Sync;

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
        fn web_graph_manager(&self) -> Option<&Self::WebGraphManager>;
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

        /// Tries to poll the next free url. Does not react to shudown.
        #[cfg(test)]
        async fn poll_next_free_url_no_shutdown<'a>(
            &'a self,
            max_miss: Option<u64>,
        ) -> UrlQueuePollResult<UrlWithGuard<'a, Self::Guardian>, Self::Error> {
            self.poll_next_free_url(ShutdownPhantom::<true>, max_miss)
                .await
        }

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

    /// A trait to support client building
    pub trait SupportsCrawling: BaseContext {
        type Client: AtraClient;

        type Error: Error + Send + Sync;

        /// Creates the crawl task for a seed.
        fn create_crawl_task<S>(&self, seed: S) -> Result<CrawlTask<S, Self::Client>, Self::Error>
        where
            S: BasicSeed;

        /// Provides an unique id for this crawl instance.
        fn create_crawl_id(&self) -> String;
    }

    pub trait SupportsDomainHandling: BaseContext {
        type DomainHandler: DomainLastCrawledManager;

        fn get_domain_manager(&self) -> &Self::DomainHandler;
    }
}
