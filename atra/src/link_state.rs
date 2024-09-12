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

use crate::database::DBActionType::{Merge, Read, Write};
use crate::database::LINK_STATE_DB_CF;
use crate::database::{DBActionType, DatabaseError, RawDatabaseError};
use crate::url::Depth;
use crate::url::UrlWithDepth;
use crate::{db_health_check, declare_column_families};
use num_enum::FromPrimitive;
use num_enum::IntoPrimitive;
use rocksdb::{BoundColumnFamily, ReadOptions, DB};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::array::TryFromSliceError;
use std::ops::RangeBounds;
use std::sync::Arc;
use strum::AsRefStr;
use strum::{Display, EnumIs};
use thiserror::Error;
use time::{error, OffsetDateTime};
use tokio::task::yield_now;

/// The state of a link
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash)]
pub struct LinkState {
    pub typ: LinkStateType,
    pub last_significant_typ: LinkStateType,
    pub timestamp: OffsetDateTime,
    pub depth: Depth,
    pub payload: Option<Vec<u8>>,
}

impl LinkState {
    pub fn new(
        typ: LinkStateType,
        last_significant_typ: LinkStateType,
        timestamp: OffsetDateTime,
        depth: Depth,
        payload: Option<Vec<u8>>,
    ) -> Self {
        Self {
            typ,
            last_significant_typ,
            timestamp,
            depth,
            payload,
        }
    }

    pub fn with_payload(
        typ: LinkStateType,
        last_significant_typ: LinkStateType,
        timestamp: OffsetDateTime,
        depth: Depth,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            typ,
            last_significant_typ,
            timestamp,
            depth,
            payload: Some(payload),
        }
    }

    pub fn without_payload(
        typ: LinkStateType,
        last_significant_typ: LinkStateType,
        timestamp: OffsetDateTime,
        depth: Depth,
    ) -> Self {
        Self {
            typ,
            last_significant_typ,
            timestamp,
            depth,
            payload: None,
        }
    }

    pub fn update_in_place(&mut self, update: Self) {
        let lst = if self.typ.is_significant() {
            self.typ
        } else {
            self.last_significant_typ
        };
        self.typ = update.typ;
        self.last_significant_typ = lst;
        self.timestamp = update.timestamp;
        self.payload = update.payload;
        self.depth = update.depth;
    }
}

/// Describes the current state of an url.
#[derive(
    Debug,
    Serialize,
    Deserialize,
    Clone,
    Copy,
    IntoPrimitive,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    FromPrimitive,
    Display,
    AsRefStr,
    Hash,
    EnumIs,
)]
#[repr(u8)]
pub enum LinkStateType {
    /// An url was discovered
    Discovered = 0u8,
    /// Shows that the link is currently processed
    ReservedForCrawl = 1u8,
    /// An link was crawled at some specific time
    Crawled = 2u8,
    /// The link was processed and stored.
    ProcessedAndStored = 3u8,
    /// An internal error.
    InternalError = 32u8,
    /// The value if unset, usually only used for updates.
    Unset = u8::MAX - 1,
    /// An unknown type
    #[num_enum(catch_all)]
    Unknown(u8),
}

impl LinkStateType {
    pub fn is_significant_raw(value: u8) -> bool {
        value < 4
    }

    pub fn is_significant(&self) -> bool {
        self < &LinkStateType::InternalError
    }

    pub fn into_update(self, url: &UrlWithDepth, payload: Option<Vec<u8>>) -> LinkState {
        LinkState::new(
            self.clone(),
            Self::Unset,
            OffsetDateTime::now_utc(),
            url.depth().clone(),
            payload,
        )
    }
}

/// The errors when creating or writing a linkstate
#[derive(Debug, Error)]
pub enum LinkStateError {
    #[error("The buffer is emptys")]
    EmptyBuffer,
    #[error("The buffer requires a length of {0} but has only {1}.")]
    BufferTooSmall(usize, usize),
    #[error(transparent)]
    NumberConversionError(#[from] TryFromSliceError),
    #[error("The marker {0} is unknown!")]
    IllegalMarker(u8),
    #[error(transparent)]
    TimestampNotReconstructable(#[from] error::ComponentRange),
}

impl LinkState {
    pub const TYP_POS: usize = 0;
    pub const LAST_SIGNIFICANT_TYP_POS: usize = 1;
    pub const OFFSET_TIME: usize = Self::TYP_POS + 2;
    pub const OFFSET_DEPTH: usize = Self::OFFSET_TIME + 16;
    pub const OFFSET_PAYLOAD: usize = Self::OFFSET_DEPTH + 24;
    pub const IDEAL_SIZE: usize = Self::OFFSET_PAYLOAD;

    fn write_timestamp(target: &mut [u8], time: &OffsetDateTime) {
        (&mut target[Self::OFFSET_TIME..Self::OFFSET_DEPTH])
            .copy_from_slice(&time.unix_timestamp_nanos().to_be_bytes())
    }

    fn write_depth_descriptor(target: &mut [u8], depth: &Depth) {
        (&mut target[Self::OFFSET_DEPTH..Self::OFFSET_DEPTH + 8])
            .copy_from_slice(&depth.depth_on_website.to_be_bytes());
        (&mut target[Self::OFFSET_DEPTH + 8..Self::OFFSET_DEPTH + 16])
            .copy_from_slice(&depth.distance_to_seed.to_be_bytes());
        (&mut target[Self::OFFSET_DEPTH + 16..Self::OFFSET_PAYLOAD])
            .copy_from_slice(&depth.total_distance_to_seed.to_be_bytes());
    }

    pub fn as_db_entry(&self) -> SmallVec<[u8; Self::IDEAL_SIZE]> {
        let mut result = [0u8; Self::IDEAL_SIZE];
        result[Self::TYP_POS] = self.typ.into();
        result[Self::LAST_SIGNIFICANT_TYP_POS] = self.last_significant_typ.into();
        Self::write_timestamp(&mut result, &self.timestamp);
        Self::write_depth_descriptor(&mut result, &self.depth);
        let mut result = SmallVec::from(result);
        match &self.payload {
            None => {}
            Some(value) => {
                result.extend_from_slice(value);
            }
        }
        result
    }

    pub fn read_type(buffer: &[u8]) -> Result<LinkStateType, LinkStateError> {
        if buffer.is_empty() {
            Err(LinkStateError::EmptyBuffer)
        } else {
            Ok(buffer[Self::TYP_POS].into())
        }
    }

    pub fn read_last_significant_typ(buffer: &[u8]) -> Result<LinkStateType, LinkStateError> {
        if buffer.len() < Self::OFFSET_TIME {
            Err(LinkStateError::BufferTooSmall(
                Self::OFFSET_TIME,
                buffer.len(),
            ))
        } else {
            Ok(buffer[Self::LAST_SIGNIFICANT_TYP_POS].into())
        }
    }

    pub fn read_timestamp(buffer: &[u8]) -> Result<OffsetDateTime, LinkStateError> {
        if buffer.len() < Self::OFFSET_DEPTH {
            return Err(LinkStateError::BufferTooSmall(
                Self::OFFSET_DEPTH,
                buffer.len(),
            ));
        }
        Ok(OffsetDateTime::from_unix_timestamp_nanos(
            i128::from_be_bytes(buffer[Self::OFFSET_TIME..Self::OFFSET_DEPTH].try_into()?),
        )?)
    }

    pub fn read_depth_desc(buffer: &[u8]) -> Result<Depth, LinkStateError> {
        if buffer.len() < Self::OFFSET_PAYLOAD {
            return Err(LinkStateError::BufferTooSmall(
                Self::OFFSET_PAYLOAD,
                buffer.len(),
            ));
        }
        let depth_on_website =
            u64::from_be_bytes((&buffer[Self::OFFSET_DEPTH..Self::OFFSET_DEPTH + 8]).try_into()?);
        let distance_to_seed = u64::from_be_bytes(
            (&buffer[Self::OFFSET_DEPTH + 8..Self::OFFSET_DEPTH + 16]).try_into()?,
        );
        let total_distance_to_seed = u64::from_be_bytes(
            (&buffer[Self::OFFSET_DEPTH + 16..Self::OFFSET_PAYLOAD]).try_into()?,
        );
        Ok(Depth::new(
            depth_on_website,
            distance_to_seed,
            total_distance_to_seed,
        ))
    }

    pub fn read_optional_payload(buffer: &[u8]) -> Option<Vec<u8>> {
        if buffer.len() <= Self::OFFSET_PAYLOAD {
            None
        } else {
            Some(Vec::from(&buffer[Self::OFFSET_PAYLOAD..]))
        }
    }

    pub fn from_db_entry(buffer: &[u8]) -> Result<LinkState, LinkStateError> {
        let typ: LinkStateType = Self::read_type(buffer)?;
        let last_significant_typ: LinkStateType = Self::read_last_significant_typ(buffer)?;
        let timestamp = Self::read_timestamp(buffer)?;
        let depth = Self::read_depth_desc(buffer)?;
        let payload = Self::read_optional_payload(buffer);

        Ok(Self {
            payload,
            depth,
            timestamp,
            typ,
            last_significant_typ,
        })
    }
}

/// Possible errors of an [LinkStateDB]
#[derive(Debug, Error)]
pub enum LinkStateDBError {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error(transparent)]
    LinkStateError(#[from] LinkStateError),
}

pub trait LinkStateManager {
    /// Sets the state of [url] to [new_state]
    fn set_state(&self, url: &UrlWithDepth, new_state: &LinkState) -> Result<(), LinkStateDBError>;
    /// Gets the state of [url] or None
    fn get_state(&self, url: &UrlWithDepth) -> Result<Option<LinkState>, LinkStateDBError>;

    /// Upserts the state of the [url] with [upsert].
    fn upsert_state(&self, url: &UrlWithDepth, upsert: &LinkState) -> Result<(), LinkStateDBError>;

    /// Basically an [upsert_state] but the update is automatically generated
    fn update_state(
        &self,
        url: &UrlWithDepth,
        new_state: LinkStateType,
    ) -> Result<(), LinkStateDBError> {
        self.upsert_state(url, &new_state.into_update(url, None))
    }

    /// Counts the provided number of links state with the provided [LinkStateType]
    fn count_state(&self, link_state_type: LinkStateType) -> Result<u64, LinkStateDBError>;

    /// Basically an [upsert_state] but the update is automatically generated
    #[allow(dead_code)]
    fn update_state_with_payload(
        &self,
        url: &UrlWithDepth,
        new_state: LinkStateType,
        payload: Vec<u8>,
    ) -> Result<(), LinkStateDBError> {
        self.upsert_state(url, &new_state.into_update(url, Some(payload)))
    }

    /// Scans for any [states] in the underlying structure.
    /// This method is async due to its expensive nature.
    #[allow(dead_code)]
    async fn scan_for_any_link_state<T: RangeBounds<LinkStateType>>(&self, states: T) -> bool;
}

/// A database knowing all the states of all urls.
#[derive(Clone, Debug)]
pub struct LinkStateDB {
    db: Arc<DB>,
}

impl LinkStateDB {
    declare_column_families! {
        self.db => cf_handle(LINK_STATE_DB_CF)
    }

    /// Panics if the needed CFs are not configured.
    pub fn new(db: Arc<DB>) -> Result<Self, LinkStateDBError> {
        db_health_check!(db: [
            Self::LINK_STATE_DB_CF => (
                if test link_state_cf_options
                else "The column family for the link states was not properly configured."
            )
        ]);
        Ok(Self { db })
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

    async fn scan_for_any_link_state_internal<T: RangeBounds<LinkStateType>>(
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
                match LinkState::read_type(value) {
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

impl LinkStateManager for LinkStateDB {
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

    fn count_state(&self, link_state_type: LinkStateType) -> Result<u64, LinkStateDBError> {
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
                if LinkState::read_type(value)? == link_state_type {
                    ct += 1;
                }
            }
            iter.next();
        }
        Ok(ct)
    }

    async fn scan_for_any_link_state<T: RangeBounds<LinkStateType>>(&self, states: T) -> bool {
        self.scan_for_any_link_state_internal(states).await
    }
}

/// A weak ref to a db for faster working
#[derive(Clone)]
pub struct WeakLinkStateDB<'a> {
    state_db: &'a LinkStateDB,
    cf: Arc<BoundColumnFamily<'a>>,
}

impl<'a> LinkStateManager for WeakLinkStateDB<'a> {
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

    fn count_state(&self, link_state_type: LinkStateType) -> Result<u64, LinkStateDBError> {
        self.state_db.count_state(link_state_type)
    }

    async fn scan_for_any_link_state<T: RangeBounds<LinkStateType>>(&self, states: T) -> bool {
        self.state_db.scan_for_any_link_state_internal(states).await
    }
}

#[cfg(test)]
mod test {
    use super::{LinkState, LinkStateDB, LinkStateManager, LinkStateType};
    use crate::database::{destroy_db, open_db};
    use crate::url::{Depth, UrlWithDepth};
    use scopeguard::defer;
    use std::sync::Arc;
    use time::OffsetDateTime;

    #[test]
    fn ser_and_deser_work() {
        let new = LinkState::with_payload(
            LinkStateType::Crawled,
            LinkStateType::Crawled,
            OffsetDateTime::now_utc().replace_nanosecond(0).unwrap(),
            Depth::ZERO + (1, 2, 3),
            vec![1, 2, 3, 4, 5],
        );

        let x = new.as_db_entry();

        let deser = LinkState::from_db_entry(&x).unwrap();

        assert_eq!(new, deser)
    }

    #[test]
    fn can_initialize() {
        defer! {let  _ = destroy_db("test.db1");}

        let db = Arc::new(open_db("test.db1").unwrap());
        let db = LinkStateDB::new(db).unwrap();

        db.set_state(
            &UrlWithDepth::from_seed("https://google.de").unwrap(),
            &LinkState::without_payload(
                LinkStateType::Discovered,
                LinkStateType::Discovered,
                OffsetDateTime::now_utc(),
                Depth::ZERO,
            ),
        )
        .unwrap();

        db.set_state(
            &UrlWithDepth::from_seed("https://amazon.de").unwrap(),
            &LinkState::without_payload(
                LinkStateType::Crawled,
                LinkStateType::Discovered,
                OffsetDateTime::now_utc(),
                Depth::ZERO,
            ),
        )
        .unwrap();

        db.upsert_state(
            &UrlWithDepth::from_seed("https://google.de").unwrap(),
            &LinkState::without_payload(
                LinkStateType::InternalError,
                LinkStateType::Discovered,
                OffsetDateTime::now_utc(),
                Depth::ZERO,
            ),
        )
        .unwrap();

        println!(
            "{:?}",
            db.get_state(&UrlWithDepth::from_seed("https://amazon.de").unwrap())
                .unwrap()
        );
        println!(
            "{:?}",
            db.get_state(&UrlWithDepth::from_seed("https://google.de").unwrap())
                .unwrap()
        );
    }

    #[test]
    fn can_initialize_weak() {
        defer! {let  _ = destroy_db("test.db2");}

        let db = Arc::new(open_db("test.db2").unwrap());
        let db = LinkStateDB::new(db).unwrap();

        {
            let db = db.weak();

            db.set_state(
                &UrlWithDepth::from_seed("https://amazon.de").unwrap(),
                &LinkState::without_payload(
                    LinkStateType::Discovered,
                    LinkStateType::Discovered,
                    OffsetDateTime::now_utc(),
                    Depth::ZERO,
                ),
            )
            .unwrap();

            db.set_state(
                &UrlWithDepth::from_seed("https://google.de").unwrap(),
                &LinkState::without_payload(
                    LinkStateType::Crawled,
                    LinkStateType::Discovered,
                    OffsetDateTime::now_utc(),
                    Depth::ZERO,
                ),
            )
            .unwrap();
        }

        println!(
            "{:?}",
            db.get_state(&UrlWithDepth::from_seed("https://amazon.de").unwrap())
                .unwrap()
        );
        println!(
            "{:?}",
            db.get_state(&UrlWithDepth::from_seed("https://google.de").unwrap())
                .unwrap()
        );
    }

    #[test]
    fn can_upset_properly() {
        defer! {let  _ = destroy_db("test.db3");}

        let db = Arc::new(open_db("test.db3").unwrap());

        let db = LinkStateDB::new(db).unwrap();

        {
            let db = db.weak();

            db.update_state(
                &UrlWithDepth::from_seed("https://amazon.de").unwrap(),
                LinkStateType::Discovered,
            )
            .unwrap();

            db.update_state(
                &UrlWithDepth::from_seed("https://google.de").unwrap(),
                LinkStateType::Discovered,
            )
            .unwrap();

            db.update_state(
                &UrlWithDepth::from_seed("https://google.de").unwrap(),
                LinkStateType::Crawled,
            )
            .unwrap();

            println!(
                "Amazon: {:?}",
                db.get_state(&UrlWithDepth::from_seed("https://amazon.de").unwrap())
            );
            println!(
                "Google: {:?}",
                db.get_state(&UrlWithDepth::from_seed("https://google.de").unwrap())
            );
        }

        println!(
            "{:?}",
            db.get_state(&UrlWithDepth::from_seed("https://amazon.de").unwrap())
                .unwrap()
        );
        println!(
            "{:?}",
            db.get_state(&UrlWithDepth::from_seed("https://google.de").unwrap())
                .unwrap()
        );
    }
}
