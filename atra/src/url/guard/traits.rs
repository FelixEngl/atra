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
use crate::url::guard::{GuardPoisonedError, GuardianError, UrlGuard};
use crate::url::AtraUrlOrigin;
use crate::url::UrlWithDepth;
use std::fmt::Debug;

/// Basic api that is not public to the rest of the code
pub unsafe trait UnsafeUrlGuardian {
    /// Lazily releases the host. This code may be unsafe or cause
    /// unforeseen crashes when not handled properly.
    ///
    /// This method is ONLY called when a guard is released. (see [super::UrlGuard])
    unsafe fn release(&self, origin: AtraUrlOrigin);
}

/// A class capable of managing origins
#[allow(dead_code)]
pub trait UrlGuardian: UnsafeUrlGuardian + Debug + Clone {
    /// Returns a guard if the reserve was successful.
    /// Returns an error if there is the domain is already in use.
    async fn try_reserve<'a>(
        &'a self,
        url: &UrlWithDepth,
    ) -> Result<UrlGuard<'a, Self>, GuardianError>;

    /// Returns true if crawling this [url] provides an additional value for the host in general
    async fn can_provide_additional_value(&self, url: &UrlWithDepth) -> bool;

    /// Returns true if there is an entry.
    /// Returns none if there is no host.
    async fn knows_origin(&self, url: &UrlWithDepth) -> Option<bool>;

    /// Returns something if there is a host and an entry
    async fn current_origin_state(&self, url: &UrlWithDepth) -> Option<GuardEntry>;

    /// Returns the currently reserved hosts
    async fn currently_reserved_origins(&self) -> Vec<AtraUrlOrigin>;

    /// Returns an error if the host is poisoned
    async fn check_if_poisoned<'a>(
        &self,
        guard: &UrlGuard<'a, Self>,
    ) -> Result<(), GuardPoisonedError>;
}
