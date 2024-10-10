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
use std::sync::{Arc, RwLock};
use crate::budget::{BudgetManager, BudgetSetting, CrawlBudget};
use crate::toolkit::in_memory_domain_manager::InMemoryDomainMappingManager;
use crate::url::AtraUrlOrigin;

#[derive(Debug)]
#[repr(transparent)]
pub struct InMemoryBudgetManager {
    inner: InMemoryDomainMappingManager<BudgetSetting>
}

impl InMemoryBudgetManager {
    pub fn new(inner: InMemoryDomainMappingManager<BudgetSetting>) -> Self {
        Self{inner}
    }
    pub fn create(crawl_budget: &CrawlBudget) -> Self {
        let mut inner = InMemoryDomainMappingManager::new(crawl_budget.default.clone());
        if let Some(value) = crawl_budget.per_host.as_ref() {
            for (k, v) in value.iter() {
                inner.set(k.clone(), v.clone())
            }
        }
        Self {inner}
    }
}

impl BudgetManager for InMemoryBudgetManager {
    fn get_default_budget(&self) -> Arc<BudgetSetting> {
        self.inner.get_default()
    }

    #[inline(always)]
    fn get_budget_for<Q: ?Sized>(&self, origin: &Q) -> Arc<BudgetSetting>
    where
        AtraUrlOrigin: Borrow<Q>,
        Q: Hash + Eq
    {
        self.inner.get_for(origin)
    }

    #[inline(always)]
    fn set_budget(&self, key: AtraUrlOrigin, value: BudgetSetting) {
        self.inner.set(key, value)
    }

    #[inline(always)]
    fn set_default_budget(&self, value: BudgetSetting) {
        self.inner.set_default(value)
    }

    fn get_export(&self) -> CrawlBudget {
        self.inner.get_content().into()
    }
}

impl From<CrawlBudget> for InMemoryBudgetManager {
    fn from(value: CrawlBudget) -> Self {
        let new = Self::new(
            InMemoryDomainMappingManager::new(value.default)
        );

        if let Some(values) = value.per_host {
            for (k, v) in values.into_iter() {
                new.set_budget(k, v)
            }
        }
        new
    }
}

