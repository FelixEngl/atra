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

use case_insensitive_string::CaseInsensitiveString;
use crate::core::crawl::errors::SeedCreationError;
use crate::core::crawl::errors::SeedCreationError::*;
use crate::core::domain::{DomainGuard, DomainManager};
use crate::core::UrlWithDepth;


/// The seed of a crawl task
pub trait CrawlSeed {
    /// A reference to the url
    fn url(&self) -> &UrlWithDepth;

    /// A reference to the domain
    fn domain(&self) -> &CaseInsensitiveString;
}



/// A guarded version, it is keeping the guard for the domain information.
/// The lifetime depends on
pub struct GuardedSeed<'a, 'guard: 'a, T: DomainManager> {
    domain_guard: &'a DomainGuard<'guard, T>,
    url: &'a UrlWithDepth
}

impl<'a, 'guard: 'a, T: DomainManager> GuardedSeed<'a, 'guard, T> {

    /// Creates a guarded seed from a guard and a url
    #[allow(dead_code)] pub fn new(domain_guard: &'a DomainGuard<'guard, T>, url: &'a UrlWithDepth) -> Result<Self, SeedCreationError> {
        if let Some(domain) = url.domain() {
            if domain.eq(domain_guard.domain()) {
                Ok(unsafe{Self::new_unchecked(domain_guard, url)})
            } else {
                Err(GuardAndUrlDifferInDomain {
                    domain_from_url: domain.inner().clone(),
                    domain_from_guard: domain_guard.domain().inner().clone()
                })
            }
        } else {
            Err(NoDomain)
        }
    }

    /// Creates the new url but does not do any domain to guard checks.
    pub unsafe fn new_unchecked(domain_guard: &'a DomainGuard<'guard, T>, url: &'a UrlWithDepth) -> Self {
        Self {
            domain_guard,
            url
        }
    }

    /// Removes the dependency from the guard.
    #[allow(dead_code)] pub fn unguard(self) -> UnguardedSeed {
        let domain = self.domain().clone();
        unsafe { UnguardedSeed::new_unchecked(self.url.clone(), domain) }
    }
}

impl<'a, 'guard: 'a, T: DomainManager> CrawlSeed for GuardedSeed<'a, 'guard, T> {
    #[inline] fn url(&self) -> &UrlWithDepth {
        self.url
    }

    #[inline] fn domain(&self) -> &CaseInsensitiveString {
        &self.domain_guard.domain()
    }
}

impl<'a, 'guard: 'a, T: DomainManager> AsRef<UrlWithDepth> for GuardedSeed<'a, 'guard, T> {
    #[inline] fn as_ref(&self) -> &UrlWithDepth {
        self.url()
    }
}

impl<'a, 'guard: 'a, T: DomainManager> AsRef<CaseInsensitiveString> for GuardedSeed<'a, 'guard, T> {
    #[inline] fn as_ref(&self) -> &CaseInsensitiveString {
        self.domain()
    }
}

/// An unguarded version when no guarding is needed
pub struct UnguardedSeed {
    url: UrlWithDepth,
    domain: CaseInsensitiveString
}


impl UnguardedSeed {

    /// Creates a new UnguardedSeed for a [url] and an associated [domain].
    #[allow(dead_code)] pub fn new(url: UrlWithDepth, domain: CaseInsensitiveString) -> Result<UnguardedSeed, SeedCreationError> {
        if let Some(domain_url) = url.domain() {
            if domain.eq(&domain_url) {
                Ok(unsafe {Self::new_unchecked(url, domain)})
            } else {
                Err(
                    GuardAndUrlDifferInDomain {
                        domain_from_url: domain_url.inner().clone(),
                        domain_from_guard: domain.inner().clone()
                    }
                )
            }
        } else {
            Err(NoDomain)
        }
    }

    /// Creates the seed but omits the domain checks.
    /// You have to make sure yourself, that the contract is valid.
    pub unsafe fn new_unchecked(
        url: UrlWithDepth,
        domain: CaseInsensitiveString
    ) -> Self {
        Self {
            url,
            domain
        }
    }

    /// Provides a guarded version of the unguarded seed iff the domain is the same.
    #[allow(dead_code)] pub fn guard<'a, 'guard: 'a, T: DomainManager>(&'a self, guard: &'a DomainGuard<'guard, T>) -> GuardedSeed<'a, 'guard, T> {
        unsafe{GuardedSeed::new_unchecked(guard, &self.url)}
    }
}

impl CrawlSeed for UnguardedSeed {
    fn url(&self) -> &UrlWithDepth {
        &self.url
    }

    fn domain(&self) -> &CaseInsensitiveString {
        &self.domain
    }
}


impl TryFrom<UrlWithDepth> for UnguardedSeed {
    type Error = SeedCreationError;

    fn try_from(value: UrlWithDepth) -> Result<Self, Self::Error> {
        let domain = value.domain().ok_or(NoDomain)?;
        Ok(unsafe {Self::new_unchecked(value, domain)})
    }
}

impl AsRef<UrlWithDepth> for UnguardedSeed {
    #[inline] fn as_ref(&self) -> &UrlWithDepth {
        self.url()
    }
}

impl AsRef<CaseInsensitiveString> for UnguardedSeed {
    #[inline] fn as_ref(&self) -> &CaseInsensitiveString {
        self.domain()
    }
}