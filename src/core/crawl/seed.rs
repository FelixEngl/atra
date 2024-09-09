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

use crate::core::crawl::errors::SeedCreationError;
use crate::core::crawl::errors::SeedCreationError::*;
use crate::core::origin::{AtraOriginProvider, OriginManager};
use crate::core::origin::AtraUrlOrigin;
use crate::core::origin::guard::OriginGuard;
use crate::core::UrlWithDepth;


/// The seed of a crawl task
pub trait CrawlSeed {
    /// A reference to the url
    fn url(&self) -> &UrlWithDepth;

    /// A reference to the host
    fn origin(&self) -> &AtraUrlOrigin;
}



/// A guarded version, it is keeping the guard for the host information.
/// The lifetime depends on
pub struct GuardedSeed<'a, 'guard: 'a, T: OriginManager> {
    host_guard: &'a OriginGuard<'guard, T>,
    url: &'a UrlWithDepth
}

impl<'a, 'guard: 'a, T: OriginManager> GuardedSeed<'a, 'guard, T> {

    /// Creates a guarded seed from a guard and a url
    #[allow(dead_code)] pub fn new(host_guard: &'a OriginGuard<'guard, T>, url: &'a UrlWithDepth) -> Result<Self, SeedCreationError> {
        if let Some(host) = url.atra_origin() {
            if host.eq(host_guard.origin()) {
                Ok(unsafe{Self::new_unchecked(host_guard, url)})
            } else {
                Err(GuardAndUrlDifferInOrigin {
                    origin_from_url: host.clone(),
                    origin_from_guard: host_guard.origin().to_owned()
                })
            }
        } else {
            Err(NoOrigin)
        }
    }

    /// Creates the new url but does not do any host to guard checks.
    pub unsafe fn new_unchecked(host_guard: &'a OriginGuard<'guard, T>, url: &'a UrlWithDepth) -> Self {
        Self {
            host_guard,
            url
        }
    }

    /// Removes the dependency from the guard.
    #[allow(dead_code)] pub fn unguard(self) -> UnguardedSeed {
        let origin = self.origin().to_owned();
        unsafe { UnguardedSeed::new_unchecked(self.url.clone(), origin) }
    }
}

impl<'a, 'guard: 'a, T: OriginManager> CrawlSeed for GuardedSeed<'a, 'guard, T> {
    #[inline] fn url(&self) -> &UrlWithDepth {
        self.url
    }

    #[inline] fn origin(&self) -> &AtraUrlOrigin {
        &self.host_guard.origin()
    }
}

impl<'a, 'guard: 'a, T: OriginManager> AsRef<UrlWithDepth> for GuardedSeed<'a, 'guard, T> {
    #[inline] fn as_ref(&self) -> &UrlWithDepth {
        self.url()
    }
}

impl<'a, 'guard: 'a, T: OriginManager> AsRef<AtraUrlOrigin> for GuardedSeed<'a, 'guard, T> {
    #[inline] fn as_ref(&self) -> &AtraUrlOrigin {
        self.origin()
    }
}

/// An unguarded version when no guarding is needed
pub struct UnguardedSeed {
    url: UrlWithDepth,
    origin: AtraUrlOrigin
}


impl UnguardedSeed {

    /// Creates a new UnguardedSeed for a [url] and an associated [host].
    pub fn new(url: UrlWithDepth, origin: AtraUrlOrigin) -> Result<UnguardedSeed, SeedCreationError> {
        if let Some(url_origin) = url.atra_origin() {
            if origin.eq(&url_origin) {
                Ok(unsafe {Self::new_unchecked(url, origin)})
            } else {
                Err(
                    GuardAndUrlDifferInOrigin {
                        origin_from_url: url_origin.clone(),
                        origin_from_guard: origin.clone()
                    }
                )
            }
        } else {
            Err(NoOrigin)
        }
    }

    /// Creates the seed but omits the host checks.
    /// You have to make sure yourself, that the contract is valid.
    pub unsafe fn new_unchecked(
        url: UrlWithDepth,
        origin: AtraUrlOrigin
    ) -> Self {
        Self {
            url,
            origin
        }
    }

    /// Provides a guarded version of the unguarded seed iff the host is the same.
    #[allow(dead_code)] pub fn guard<'a, 'guard: 'a, T: OriginManager>(&'a self, guard: &'a OriginGuard<'guard, T>) -> GuardedSeed<'a, 'guard, T> {
        unsafe{GuardedSeed::new_unchecked(guard, &self.url)}
    }
}

impl CrawlSeed for UnguardedSeed {
    fn url(&self) -> &UrlWithDepth {
        &self.url
    }

    fn origin(&self) -> &AtraUrlOrigin {
        &self.origin
    }
}


impl TryFrom<UrlWithDepth> for UnguardedSeed {
    type Error = SeedCreationError;

    fn try_from(value: UrlWithDepth) -> Result<Self, Self::Error> {
        let host = value.atra_origin().ok_or(NoOrigin)?;
        Ok(unsafe {Self::new_unchecked(value, host)})
    }
}

impl AsRef<UrlWithDepth> for UnguardedSeed {
    #[inline] fn as_ref(&self) -> &UrlWithDepth {
        self.url()
    }
}

impl AsRef<AtraUrlOrigin> for UnguardedSeed {
    #[inline] fn as_ref(&self) -> &AtraUrlOrigin {
        self.origin()
    }
}