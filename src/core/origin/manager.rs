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

use std::fmt::Debug;
use crate::core::origin::AtraUrlOrigin;
use crate::core::origin::entry::OriginEntry;
use crate::core::origin::errors::{GuardPoisonedError, OriginManagerError};
use crate::core::origin::guard::OriginGuard;
use crate::core::UrlWithDepth;

/// Basic api that is not public to the rest of the code
pub(crate) trait InternalOriginManager {
    /// Lazily releases the host
    fn release(&self, origin: AtraUrlOrigin);
}

/// A class capable of managing origins
pub trait OriginManager: InternalOriginManager + Debug + Clone {
    /// Returns a guard if the reserve was successful.
    /// Returns an error if there is the domain is already in use.
    async fn try_reserve<'a>(&'a self, url: &UrlWithDepth) -> Result<OriginGuard<'a, Self>, OriginManagerError>;

    /// Returns true if crawling this [url] provides an additional value for the host in general
    #[allow(dead_code)] async fn can_provide_additional_value(&self, url: &UrlWithDepth) -> bool;

    /// Returns true if there is an entry.
    /// Returns none if there is no host.
    async fn knows_origin(&self, url: &UrlWithDepth) -> Option<bool>;

    /// Returns something if there is a host and an entry
    async fn current_origin_state(&self, url: &UrlWithDepth) -> Option<OriginEntry>;

    /// Returns the currently reserved hosts
    async fn currently_reserved_origins(&self) -> Vec<AtraUrlOrigin>;

    /// Returns an error if the host is poisoned
    async fn check_if_poisoned<'a>(&self, guard: &OriginGuard<'a, Self>) -> Result<(), GuardPoisonedError>;
}


