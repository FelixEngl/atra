//Copyright 2024 Felix Engl
//
//Licensed under the Apache License, Version 2.0 (the "License");
//you may not use this file except in compliance with the License.
//You may obtain a copy of the License at
//
//    http://www.apache.org/licenses/LICENSE-2.0
//
//Unless required by applicable law or agreed to in writing, software
//distributed under the License is distributed on an "AS IS" BASIS,
//WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//See the License for the specific language governing permissions and
//limitations under the License.

use std::sync::Arc;
use rocksdb::{DB};
use crate::core::config::Configs;
use crate::core::crawl::slim::SlimCrawlResult;
use crate::core::database_error::{DatabaseError, RawDatabaseError, RawIOError};
use crate::core::database_error::DBActionType::{Read, Write};
use crate::core::UrlWithDepth;
use crate::db_health_check;
use crate::declare_column_families;



/// Manages the crawled websites in a database until it is flushed
#[derive(Debug, Clone)]
pub struct CrawlDB {
    db: Arc<DB>
}

/// Uses prefix
impl CrawlDB {

    declare_column_families! {
        self.db => cf_handle(CRAWL_DB_CF)
    }

    /// Panics if the needed CFs are not configured.
    pub fn new(db: Arc<DB>, _: &Configs) -> Result<Self, rocksdb::Error> {
        db_health_check!(db: [
            Self::CRAWL_DB_CF => (
                if test crawled_page_cf_options
                else "The head-cf for the CrawlDB is missing!"
            )
        ]);
        Ok(Self{db})
    }

    /// Adds a single [value]
    pub fn add(&self, value: &SlimCrawlResult) -> Result<(), DatabaseError> {
        let key = &value.url;
        let serialized = match bincode::serialize(&value) {
            Ok(value) => value,
            Err(err) => return Err(err.enrich_ser(
                Self::CRAWL_DB_CF,
                key,
                value.clone()
            ))
        };
        self.db.put_cf(
            &self.cf_handle(),
            key,
            &serialized
        ).enrich_with_entry(
            Self::CRAWL_DB_CF,
            Write,
            key,
            &serialized
        )?;

        Ok(())
    }

    // /// Adds in bulk, returns the number of added elements.
    // pub fn bulk_add<I: IntoIterator<Item=SlimCrawlResult>>(&self, values: I) -> Result<usize, DatabaseError> {
    //     let handle = self.cf_handle();
    //     let mut batch = rocksdb::WriteBatch::default();
    //     let mut added_elements = 0usize;
    //     for value in values {
    //         let key = value.url.as_str().as_bytes();
    //         let value = bincode::serialize(&value).enrich_ser(
    //             Self::CRAWL_DB_CF,
    //             key,
    //             value.clone()
    //         )?;
    //         self.check_size(key, &value)?;
    //         batch.put_cf(&handle, key, value);
    //         added_elements += 1;
    //     }
    //
    //
    //     self.db.write(batch).enrich_without_entry(
    //         CRAWL_DB_CF,
    //         DBActionType::BulkWrite,
    //         &[],
    //     )?;
    //     Ok(added_elements)
    // }

    /// Gets the complete entry for the [url]
    pub fn get(&self, url: &UrlWithDepth) -> Result<Option<SlimCrawlResult>, DatabaseError> {
        let handle = self.cf_handle();
        let key = url.as_str().as_bytes();
        if self.db.key_may_exist_cf(&handle, key) {
            if let Some(pinned) = self.db.get_pinned_cf(&handle, key).enrich_without_entry(
                Self::CRAWL_DB_CF,
                Read,
                url,
            )? {
                Ok(
                    Some(
                        match bincode::deserialize(pinned.as_ref()) {
                            Ok(value) => value,
                            Err(err) => {
                                return Err(err.enrich_de(Self::CRAWL_DB_CF, key, pinned.to_vec()))
                            }
                        }
                    )
                )
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }


    // pub fn contains(&self, url: &UrlWithDepth) -> Result<bool, DatabaseError> {
    //     let handle = self.cf_handle();
    //     let key = url.as_str().as_bytes();
    //     if self.db.key_may_exist_cf(&handle, key) {
    //         let mut options = ReadOptions::default();
    //         options.set_iterate_range(rocksdb::PrefixRange(&key[..15]));
    //         options.fill_cache(false);
    //         Ok(
    //             self.db
    //                 .get_pinned_cf_opt(&handle, key, &options)
    //                 .enrich_without_entry(Self::CRAWL_DB_CF, Read, key)?
    //                 .is_some()
    //         )
    //     } else {
    //         Ok(false)
    //     }
    // }

    // pub fn iter(&self) -> impl Iterator<Item=Result<SlimCrawlResult, DatabaseError>> + '_ {
    //     let handle = self.cf_handle();
    //     let _ = self.db.flush_cf(&handle).enrich_no_key(Self::CRAWL_DB_CF, DBActionType::Flush);
    //     self.db.iterator_cf(&handle, IteratorMode::Start).map(|value| {
    //         match value {
    //             Ok(found) => {
    //                 match bincode::deserialize::<SlimCrawlResult>(&found.1) {
    //                     Ok(header) => {
    //                         Ok(header)
    //                     }
    //                     Err(err) => {
    //                         Err(err.enrich_de(Self::CRAWL_DB_CF, found.0, found.1.to_vec()))
    //                     }
    //                 }
    //             }
    //             Err(err) => {Err(err.enrich_no_key(Self::CRAWL_DB_CF, DBActionType::Iterate))}
    //         }
    //     })
    // }
}






