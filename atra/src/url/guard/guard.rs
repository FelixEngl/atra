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

use crate::url::guard::entry::GuardEntry;
use crate::url::guard::{GuardPoisonedError, UrlGuardian};
use crate::url::AtraOriginProvider;
use crate::url::AtraUrlOrigin;
use crate::url::Depth;
use crate::url::UrlWithDepth;
use std::fmt;
use std::marker::PhantomData;
use std::time::SystemTime;

/// A guard that works basically like a Mutex or RwLock guard.
/// Allows to block a domain until the guard is dropped.
#[clippy::has_significant_drop]
pub struct UrlGuard<'a, Guardian>
where
    Guardian: UrlGuardian,
{
    pub(super) reserved_at: SystemTime,
    pub(super) origin: AtraUrlOrigin,
    pub(super) origin_manager: *const Guardian,
    pub(super) entry: GuardEntry,
    pub(super) _marker: PhantomData<&'a Guardian>,
}

unsafe impl<'a, Guardian: UrlGuardian> Sync for UrlGuard<'a, Guardian> where Guardian: UrlGuardian {}
unsafe impl<'a, Guardian: UrlGuardian> Send for UrlGuard<'a, Guardian> where Guardian: UrlGuardian {}

impl<'a, Guardian> fmt::Debug for UrlGuard<'a, Guardian>
where
    Guardian: UrlGuardian,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HostGuard")
            .field("reserved_at", &self.reserved_at)
            .field("origin", &self.origin)
            .field("entry", &self.entry)
            .finish_non_exhaustive()
    }
}

#[allow(dead_code)]
impl<'a, Guardian> UrlGuard<'a, Guardian>
where
    Guardian: UrlGuardian,
{
    /// Checks the guard is poisoned.
    pub async fn check_for_poison(&self) -> Result<(), GuardPoisonedError> {
        unsafe { &*self.origin_manager }
            .check_if_poisoned(self)
            .await
    }

    /// When was the guard reserved?
    pub fn reserved_at(&self) -> SystemTime {
        self.reserved_at
    }

    /// What is the associated origin?
    pub fn origin(&self) -> &AtraUrlOrigin {
        &self.origin
    }

    /// Returns true iff the domain of [url] is protected.
    /// If there is no url it returns none.
    pub fn url_has_protected_origin(&self, url: &UrlWithDepth) -> Option<bool> {
        url.atra_origin().map(|value| value == self.origin)
    }

    /// Returns the domain entry
    pub fn entry(&self) -> &GuardEntry {
        &self.entry
    }

    /// Returns the depth associated to the domain guard.
    pub fn depth(&self) -> Depth {
        self.entry.depth
    }

    /// Returns true if the url has some kind of potential to add additional value to the crawl.
    pub fn has_additional_value(&self, url: &UrlWithDepth) -> bool {
        url.depth() < &self.entry.depth
    }
}

impl<'a, Guardian> Drop for UrlGuard<'a, Guardian>
where
    Guardian: UrlGuardian,
{
    fn drop(&mut self) {
        unsafe {
            (&*self.origin_manager).release(self.origin.clone());
        }
    }
}
