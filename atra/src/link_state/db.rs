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

use crate::database::DBActionType::{Merge, Read, Write};
use crate::database::{DBActionType, RawDatabaseError, LINK_STATE_DB_CF};
use crate::link_state::{LinkState, LinkStateDB, LinkStateDBError, LinkStateKind};
use crate::url::UrlWithDepth;
use crate::{db_health_check, declare_column_families};
use rocksdb::{BoundColumnFamily, ReadOptions, DB};
use std::ops::RangeBounds;
use std::sync::Arc;
use tokio::task::yield_now;

/// A database knowing all the states of all urls.
#[derive(Clone, Debug)]
pub struct LinkStateRockDB {
    db: Arc<DB>,
}

impl LinkStateRockDB {
    declare_column_families! {
        self.db => cf_handle(LINK_STATE_DB_CF)
    }

    /// Panics if the needed CFs are not configured.
    pub fn new(db: Arc<DB>) -> Self {
        db_health_check!(db: [
            Self::LINK_STATE_DB_CF => (
                if test link_state_cf_options
                else "The column family for the link states was not properly configured."
            )
        ]);
        Self { db }
    }

    fn set_state_internal(
        &self,
        cf: &Arc<BoundColumnFamily>,
        url: &UrlWithDepth,
        url_state: &LinkState,
    ) -> Result<(), LinkStateDBError> {
        let url_state = url_state.as_db_entry();
        Ok(self.db.put_cf(cf, url, &url_state).enrich_with_entry(
            Self::LINK_STATE_DB_CF,
            Write,
            url,
            &url_state,
        )?)
    }

    fn get_state_internal(
        &self,
        cf: &Arc<BoundColumnFamily>,
        url: &UrlWithDepth,
    ) -> Result<Option<LinkState>, LinkStateDBError> {
        let found = self.db.get_pinned_cf(cf, url).enrich_without_entry(
            Self::LINK_STATE_DB_CF,
            Read,
            url,
        )?;
        if let Some(found) = found {
            Ok(Some(LinkState::from_db_entry(&found)?))
        } else {
            Ok(None)
        }
    }

    fn upsert_state_internal(
        &self,
        cf: &Arc<BoundColumnFamily>,
        url: &UrlWithDepth,
        url_state: &LinkState,
    ) -> Result<(), LinkStateDBError> {
        let url_state = url_state.as_db_entry();
        Ok(self.db.merge_cf(cf, url, &url_state).enrich_with_entry(
            Self::LINK_STATE_DB_CF,
            Merge,
            url,
            &url_state,
        )?)
    }

    async fn scan_for_any_link_state_internal<T: RangeBounds<LinkStateKind>>(
        &self,
        states: T,
    ) -> bool {
        let mut options = ReadOptions::default();
        options.fill_cache(false);

        const MAX_STEP_SIZE: usize = 1_000;

        match self.db.flush_cf(&self.cf_handle()) {
            Ok(_) => {}
            Err(err) => {
                log::warn!("Failed to flush before scanning {err}");
            }
        };

        let mut iter = self.db.raw_iterator_cf_opt(&self.cf_handle(), options);

        let mut pos = 0usize;
        iter.seek_to_first();
        while iter.valid() {
            if pos % MAX_STEP_SIZE == 0 {
                yield_now().await;
            }
            if let Some(value) = iter.value() {
                match LinkState::read_kind(value) {
                    Ok(ref found) => {
                        if states.contains(found) {
                            return true;
                        }
                    }
                    Err(_) => {}
                }
            }
            iter.next();
            pos += 1;
        }
        return false;
    }

    // Returns a weak ref that is faster for R/W-Actions.
    #[cfg(test)]
    pub fn weak(&self) -> WeakLinkStateDB {
        WeakLinkStateDB {
            state_db: self,
            cf: self.cf_handle(),
        }
    }
}

impl LinkStateDB for LinkStateRockDB {
    fn set_state(&self, url: &UrlWithDepth, url_state: &LinkState) -> Result<(), LinkStateDBError> {
        let handle = self.cf_handle();
        self.set_state_internal(&handle, url, url_state)
    }

    fn get_state(&self, url: &UrlWithDepth) -> Result<Option<LinkState>, LinkStateDBError> {
        let handle = self.cf_handle();
        self.get_state_internal(&handle, url)
    }

    fn upsert_state(
        &self,
        url: &UrlWithDepth,
        url_state: &LinkState,
    ) -> Result<(), LinkStateDBError> {
        let handle = self.cf_handle();
        self.upsert_state_internal(&handle, url, url_state)
    }

    fn count_state(&self, link_state_type: LinkStateKind) -> Result<u64, LinkStateDBError> {
        let handle = self.cf_handle();
        self.db
            .flush_cf(&handle)
            .enrich_no_key(LINK_STATE_DB_CF, DBActionType::Flush)?;
        let mut iter = self.db.raw_iterator_cf(&handle);
        // Forwards iteration
        iter.seek_to_first();
        let mut ct = 0u64;
        while iter.valid() {
            if let Some(value) = iter.value() {
                if LinkState::read_kind(value)? == link_state_type {
                    ct += 1;
                }
            }
            iter.next();
        }
        Ok(ct)
    }

    async fn scan_for_any_link_state<T: RangeBounds<LinkStateKind>>(&self, states: T) -> bool {
        self.scan_for_any_link_state_internal(states).await
    }
}

/// A weak ref to a db for faster working
#[derive(Clone)]
pub struct WeakLinkStateDB<'a> {
    state_db: &'a LinkStateRockDB,
    cf: Arc<BoundColumnFamily<'a>>,
}

impl<'a> LinkStateDB for WeakLinkStateDB<'a> {
    fn set_state(&self, url: &UrlWithDepth, url_state: &LinkState) -> Result<(), LinkStateDBError> {
        self.state_db.set_state_internal(&self.cf, url, url_state)
    }

    fn get_state(&self, url: &UrlWithDepth) -> Result<Option<LinkState>, LinkStateDBError> {
        self.state_db.get_state_internal(&self.cf, url)
    }

    fn upsert_state(
        &self,
        url: &UrlWithDepth,
        url_state: &LinkState,
    ) -> Result<(), LinkStateDBError> {
        self.state_db
            .upsert_state_internal(&self.cf, url, url_state)
    }

    fn count_state(&self, link_state_type: LinkStateKind) -> Result<u64, LinkStateDBError> {
        self.state_db.count_state(link_state_type)
    }

    async fn scan_for_any_link_state<T: RangeBounds<LinkStateKind>>(&self, states: T) -> bool {
        self.state_db.scan_for_any_link_state_internal(states).await
    }
}
