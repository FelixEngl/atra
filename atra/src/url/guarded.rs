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

use crate::seed::UnguardedSeed;
use crate::url::guard::{UrlGuard, UrlGuardian};
use crate::url::UrlWithDepth;
use cfg_if::cfg_if;
cfg_if! {
    if #[cfg(test)] {
        use crate::seed::{GuardedSeed, SeedCreationError};
        use crate::url::AtraOriginProvider;
    }
}

/// A guard with an associated seed url
pub struct UrlWithGuard<'a, T: UrlGuardian> {
    guard: UrlGuard<'a, T>,
    seed_url: UrlWithDepth,
}

impl<'a, T: UrlGuardian> UrlWithGuard<'a, T> {
    /// Creates a DomainGuardWithSeed but asserts that the seed creation can wor beforehand.
    #[cfg(test)]
    pub fn new(guard: UrlGuard<'a, T>, seed_url: UrlWithDepth) -> Result<Self, SeedCreationError> {
        if let Some(host) = seed_url.atra_origin() {
            if guard.origin().eq(&host) {
                Ok(unsafe { Self::new_unchecked(guard, seed_url) })
            } else {
                Err(SeedCreationError::GuardAndUrlDifferInOrigin {
                    origin_from_url: host.clone(),
                    origin_from_guard: guard.origin().clone(),
                })
            }
        } else {
            Err(SeedCreationError::NoOrigin)
        }
    }

    /// Creates a DomainGuardWithSeed without doing any domain checks.
    pub unsafe fn new_unchecked(guard: UrlGuard<'a, T>, seed_url: UrlWithDepth) -> Self {
        Self { guard, seed_url }
    }

    /// Returns the domain guard
    #[cfg(test)]
    pub fn guard(&self) -> &UrlGuard<'a, T> {
        &self.guard
    }

    /// Returns the seed url
    #[cfg(test)]
    pub fn seed_url(&self) -> &UrlWithDepth {
        &self.seed_url
    }

    /// Returns a guarded seed instance
    #[cfg(test)]
    pub fn get_guarded_seed<'b>(&'b self) -> GuardedSeed<'b, 'a, T> {
        unsafe { GuardedSeed::new_unchecked(&self.guard, &self.seed_url) }
    }

    pub fn get_unguarded_seed(&self) -> UnguardedSeed {
        unsafe { UnguardedSeed::new_unchecked(self.seed_url.clone(), self.guard.origin().clone()) }
    }
}
