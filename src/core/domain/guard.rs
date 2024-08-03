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

use std::fmt;
use std::marker::PhantomData;
use std::time::SystemTime;
use case_insensitive_string::CaseInsensitiveString;
use crate::core::depth::DepthDescriptor;
use crate::core::domain::{DomainEntry, DomainManager, GuardPoisonedError};
use crate::core::UrlWithDepth;

/// A guard that works basically like a Mutex or RwLock guard.
/// Allows to block a domain until the guard is dropped.
#[clippy::has_significant_drop]
pub struct DomainGuard<'a, T: DomainManager> {
    pub(crate) reserved_at: SystemTime,
    pub(crate) domain: CaseInsensitiveString,
    pub(crate) domain_manager: *const T,
    pub(crate) domain_entry: DomainEntry,
    pub(crate) _marker: PhantomData<&'a T>
}

unsafe impl<'a, T: DomainManager> Sync for DomainGuard<'a, T>{}
unsafe impl<'a, T: DomainManager> Send for DomainGuard<'a, T>{}

impl<'a, T: DomainManager> fmt::Debug for DomainGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DomainGuard")
            .field("reserved_at", &self.reserved_at)
            .field("domain", &self.domain)
            .field("domain_entry", &self.domain_entry)
            .finish_non_exhaustive()
    }
}

#[allow(dead_code)]
impl<'a, T: DomainManager> DomainGuard<'a, T> {

    /// Checks the guard is poisoned.
    pub async fn check_for_poison(&self) -> Result<(), GuardPoisonedError> {
        unsafe{&*self.domain_manager}.check_if_poisoned(self).await
    }

    /// When was the guard reserved?
    pub fn reserved_at(&self) -> SystemTime {
        self.reserved_at
    }

    /// What is the associated domain?
    pub fn domain(&self) -> &CaseInsensitiveString {
        &self.domain
    }

    /// Returns true iff the domain of [url] is protected.
    /// If there is no url it returns none.
    pub fn url_has_protected_domain(&self, url: &UrlWithDepth) -> Option<bool> {
        url.domain().map(|value| value == self.domain)
    }

    /// Returns the domain entry
    pub fn domain_entry(&self) -> &DomainEntry {
        &self.domain_entry
    }

    /// Returns the depth associated to the domain guard.
    pub fn depth(&self) -> DepthDescriptor {
        self.domain_entry.depth
    }

    /// Returns true if the url has some kind of potential to add additional value to the crawl.
    pub fn has_additional_value(&self, url: &UrlWithDepth) -> bool {
        url.depth() < &self.domain_entry.depth
    }
}

impl<'a, T: DomainManager> Drop for DomainGuard<'a, T> {
    fn drop(&mut self) {
        unsafe{&*self.domain_manager}.release_domain(self.domain.clone());
    }
}