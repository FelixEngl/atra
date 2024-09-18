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

use crate::url::AtraUrlOrigin;
use crate::{db_health_check, declare_column_families};
use rocksdb::DB;
use std::sync::Arc;
use time::OffsetDateTime;

pub trait DomainLastCrawledManager {
    async fn register_access(&self, origin: &AtraUrlOrigin);

    async fn get_last_access(&self, origin: &AtraUrlOrigin) -> Option<OffsetDateTime>;
}

#[derive(Debug, Clone)]
pub struct DomainLastCrawledDatabaseManager {
    db: Arc<DB>,
}

impl DomainLastCrawledDatabaseManager {
    declare_column_families! {
        self.db => cf_handle(DOMAIN_MANAGER_DB_CF)
    }

    pub fn new(db: Arc<DB>) -> Self {
        db_health_check!(db: [
            Self::DOMAIN_MANAGER_DB_CF => (
                if test domain_manager_cf_options
                else "The head-cf for the domain manager db is missing!"
            )
        ]);

        Self { db }
    }
}

impl DomainLastCrawledManager for DomainLastCrawledDatabaseManager {
    async fn register_access(&self, domain: &AtraUrlOrigin) {
        let _ = self.db.put_cf(
            &self.cf_handle(),
            domain.as_bytes(),
            &bincode::serialize(&OffsetDateTime::now_utc()).unwrap(),
        );
    }

    async fn get_last_access(&self, domain: &AtraUrlOrigin) -> Option<OffsetDateTime> {
        let handle = self.cf_handle();
        let key = domain.as_bytes();
        if self.db.key_may_exist_cf(&handle, key) {
            if let Ok(Some(pinned)) = self.db.get_pinned_cf(&handle, key) {
                bincode::deserialize(pinned.as_ref()).ok()
            } else {
                None
            }
        } else {
            None
        }
    }
}
