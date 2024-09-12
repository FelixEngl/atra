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

use crate::seed::error::SeedCreationError;
use crate::seed::unguarded::UnguardedSeed;
use crate::seed::BasicSeed;
use crate::url::guard::{UrlGuard, UrlGuardian};
use crate::url::AtraUrlOrigin;
use crate::url::{AtraOriginProvider, UrlWithDepth};

/// A guarded version, it is keeping the guard for the host information.
/// The lifetime depends on
pub struct GuardedSeed<'a, 'guard: 'a, T: UrlGuardian> {
    host_guard: &'a UrlGuard<'guard, T>,
    url: &'a UrlWithDepth,
}

impl<'a, 'guard: 'a, T: UrlGuardian> GuardedSeed<'a, 'guard, T> {
    /// Creates a guarded seed from a guard and a url
    pub fn new(
        host_guard: &'a UrlGuard<'guard, T>,
        url: &'a UrlWithDepth,
    ) -> Result<Self, SeedCreationError> {
        if let Some(host) = url.atra_origin() {
            if host.eq(host_guard.origin()) {
                Ok(unsafe { Self::new_unchecked(host_guard, url) })
            } else {
                Err(SeedCreationError::GuardAndUrlDifferInOrigin {
                    origin_from_url: host.clone(),
                    origin_from_guard: host_guard.origin().to_owned(),
                })
            }
        } else {
            Err(SeedCreationError::NoOrigin)
        }
    }

    /// Creates the new url but does not do any host to guard checks.
    pub unsafe fn new_unchecked(
        host_guard: &'a UrlGuard<'guard, T>,
        url: &'a UrlWithDepth,
    ) -> Self {
        Self { host_guard, url }
    }

    /// Removes the dependency from the guard.
    pub fn unguard(self) -> UnguardedSeed {
        let origin = self.origin().to_owned();
        unsafe { UnguardedSeed::new_unchecked(self.url.clone(), origin) }
    }
}

impl<'a, 'guard: 'a, T: UrlGuardian> BasicSeed for GuardedSeed<'a, 'guard, T> {
    #[inline]
    fn url(&self) -> &UrlWithDepth {
        self.url
    }

    #[inline]
    fn origin(&self) -> &AtraUrlOrigin {
        &self.host_guard.origin()
    }
}

impl<'a, 'guard: 'a, T: UrlGuardian> AsRef<UrlWithDepth> for GuardedSeed<'a, 'guard, T> {
    #[inline]
    fn as_ref(&self) -> &UrlWithDepth {
        self.url()
    }
}

impl<'a, 'guard: 'a, T: UrlGuardian> AsRef<AtraUrlOrigin> for GuardedSeed<'a, 'guard, T> {
    #[inline]
    fn as_ref(&self) -> &AtraUrlOrigin {
        self.origin()
    }
}
