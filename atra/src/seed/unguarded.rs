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
use crate::seed::guarded::GuardedSeed;
use crate::seed::BasicSeed;
use crate::url::guard::{UrlGuard, UrlGuardian};
use crate::url::AtraOriginProvider;
use crate::url::AtraUrlOrigin;
use crate::url::UrlWithDepth;

/// An unguarded version when no guarding is needed
pub struct UnguardedSeed {
    url: UrlWithDepth,
    origin: AtraUrlOrigin,
}

impl UnguardedSeed {
    /// Creates a new UnguardedSeed for a [url] and an associated [host].
    pub fn new(
        url: UrlWithDepth,
        origin: AtraUrlOrigin,
    ) -> Result<UnguardedSeed, SeedCreationError> {
        if let Some(url_origin) = url.atra_origin() {
            if origin.eq(&url_origin) {
                Ok(unsafe { Self::new_unchecked(url, origin) })
            } else {
                Err(SeedCreationError::GuardAndUrlDifferInOrigin {
                    origin_from_url: url_origin.clone(),
                    origin_from_guard: origin.clone(),
                })
            }
        } else {
            Err(SeedCreationError::NoOrigin)
        }
    }

    /// Creates the seed but omits the host checks.
    /// You have to make sure yourself, that the contract is valid.
    pub unsafe fn new_unchecked(url: UrlWithDepth, origin: AtraUrlOrigin) -> Self {
        Self { url, origin }
    }

    /// Provides a guarded version of the unguarded seed iff the host is the same.
    pub fn guard<'a, 'guard: 'a, T: UrlGuardian>(
        &'a self,
        guard: &'a UrlGuard<'guard, T>,
    ) -> GuardedSeed<'a, 'guard, T> {
        unsafe { GuardedSeed::new_unchecked(guard, &self.url) }
    }
}

impl BasicSeed for UnguardedSeed {
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
        let host = value.atra_origin().ok_or(SeedCreationError::NoOrigin)?;
        Ok(unsafe { Self::new_unchecked(value, host) })
    }
}

impl AsRef<UrlWithDepth> for UnguardedSeed {
    #[inline]
    fn as_ref(&self) -> &UrlWithDepth {
        self.url()
    }
}

impl AsRef<AtraUrlOrigin> for UnguardedSeed {
    #[inline]
    fn as_ref(&self) -> &AtraUrlOrigin {
        self.origin()
    }
}
