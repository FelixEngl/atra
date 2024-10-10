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

mod manager;
mod budget;

use std::borrow::Borrow;
use std::hash::Hash;
use std::sync::Arc;
pub use budget::*;
pub use manager::InMemoryBudgetManager;
use crate::url::AtraUrlOrigin;

pub trait BudgetManager: From<CrawlBudget> {
    fn get_default_budget(&self) -> Arc<BudgetSetting>;

    fn get_budget_for<Q: ?Sized>(&self, origin: &Q) -> Arc<BudgetSetting>
    where
        AtraUrlOrigin: Borrow<Q>,
        Q: Hash + Eq;

    fn set_budget(&self, key: AtraUrlOrigin, value: BudgetSetting);

    fn set_default_budget(&self, value: BudgetSetting);


    fn import(&self, budget: &CrawlBudget) {
        self.set_default_budget(budget.default.clone());
        if let Some(ref per_domain) = budget.per_host {
            for (k, v) in per_domain.iter() {
                self.set_budget(k.clone(), v.clone());
            }
        }
    }

    fn get_export(&self) -> CrawlBudget;
}