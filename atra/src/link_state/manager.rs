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

use crate::database::DatabaseError;
use crate::link_state::traits::LinkStateManager;
use crate::link_state::{
    IsSeedYesNo, LinkStateDB, LinkStateDBError, LinkStateKind, LinkStateLike, LinkStateRockDB,
    RawLinkState, RecrawlYesNo,
};
use crate::url::{AtraUri, UrlWithDepth};
use rocksdb::{DBIteratorWithThreadMode, DBWithThreadMode, IteratorMode, MultiThreaded, DB};
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::sync::RwLock;
use tokio::task::yield_now;

#[derive(Debug)]
pub struct DatabaseLinkStateManager<DB: LinkStateDB> {
    db: DB,
    last_scan_over_link_states: RwLock<Option<(bool, OffsetDateTime)>>,
}

impl DatabaseLinkStateManager<LinkStateRockDB> {
    pub fn new(db: Arc<DB>) -> Self {
        Self {
            db: LinkStateRockDB::new(db),
            last_scan_over_link_states: RwLock::new(None),
        }
    }

    pub fn len(&self) -> usize {
        self.db.len()
    }

    pub fn iter(
        &self,
        mode: IteratorMode,
    ) -> DBIteratorWithThreadMode<DBWithThreadMode<MultiThreaded>> {
        self.db.iter(mode)
    }
}

impl<DB: LinkStateDB> LinkStateManager for DatabaseLinkStateManager<DB> {
    type Error = LinkStateDBError;

    fn crawled_websites(&self) -> Result<u64, Self::Error> {
        self.db.count_state(LinkStateKind::ProcessedAndStored)
    }

    async fn update_link_state<P>(
        &self,
        url: &UrlWithDepth,
        state: LinkStateKind,
        is_seed: Option<IsSeedYesNo>,
        recrawl: Option<RecrawlYesNo>,
        payload: Option<Option<&P>>,
    ) -> Result<(), Self::Error>
    where
        P: ?Sized + AsRef<[u8]>,
    {
        match self.db.update_state(url, state, is_seed, recrawl, payload) {
            Err(LinkStateDBError::Database(DatabaseError::RecoverableFailure { .. })) => {
                yield_now().await;
                self.db.update_state(url, state, is_seed, recrawl, payload)
            }
            escalate => escalate,
        }
    }

    fn get_link_state_sync(&self, url: &UrlWithDepth) -> Result<Option<RawLinkState>, Self::Error> {
        match self.db.get_state(url) {
            Err(LinkStateDBError::Database(DatabaseError::RecoverableFailure { .. })) => {
                self.db.get_state(url)
            }
            escalate => escalate,
        }
    }

    async fn get_link_state(
        &self,
        url: &UrlWithDepth,
    ) -> Result<Option<RawLinkState>, Self::Error> {
        match self.db.get_state(url) {
            Err(LinkStateDBError::Database(DatabaseError::RecoverableFailure { .. })) => {
                self.db.get_state(url)
            }
            escalate => escalate,
        }
    }

    async fn check_if_there_are_any_crawlable_links(&self, max_age: Duration) -> bool {
        let lock = self.last_scan_over_link_states.read().await;
        if let Some(value) = lock.as_ref() {
            if (OffsetDateTime::now_utc() - value.1) <= max_age {
                return value.0;
            }
        }
        drop(lock);
        let mut lock = self.last_scan_over_link_states.write().await;
        if let Some(value) = lock.as_ref() {
            if OffsetDateTime::now_utc() - value.1 <= max_age {
                return value.0;
            }
        }
        let found = self
            .db
            .scan_for_any_link_state(LinkStateKind::Discovered..=LinkStateKind::Crawled)
            .await;
        lock.replace((found, OffsetDateTime::now_utc()));
        found
    }

    async fn check_if_there_are_any_recrawlable_links(&self) -> bool {
        self.db
            .scan_for_value(|_, v| {
                if let Ok(value) = RawLinkState::read_recrawl(v) {
                    value.is_yes()
                } else {
                    false
                }
            })
            .await
    }

    async fn collect_recrawlable_links<F: Fn(IsSeedYesNo, UrlWithDepth) -> ()>(
        &self,
        collector: F,
    ) {
        self.db.collect_values(|_, k, v| {
            let raw = unsafe { RawLinkState::from_slice_unchecked(v.as_ref()) };
            if raw.recrawl().is_yes() {
                let uri: AtraUri = String::from_utf8_lossy(k).parse().unwrap();
                collector(raw.is_seed(), UrlWithDepth::new(uri, raw.depth()));
                true
            } else {
                true
            }
        })
    }

    async fn collect_all_links<F: Fn(IsSeedYesNo, UrlWithDepth) -> ()>(&self, collector: F) {
        self.db.collect_values(|_, k, v| {
            let raw = unsafe { RawLinkState::from_slice_unchecked(v.as_ref()) };
            let uri: AtraUri = String::from_utf8_lossy(k).parse().unwrap();
            collector(raw.is_seed(), UrlWithDepth::new(uri, raw.depth()));
            true
        })
    }
}

#[cfg(test)]
mod test {}
