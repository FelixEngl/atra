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
use crate::database::{execute_iter, get_len, DBActionType, RawDatabaseError, LINK_STATE_DB_CF};
use crate::link_state::{
    LinkStateDB, LinkStateDBError, LinkStateKind, LinkStateLike, RawLinkState,
};
use crate::url::UrlWithDepth;
use crate::{db_health_check, declare_column_families};
use rocksdb::{
    BoundColumnFamily, DBIteratorWithThreadMode, DBWithThreadMode, IteratorMode, MultiThreaded,
    ReadOptions, DB,
};
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
        url_state: &impl LinkStateLike,
    ) -> Result<(), LinkStateDBError> {
        let raw = url_state.as_raw_link_state().into_owned();
        Ok(self.db.put_cf(cf, url, &raw).enrich_with_entry(
            Self::LINK_STATE_DB_CF,
            Write,
            url,
            &raw,
        )?)
    }

    fn get_state_internal(
        &self,
        cf: &Arc<BoundColumnFamily>,
        url: &UrlWithDepth,
    ) -> Result<Option<RawLinkState>, LinkStateDBError> {
        let found = self.db.get_pinned_cf(cf, url).enrich_without_entry(
            Self::LINK_STATE_DB_CF,
            Read,
            url,
        )?;
        if let Some(found) = found {
            Ok(Some(RawLinkState::from_slice(&found)?))
        } else {
            Ok(None)
        }
    }

    fn upsert_state_internal(
        &self,
        cf: &Arc<BoundColumnFamily>,
        url: &UrlWithDepth,
        upsert: &impl LinkStateLike,
    ) -> Result<(), LinkStateDBError> {
        let raw = upsert.as_raw_link_state().into_owned();
        Ok(self.db.merge_cf(cf, url, &raw).enrich_with_entry(
            Self::LINK_STATE_DB_CF,
            Merge,
            url,
            &raw,
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
                match RawLinkState::read_kind(value) {
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

    pub fn len(&self) -> usize {
        get_len(&self.db, self.cf_handle())
    }

    pub fn iter(
        &self,
        mode: IteratorMode,
    ) -> DBIteratorWithThreadMode<DBWithThreadMode<MultiThreaded>> {
        execute_iter(&self.db, self.cf_handle(), mode)
    }
}

impl LinkStateDB for LinkStateRockDB {
    fn set_state(
        &self,
        url: &UrlWithDepth,
        url_state: &impl LinkStateLike,
    ) -> Result<(), LinkStateDBError> {
        let handle = self.cf_handle();
        self.set_state_internal(&handle, url, url_state)
    }

    fn get_state(&self, url: &UrlWithDepth) -> Result<Option<RawLinkState>, LinkStateDBError> {
        let handle = self.cf_handle();
        self.get_state_internal(&handle, url)
    }

    fn upsert_state(
        &self,
        url: &UrlWithDepth,
        upsert: &impl LinkStateLike,
    ) -> Result<(), LinkStateDBError> {
        let handle = self.cf_handle();
        self.upsert_state_internal(&handle, url, upsert)
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
                if RawLinkState::read_kind(value)? == link_state_type {
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

    async fn scan_for_value<F>(&self, scanner: F) -> bool
    where
        F: Fn(&[u8], &[u8]) -> bool,
    {
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
            if let Some((key, value)) = iter.item() {
                if scanner(key, value) {
                    return true;
                }
            }
            iter.next();
            pos += 1;
        }
        false
    }

    fn collect_values<F>(&self, collector: F)
    where
        F: Fn(u64, &[u8], &[u8]) -> bool,
    {
        let mut options = ReadOptions::default();
        options.fill_cache(false);

        match self.db.flush_cf(&self.cf_handle()) {
            Ok(_) => {}
            Err(err) => {
                log::warn!("Failed to flush before scanning {err}");
            }
        };

        let mut iter = self.db.raw_iterator_cf_opt(&self.cf_handle(), options);

        let mut pos = 0u64;
        iter.seek_to_first();
        while iter.valid() {
            if let Some((key, value)) = iter.item() {
                if !collector(pos, key, value) {
                    break;
                }
            }
            iter.next();
            pos += 1;
        }
    }
}

/// A weak ref to a db for faster working
#[derive(Clone)]
pub struct WeakLinkStateDB<'a> {
    state_db: &'a LinkStateRockDB,
    cf: Arc<BoundColumnFamily<'a>>,
}

impl<'a> LinkStateDB for WeakLinkStateDB<'a> {
    fn set_state(
        &self,
        url: &UrlWithDepth,
        upsert: &impl LinkStateLike,
    ) -> Result<(), LinkStateDBError> {
        self.state_db.set_state_internal(&self.cf, url, upsert)
    }

    fn get_state(&self, url: &UrlWithDepth) -> Result<Option<RawLinkState>, LinkStateDBError> {
        self.state_db.get_state_internal(&self.cf, url)
    }

    fn upsert_state(
        &self,
        url: &UrlWithDepth,
        url_state: &impl LinkStateLike,
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

    async fn scan_for_value<F>(&self, scanner: F) -> bool
    where
        F: Fn(&[u8], &[u8]) -> bool,
    {
        self.state_db.scan_for_value(scanner).await
    }

    fn collect_values<F>(&self, collector: F)
    where
        F: Fn(u64, &[u8], &[u8]) -> bool,
    {
        self.state_db.collect_values(collector)
    }
}

#[cfg(test)]
mod test {
    use crate::database::{destroy_db, open_db};
    use crate::link_state::{
        DatabaseLinkStateManager, IsSeedYesNo, LinkStateDB, LinkStateKind, LinkStateLike,
        LinkStateManager, LinkStateRockDB, RawLinkState, RecrawlYesNo,
    };
    use crate::queue::{SupportsForcedQueueElement, UrlQueue, UrlQueueElement};
    use crate::test_impls::{InMemoryLinkStateManager, TestUrlQueue};
    use crate::url::{Depth, UrlWithDepth};
    use rocksdb::DB;
    use std::sync::Arc;
    use time::{Duration, OffsetDateTime};

    async fn run_push_test(manager: &impl LinkStateManager) {
        let youtube: UrlWithDepth = "https://www.youtube.com/".parse().unwrap();

        let mut ebay: UrlWithDepth = "https://www.ebay.com/".parse().unwrap();
        ebay.depth = Depth::new(1, 2, 3);
        let ebay = ebay;

        let amazon: UrlWithDepth = "https://www.youtube.com/".parse().unwrap();

        manager
            .update_link_state_no_payload(
                &youtube,
                LinkStateKind::Discovered,
                Some(IsSeedYesNo::Yes),
                Some(RecrawlYesNo::No),
            )
            .await
            .unwrap();
        manager
            .update_link_state_no_payload(
                &ebay,
                LinkStateKind::Discovered,
                Some(IsSeedYesNo::Yes),
                Some(RecrawlYesNo::Yes),
            )
            .await
            .unwrap();
        manager
            .update_link_state_no_payload(
                &amazon,
                LinkStateKind::Discovered,
                Some(IsSeedYesNo::Yes),
                Some(RecrawlYesNo::No),
            )
            .await
            .unwrap();

        let youtube_upsert = RawLinkState::new_preconfigured_upsert_no_payload(
            &youtube,
            LinkStateKind::Discovered,
            Some(IsSeedYesNo::Yes),
            Some(RecrawlYesNo::No),
        );
        let ebay_upsert = RawLinkState::new_preconfigured_upsert_no_payload(
            &ebay,
            LinkStateKind::Discovered,
            Some(IsSeedYesNo::Yes),
            Some(RecrawlYesNo::Yes),
        );
        let amazon_upsert = RawLinkState::new_preconfigured_upsert_no_payload(
            &amazon,
            LinkStateKind::Discovered,
            Some(IsSeedYesNo::Yes),
            Some(RecrawlYesNo::No),
        );

        assert_eq!(
            None,
            manager
                .get_link_state(&"https://www.yahoo.com/".parse().unwrap())
                .await
                .unwrap()
        );

        let real_values_youtube = manager.get_link_state(&youtube).await.unwrap().unwrap();
        let real_values_ebay = manager.get_link_state(&ebay).await.unwrap().unwrap();
        let real_values_amazon = manager.get_link_state(&amazon).await.unwrap().unwrap();

        println!("{:?}", real_values_youtube.as_link_state());
        println!("{:?}", real_values_ebay.as_link_state());
        println!("{:?}", real_values_amazon.as_link_state());
        assert!(
            youtube_upsert.eq_without_special_fields(&real_values_youtube),
            "{:?} != {:?}",
            youtube_upsert,
            real_values_youtube
        );
        assert!(
            ebay_upsert.eq_without_special_fields(&real_values_ebay),
            "{:?} != {:?}",
            ebay_upsert,
            real_values_ebay
        );
        assert!(
            amazon_upsert.eq_without_special_fields(&real_values_amazon),
            "{:?} != {:?}",
            amazon_upsert,
            real_values_amazon
        );

        manager
            .update_link_state_no_meta_and_payload(&youtube, LinkStateKind::Crawled)
            .await
            .unwrap();
        manager
            .update_link_state_no_meta_and_payload(&ebay, LinkStateKind::ReservedForCrawl)
            .await
            .unwrap();

        let youtube_upsert = RawLinkState::new_preconfigured_upsert_no_payload(
            &youtube,
            LinkStateKind::Crawled,
            Some(IsSeedYesNo::Yes),
            Some(RecrawlYesNo::No),
        );
        let ebay_upsert = RawLinkState::new_preconfigured_upsert_no_payload(
            &ebay,
            LinkStateKind::ReservedForCrawl,
            Some(IsSeedYesNo::Yes),
            Some(RecrawlYesNo::Yes),
        );

        let real_values_youtube = manager.get_link_state(&youtube).await.unwrap().unwrap();
        let real_values_ebay = manager.get_link_state(&ebay).await.unwrap().unwrap();

        println!("----");
        println!("{:?}", real_values_youtube.as_link_state());
        println!("{:?}", real_values_ebay.as_link_state());
        assert!(
            youtube_upsert.eq_without_special_fields(&real_values_youtube),
            "{:?} != {:?}",
            youtube_upsert,
            real_values_youtube
        );
        assert!(
            ebay_upsert.eq_without_special_fields(&real_values_ebay),
            "{:?} != {:?}",
            ebay_upsert,
            real_values_ebay
        );

        manager
            .update_link_state_no_payload(
                &youtube,
                LinkStateKind::ProcessedAndStored,
                None,
                Some(RecrawlYesNo::Yes),
            )
            .await
            .unwrap();

        let youtube_upsert = RawLinkState::new_preconfigured_upsert_no_payload(
            &youtube,
            LinkStateKind::ProcessedAndStored,
            Some(IsSeedYesNo::Yes),
            Some(RecrawlYesNo::Yes),
        );

        let real_values_youtube = manager.get_link_state(&youtube).await.unwrap().unwrap();

        println!("----");
        println!("{:?}", real_values_youtube.as_link_state());
        println!("{:?}", real_values_ebay.as_link_state());
        assert!(
            youtube_upsert.eq_without_special_fields(&real_values_youtube),
            "{:?} != {:?}",
            youtube_upsert,
            real_values_youtube
        );
        assert!(
            ebay_upsert.eq_without_special_fields(&real_values_ebay),
            "{:?} != {:?}",
            ebay_upsert,
            real_values_ebay
        );

        manager
            .update_link_state_no_payload(
                &youtube,
                LinkStateKind::InternalError,
                None,
                Some(RecrawlYesNo::Unknown(12)),
            )
            .await
            .unwrap();
        manager
            .update_link_state_no_payload(
                &ebay,
                LinkStateKind::ProcessedAndStored,
                None,
                Some(RecrawlYesNo::No),
            )
            .await
            .unwrap();

        let youtube_upsert = RawLinkState::new_preconfigured_upsert_no_payload(
            &youtube,
            LinkStateKind::InternalError,
            Some(IsSeedYesNo::Yes),
            Some(RecrawlYesNo::Yes),
        );
        let ebay_upsert = RawLinkState::new_preconfigured_upsert_no_payload(
            &ebay,
            LinkStateKind::ProcessedAndStored,
            Some(IsSeedYesNo::Yes),
            Some(RecrawlYesNo::No),
        );

        let real_values_youtube = manager.get_link_state(&youtube).await.unwrap().unwrap();
        let real_values_ebay = manager.get_link_state(&ebay).await.unwrap().unwrap();

        println!("----");
        println!("{:?}", real_values_youtube.as_link_state());
        println!("{:?}", real_values_ebay.as_link_state());
        assert!(
            youtube_upsert.eq_without_special_fields(&real_values_youtube),
            "{:?} != {:?}",
            youtube_upsert,
            real_values_youtube
        );
        assert!(
            ebay_upsert.eq_without_special_fields(&real_values_ebay),
            "{:?} != {:?}",
            ebay_upsert,
            real_values_ebay
        );

        assert!(manager.check_if_there_are_any_recrawlable_links().await);
        let col = TestUrlQueue::default();
        let c = &col;
        manager
            .collect_recrawlable_links(|_, value| {
                c.force_enqueue(UrlQueueElement::new(false, 0, false, value))
                    .unwrap()
            })
            .await;
        assert_eq!(1, col.len().await);
        println!("---- {:?}", col.dequeue().await.unwrap().unwrap().take())
    }

    #[tokio::test]
    async fn db_can_be_managed_test_impl() {
        let manager = InMemoryLinkStateManager::new();
        run_push_test(&manager).await;
    }

    #[tokio::test]
    async fn db_can_be_managed() {
        use scopeguard::defer;
        defer!(destroy_db("test/lnk_db0").unwrap(););
        std::fs::create_dir_all("test").unwrap();
        let db: Arc<DB> = open_db("test/lnk_db0").unwrap().into();
        let manager = DatabaseLinkStateManager::new(db.clone());

        run_push_test(&manager).await;

        let youtube: UrlWithDepth = "https://www.youtube.com/".parse().unwrap();

        let mut ebay: UrlWithDepth = "https://www.ebay.com/".parse().unwrap();
        ebay.depth = Depth::new(1, 2, 3);
        let ebay = ebay;

        let raw_db = LinkStateRockDB::new(db);

        let youtube_upsert = RawLinkState::new_preconfigured_upsert_no_payload(
            &youtube,
            LinkStateKind::InternalError,
            Some(IsSeedYesNo::Yes),
            Some(RecrawlYesNo::Yes),
        );

        let ebay_upsert = RawLinkState::new_preconfigured_upsert_no_payload(
            &ebay,
            LinkStateKind::ProcessedAndStored,
            Some(IsSeedYesNo::Yes),
            Some(RecrawlYesNo::No),
        );

        let mut ebay_upsert_defect = RawLinkState::new();
        ebay_upsert_defect.set_kind(LinkStateKind::InternalError);
        ebay_upsert_defect.set_timestamp(OffsetDateTime::now_utc() - Duration::weeks(1));
        ebay_upsert_defect.set_recrawl(RecrawlYesNo::Unknown(12));

        raw_db.upsert_state(&ebay, &ebay_upsert_defect).unwrap();

        let real_values_youtube = manager.get_link_state(&youtube).await.unwrap().unwrap();
        let real_values_ebay = manager.get_link_state(&ebay).await.unwrap().unwrap();

        println!("----");
        println!(
            "-- {:?}\n-- {:?}",
            youtube_upsert.as_link_state(),
            real_values_youtube.as_link_state()
        );
        assert!(
            youtube_upsert.eq_without_special_fields(&real_values_youtube),
            "{:?} != {:?}",
            youtube_upsert,
            real_values_youtube
        );
        println!("{:?}", real_values_ebay.as_link_state());
        assert!(
            ebay_upsert.eq_without_special_fields(&real_values_ebay),
            "{:?} != {:?}",
            ebay_upsert,
            real_values_ebay
        );
    }
}
