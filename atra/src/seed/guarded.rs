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
#[cfg(test)]
use crate::seed::unguarded::UnguardedSeed;
use crate::seed::BasicSeed;
use crate::url::guard::UrlGuardian;
use crate::url::{AtraOriginProvider, UrlWithDepth};
use crate::url::{AtraUrlOrigin, UrlWithGuard};
use std::mem::transmute;

/// A guarded version, it is keeping the guard for the host information.
/// The lifetime depends on
#[repr(transparent)]
pub struct GuardedSeed<'guard, Guardian>
where
    Guardian: UrlGuardian + 'static,
{
    url_with_guard: &'guard UrlWithGuard<'static, Guardian>,
}

unsafe impl<'guard, Guardian> Send for GuardedSeed<'guard, Guardian> where
    Guardian: UrlGuardian + 'static
{
}
unsafe impl<'guard, Guardian> Sync for GuardedSeed<'guard, Guardian> where
    Guardian: UrlGuardian + 'static
{
}

impl<'guard, Guardian> GuardedSeed<'guard, Guardian>
where
    Guardian: UrlGuardian + 'static,
{
    /// Creates a guarded seed from a guard and a url
    pub fn new<'a>(
        url_with_guard: &'guard UrlWithGuard<'a, Guardian>,
    ) -> Result<Self, SeedCreationError>
    where
        'guard: 'a,
    {
        if let Some(host) = url_with_guard.seed_url().atra_origin() {
            if host.eq(url_with_guard.guard().origin()) {
                Ok(unsafe { Self::new_unchecked(url_with_guard) })
            } else {
                Err(SeedCreationError::GuardAndUrlDifferInOrigin {
                    origin_from_url: host.clone(),
                    origin_from_guard: url_with_guard.guard().origin().to_owned(),
                })
            }
        } else {
            Err(SeedCreationError::NoOrigin)
        }
    }

    /// Creates the new url but does not do any host to guard checks.
    pub unsafe fn new_unchecked<'a>(url_with_guard: &'guard UrlWithGuard<'a, Guardian>) -> Self
    where
        'guard: 'a,
    {
        Self {
            url_with_guard: unsafe { transmute(url_with_guard) },
        }
    }
}

impl<'guard, Guardian> BasicSeed for GuardedSeed<'guard, Guardian>
where
    Guardian: UrlGuardian + 'static,
{
    #[inline]
    fn url(&self) -> &UrlWithDepth {
        self.url_with_guard.seed_url()
    }

    #[inline]
    fn origin(&self) -> &AtraUrlOrigin {
        self.url_with_guard.guard().origin()
    }

    fn is_original_seed(&self) -> bool {
        self.url_with_guard.is_seed()
    }

    #[cfg(test)]
    #[inline]
    fn create_unguarded(&self) -> UnguardedSeed {
        self.url_with_guard.get_unguarded_seed()
    }
}

impl<'guard, Guardian> AsRef<UrlWithDepth> for GuardedSeed<'guard, Guardian>
where
    Guardian: UrlGuardian + 'static,
{
    #[inline]
    fn as_ref(&self) -> &UrlWithDepth {
        self.url()
    }
}

impl<'guard, Guardian> AsRef<AtraUrlOrigin> for GuardedSeed<'guard, Guardian>
where
    Guardian: UrlGuardian + 'static,
{
    #[inline]
    fn as_ref(&self) -> &AtraUrlOrigin {
        self.origin()
    }
}
