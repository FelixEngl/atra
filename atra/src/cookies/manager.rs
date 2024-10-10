// Copyright 2024. Felix Engl
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

use std::borrow::Borrow;
use std::hash::Hash;
use std::sync::Arc;
use crate::cookies::{CookieManager, CookieSettings};
use crate::toolkit::in_memory_domain_manager::InMemoryDomainMappingManager;
use crate::url::AtraUrlOrigin;

#[derive(Debug)]
#[repr(transparent)]
pub struct InMemoryCookieManager {
    inner: InMemoryDomainMappingManager<Option<String>>
}

impl InMemoryCookieManager {
    pub fn new(inner: InMemoryDomainMappingManager<Option<String>>) -> Self {
        Self{inner}
    }

    pub fn create(settings: Option<&CookieSettings>) -> Self {
        match settings {
            None => {
                Self::new(InMemoryDomainMappingManager::new(None))
            }
            Some(value) => {
                let new = Self::new(InMemoryDomainMappingManager::new(value.default.clone()));
                if let Some(value) = value.per_host.as_ref() {
                    for (k, v) in value.iter() {
                        new.set_cookie(k.clone(), Some(v.clone()));
                    }
                }
                new
            }
        }
    }
}

impl Into<Option<CookieSettings>> for InMemoryCookieManager {
    fn into(self) -> Option<CookieSettings> {
        let (default, per_domain) = self.inner.get_content().into();
    }
}

impl CookieManager for InMemoryCookieManager {
    #[inline(always)]
    fn get_default_cookie(&self) -> Arc<Option<String>> {
        self.inner.get_default()
    }

    #[inline(always)]
    fn get_cookies_for<Q: ?Sized>(&self, domain: &Q) -> Arc<Option<String>>
    where
        AtraUrlOrigin: Borrow<Q>,
        Q: Hash + Eq
    {
        self.inner.get_for(domain)
    }

    #[inline(always)]
    fn set_cookie(&self, key: AtraUrlOrigin, value: Option<String>) {
        self.inner.set(key, value)
    }

    #[inline(always)]
    fn set_default_cookie(&self, value: Option<String>) {
        self.inner.set_default(value)
    }
}