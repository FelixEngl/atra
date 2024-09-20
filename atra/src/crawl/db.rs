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

use crate::config::Config;
use crate::crawl::SlimCrawlResult;
use crate::database::DBActionType::{Read, Write};
use crate::database::{execute_iter, get_len, DatabaseError, RawDatabaseError, RawIOError};
use crate::db_health_check;
use crate::declare_column_families;
use crate::url::{UrlWithDepth};
use rocksdb::{DBIteratorWithThreadMode, DBWithThreadMode, MultiThreaded, DB};
use std::sync::Arc;

/// Manages the crawled websites in a database until it is flushed
#[derive(Debug, Clone)]
pub struct CrawlDB {
    db: Arc<DB>,
}

/// Uses prefix
impl CrawlDB {
    declare_column_families! {
        self.db => cf_handle(CRAWL_DB_CF)
    }

    /// Panics if the needed CFs are not configured.
    pub fn new(db: Arc<DB>, _: &Config) -> Result<Self, rocksdb::Error> {
        db_health_check!(db: [
            Self::CRAWL_DB_CF => (
                if test crawled_page_cf_options
                else "The head-cf for the CrawlDB is missing!"
            )
        ]);
        Ok(Self { db })
    }

    /// Adds a single [value]
    pub fn add(&self, value: &SlimCrawlResult) -> Result<(), DatabaseError> {
        let key = &value.meta.url;
        let serialized = match bincode::serialize(&value) {
            Ok(value) => value,
            Err(err) => return Err(err.enrich_ser(Self::CRAWL_DB_CF, key, value.clone())),
        };
        self.db
            .put_cf(&self.cf_handle(), key, &serialized)
            .enrich_with_entry(Self::CRAWL_DB_CF, Write, key, &serialized)?;

        Ok(())
    }

    /// Gets the complete entry for the [url]
    pub fn get(&self, url: &UrlWithDepth) -> Result<Option<SlimCrawlResult>, DatabaseError> {
        let handle = self.cf_handle();
        let key = url.as_bytes();
        if self.db.key_may_exist_cf(&handle, key) {
            if let Some(pinned) = self.db.get_pinned_cf(&handle, key).enrich_without_entry(
                Self::CRAWL_DB_CF,
                Read,
                url,
            )? {
                Ok(Some(match bincode::deserialize(pinned.as_ref()) {
                    Ok(value) => value,
                    Err(err) => return Err(err.enrich_de(Self::CRAWL_DB_CF, key, pinned.to_vec())),
                }))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    pub fn len(&self) -> usize {
        get_len(&self.db, self.cf_handle())
    }

    pub fn iter(&self) -> DBIteratorWithThreadMode<DBWithThreadMode<MultiThreaded>> {
        execute_iter(&self.db, self.cf_handle())
    }
}
