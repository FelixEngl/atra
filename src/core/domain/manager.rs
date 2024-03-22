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
use case_insensitive_string::CaseInsensitiveString;
use crate::core::domain::{DomainEntry, DomainGuard, DomainManagerError, GuardPoisonedError};
use crate::core::UrlWithDepth;

/// Basic api that is not public to the rest of the code
pub(crate) trait InternalDomainManager {
    /// Lazily releases the domain
    fn release_domain(&self, domain: CaseInsensitiveString);
}

/// A class capable of managing domains
pub trait DomainManager: InternalDomainManager + Debug + Clone {
    /// Returns a guard if the reserve was successful.
    /// Returns an error if there is no domain or the domain is already used.
    async fn try_reserve_domain<'a>(&'a self, url: &UrlWithDepth) -> Result<DomainGuard<'a, Self>, DomainManagerError>;

    /// Returns true if crawling this [url] provides an additional value for the domain in general
    async fn can_provide_additional_value(&self, url: &UrlWithDepth) -> bool;

    /// Returns true if there is an entry.
    /// Returns none if there is no domain.
    async fn knows_domain(&self, url: &UrlWithDepth) -> Option<bool>;

    /// Returns something if there is a domain and an entry
    async fn current_domain_state(&self, url: &UrlWithDepth) -> Option<DomainEntry>;

    /// Returns the currently reserved domains
    async fn currently_reserved_domains(&self) -> Vec<CaseInsensitiveString>;

    /// Returns an error if the domain is poisoned
    async fn check_if_poisoned<'a>(&self, guard: &DomainGuard<'a, Self>) -> Result<(), GuardPoisonedError>;
}


