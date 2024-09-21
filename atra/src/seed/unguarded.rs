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
use crate::seed::BasicSeed;
use crate::url::AtraOriginProvider;
use crate::url::AtraUrlOrigin;
use crate::url::UrlWithDepth;

/// An unguarded version when no guarding is needed
#[derive(Clone, Debug)]
pub struct UnguardedSeed {
    url: UrlWithDepth,
    origin: AtraUrlOrigin,
    is_seed: bool,
}

impl UnguardedSeed {
    /// Creates a new UnguardedSeed for a [url] and an associated [host].
    pub fn new(
        url: UrlWithDepth,
        origin: AtraUrlOrigin,
        is_seed: bool,
    ) -> Result<UnguardedSeed, SeedCreationError> {
        if let Some(url_origin) = url.atra_origin() {
            if origin.eq(&url_origin) {
                Ok(unsafe { Self::new_unchecked(url, origin, is_seed) })
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
    pub unsafe fn new_unchecked(url: UrlWithDepth, origin: AtraUrlOrigin, is_seed: bool) -> Self {
        Self {
            url,
            origin,
            is_seed,
        }
    }

    #[cfg(test)]
    pub fn from_url<S: AsRef<str>>(value: S) -> Result<UnguardedSeed, SeedCreationError> {
        let url: UrlWithDepth = value.as_ref().parse().unwrap();
        let data = url.atra_origin().unwrap();
        Self::new(url, data, false)
    }
}

impl BasicSeed for UnguardedSeed {
    fn url(&self) -> &UrlWithDepth {
        &self.url
    }

    fn origin(&self) -> &AtraUrlOrigin {
        &self.origin
    }

    fn is_original_seed(&self) -> bool {
        self.is_seed
    }

    #[cfg(test)]
    fn create_unguarded(&self) -> UnguardedSeed {
        self.clone()
    }
}

impl TryFrom<UrlWithDepth> for UnguardedSeed {
    type Error = SeedCreationError;

    fn try_from(value: UrlWithDepth) -> Result<Self, Self::Error> {
        let host = value.atra_origin().ok_or(SeedCreationError::NoOrigin)?;
        Ok(unsafe { Self::new_unchecked(value, host, false) })
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
