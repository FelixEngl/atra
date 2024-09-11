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

use std::sync::Arc;
use time::Duration;
use crate::client::Client;
use crate::robots::{CachedRobots, RobotsError, RobotsManager};
use crate::origin::{AtraOriginProvider, AtraUrlOrigin};
use crate::url::url_with_depth::UrlWithDepth;

/// A trait for unifying different robots information providers
pub trait RobotsInformation {
    /// Try to get the underlying robots.txt if it exists in any cache layer.
    /// Does return None if it needs a download.
    async fn get(&self, url: &UrlWithDepth) -> Result<Option<Arc<CachedRobots>>, RobotsError>;

    /// Gets the caches robots.txt.
    /// If it is not found in any layer it downloads it.
    /// If the download fails is creats a replacement with default values for a missing robots.txt
    async fn get_or_retrieve(&self, client: &Client, url: &UrlWithDepth) -> Result<Arc<CachedRobots>, RobotsError> ;

    /// Get the duration needed for the intervall between the requests.
    async fn get_or_retrieve_delay(&self, client: &Client, url: &UrlWithDepth) -> Option<Duration>;

    /// Tries to check in any of the cache-layers, if there is no cache entry or an error it returns None
    async fn check_if_allowed_fast(&self, url: &UrlWithDepth) -> Option<bool>;

    /// Tries to check in any of the cache-layers, if there is an error it returns false
    async fn check_if_allowed(&self, client: &Client, url: &UrlWithDepth) -> bool;
}

pub enum AnyRobotsInformation<R: RobotsManager> {
    Origin(OriginSpecificRobotsInformation<R>),
    General(GeneralRobotsInformation<R>)
}

impl<R: RobotsManager> RobotsInformation for AnyRobotsInformation<R> {
    #[doc = " Try to get the underlying robots.txt if it exists in any cache layer."] 
    #[doc = " Does return None if it needs a download."]  
    #[inline]   
    async fn get(&self, url: &UrlWithDepth) -> Result<Option<Arc<CachedRobots>>, RobotsError> {  
        match self {
            AnyRobotsInformation::Origin(a) => { a.get(url).await   }
            AnyRobotsInformation::General(b) => { b.get(url).await   }
        }
    }
    #[doc = " Gets the caches robots.txt."]
    #[doc = " If it is not found in any layer it downloads it."]
    #[doc = " If the download fails is creats a replacement with default values for a missing robots.txt"]
    #[inline]
    async fn get_or_retrieve(&self, client: &Client, url: &UrlWithDepth) -> Result<Arc<CachedRobots>, RobotsError> {
        match self {
            AnyRobotsInformation::Origin(a) => { a.get_or_retrieve(client, url).await   }
            AnyRobotsInformation::General(b) => { b.get_or_retrieve(client, url).await   }
        }
    }
    #[doc = " Get the duration needed for the intervall between the requests."]
    #[inline]
    async fn get_or_retrieve_delay(&self, client: &Client, url: &UrlWithDepth) -> Option<Duration> {
        match self {
            AnyRobotsInformation::Origin(a) => { a.get_or_retrieve_delay(client, url).await   }
            AnyRobotsInformation::General(b) => { b.get_or_retrieve_delay(client, url).await   }
        }
    }
    #[doc = " Tries to check in any of the cache-layers, if there is no cache entry or an error it returns None"]
    #[inline]
    async fn check_if_allowed_fast(&self, url: &UrlWithDepth) -> Option<bool> {
        match self {
            AnyRobotsInformation::Origin(a) => { a.check_if_allowed_fast(url).await   }
            AnyRobotsInformation::General(b) => { b.check_if_allowed_fast(url).await   }
        }
    }
    #[doc = " Tries to check in any of the cache-layers, if there is an error it returns false"]
    #[inline]
    async fn check_if_allowed(&self, client: &Client, url: &UrlWithDepth) -> bool {
        match self {
            AnyRobotsInformation::Origin(a) => { a.check_if_allowed(client, url).await   }
            AnyRobotsInformation::General(b) => { b.check_if_allowed(client, url).await   }
        }
    }
}

/// Same as [GeneralRobotsInformation] but is bound to a specific domain
pub struct OriginSpecificRobotsInformation<R: RobotsManager> {
    origin: AtraUrlOrigin,
    origin_cached: Arc<CachedRobots>,
    general: GeneralRobotsInformation<R>
}

// impl<R: RobotsManager> DomainSpecificRobotsInformation<R> {
//     pub fn into_inner(self) -> GeneralRobotsInformation<R> {
//         self.general
//     }
// }

impl<R: RobotsManager> RobotsInformation for OriginSpecificRobotsInformation<R> {
    async fn get(&self, url: &UrlWithDepth) -> Result<Option<Arc<CachedRobots>>, RobotsError> {
        if let Some(origin) = url.atra_origin() {
            if origin == self.origin {
                log::trace!("Robots: Fast");
                return Ok(Some(self.origin_cached.clone()))
            }
        }
        self.general.get(url).await
    }

    async fn get_or_retrieve(&self, client: &Client, url: &UrlWithDepth) -> Result<Arc<CachedRobots>, RobotsError> {
        if let Some(origin) = url.atra_origin() {
            if origin == self.origin {
                log::trace!("Robots: Fast");
                return Ok(self.origin_cached.clone())
            }
        }
        self.general.get_or_retrieve(client, url).await
    }

    async fn get_or_retrieve_delay(&self, client: &Client, url: &UrlWithDepth) -> Option<Duration> {
        if let Some(origin) = url.atra_origin() {
            if origin == self.origin {
                log::trace!("Robots: Fast");
                return self.origin_cached.delay()
            }
        }
        self.general.get_or_retrieve_delay(client, url).await
    }

    async fn check_if_allowed_fast(&self, url: &UrlWithDepth) -> Option<bool> {
        if let Some(origin) = url.atra_origin() {
            if origin == self.origin {
                log::trace!("Robots: Fast");
                return Some(self.origin_cached.allowed(&url.as_str()))
            }
        }
        self.general.check_if_allowed_fast(url).await
    }

    async fn check_if_allowed(&self, client: &Client, url: &UrlWithDepth) -> bool {
        if let Some(origin) = url.atra_origin() {
            if origin == self.origin {
                log::trace!("Robots: Fast");
                return self.origin_cached.allowed(&url.as_str())
            }
        }
        self.general.check_if_allowed(client, url).await
    }
}

/// A wrapper for ShareableRobotsManager with some config.
/// Also holds a persistent, possibly endless amount of cached robots.txt instances.
/// Should only be used internally and dropped after use.
#[derive(Debug)]
pub struct GeneralRobotsInformation<R: RobotsManager> {
    inner: R,
    agent: String,
    max_age: Option<Duration>,
}

impl<R: RobotsManager> GeneralRobotsInformation<R> {

    pub fn new(
        inner: R,
        agent: String,
        max_age: Option<Duration>
    ) -> Self {
        Self {
            inner,
            agent,
            max_age,
        }
    }

    // pub fn into_inner(self) -> R {
    //     return self.inner
    // }

    pub async fn bind_to_domain(self, client: &Client, url: &UrlWithDepth) -> AnyRobotsInformation<R> {
        let domain = match url.atra_origin() {
            None => {
                log::debug!("No domain for for {url}");
                return AnyRobotsInformation::General(self)
            }
            Some(found) => {found}
        };
        match self.get_or_retrieve(client, url).await {
            Ok(domain_cached) => {
                AnyRobotsInformation::Origin(OriginSpecificRobotsInformation {
                    origin_cached: domain_cached,
                    general: self,
                    origin: domain
                })
            }
            Err(err) => {
                log::debug!("Failed to retrieve the robots.txt for {url} with {err}");
                AnyRobotsInformation::General(self)
            }
        }


    }
}

impl<R: RobotsManager> RobotsInformation for GeneralRobotsInformation<R> {


    /// Try to get the underlying robots.txt if it exists in any cache layer.
    /// Does return None if it needs a download.
    async fn get(&self, url: &UrlWithDepth) -> Result<Option<Arc<CachedRobots>>, RobotsError> {
        if let Some(ref value) = self.max_age {
            self.inner.get(self.agent.as_str(), url, Some(value)).await
        } else {
            self.inner.get(self.agent.as_str(), url, None).await
        }
    }

    /// Gets the caches robots.txt.
    /// If it is not found in any layer it downloads it.
    /// If the download fails is creats a replacement with default values for a missing robots.txt
    async fn get_or_retrieve(&self, client: &Client, url: &UrlWithDepth) -> Result<Arc<CachedRobots>, RobotsError> {
        if let Some(ref value) = self.max_age {
            self.inner.get_or_retrieve(client, self.agent.as_str(), url, Some(value)).await
        } else {
            self.inner.get_or_retrieve(client, self.agent.as_str(), url, None).await
        }
    }

    /// Get the duration needed for the intervall between the requests.
    async fn get_or_retrieve_delay(&self, client: &Client, url: &UrlWithDepth) -> Option<Duration> {
        match self.get_or_retrieve(client, url).await {
            Ok(found) => {found.delay()}
            Err(_) => {
                log::trace!("RobotsTXT: No Delay for {}", url);
                None
            }
        }
    }

    /// Tries to check in any of the cache-layers, if there is no cache entry or an error it returns None
    async fn check_if_allowed_fast(&self, url: &UrlWithDepth) -> Option<bool> {
        let found = self.get(url).await.ok().flatten();
        found.map(|found| found.allowed(&url.as_str()))
    }

    /// Tries to check in any of the cache-layers, if there is an error it returns false
    async fn check_if_allowed(&self, client: &Client, url: &UrlWithDepth) -> bool {
        match self.get_or_retrieve(client, url).await.map(|found| found.allowed(&url.as_str())) {
            Ok(result) => {result}
            Err(err) => {
                log::trace!("Failed robots check: {}", err);
                false
            }
        }
    }
}