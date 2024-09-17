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

use crate::link_state::state::traits::LinkStateLike;
use crate::link_state::{
    IsSeedYesNo, LinkState, LinkStateError, LinkStateKind, RecrawlYesNo, UNSET,
};
use crate::url::{Depth, UrlWithDepth};
use rocksdb::MergeOperands;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::borrow::Cow;
use std::ops::Deref;
use time::OffsetDateTime;

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
#[serde(transparent)]
pub struct RawLinkState {
    inner: SmallVec<[u8; RawLinkState::IDEAL_SIZE]>,
}

impl RawLinkState {
    pub const KIND_POS: usize = 0;
    pub const LAST_SIGNIFICANT_KIND_POS: usize = Self::KIND_POS + 1;
    pub const RECRAWL_POS: usize = Self::LAST_SIGNIFICANT_KIND_POS + 1;
    pub const IS_SEED_POS: usize = Self::RECRAWL_POS + 1;
    pub const OFFSET_TIME: usize = Self::IS_SEED_POS + 1;
    pub const OFFSET_DEPTH: usize = Self::OFFSET_TIME + 16;
    pub const OFFSET_PAYLOAD: usize = Self::OFFSET_DEPTH + 24;
    pub const IDEAL_SIZE: usize = Self::OFFSET_PAYLOAD;

    const PREDEFINED_UPSERT: [u8; RawLinkState::IDEAL_SIZE] = [
        /* KIND */
        UNSET, /* LAST SIGN. KIND */
        UNSET, /* RECRAWL */
        UNSET, /* IS_SEED */
        UNSET, /* TIME */
        0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8,
        /* DEPTH */
        0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8,
        0u8, 0u8, 0u8, 0u8, 0u8, 0u8,
    ];

    pub fn new() -> Self {
        Self {
            inner: SmallVec::from_const(Self::PREDEFINED_UPSERT),
        }
    }

    pub fn new_preconfigured_upsert_no_payload(
        url: &UrlWithDepth,
        kind: LinkStateKind,
        is_seed: Option<IsSeedYesNo>,
        recrawl: Option<RecrawlYesNo>,
    ) -> Self {
        Self::new_preconfigured_upsert(url, kind, is_seed, recrawl, None::<Option<&[u8]>>)
    }

    pub fn new_preconfigured_upsert<P>(
        url: &UrlWithDepth,
        kind: LinkStateKind,
        is_seed: Option<IsSeedYesNo>,
        recrawl: Option<RecrawlYesNo>,
        payload: Option<Option<P>>,
    ) -> Self
    where
        P: AsRef<[u8]>,
    {
        let mut new = Self::new();
        new.set_kind(kind);
        new.set_timestamp(OffsetDateTime::now_utc());
        new.set_depth(url.depth());
        if let Some(is_seed) = is_seed {
            new.set_is_seed(is_seed);
        }
        if let Some(recrawl) = recrawl {
            new.set_recrawl(recrawl);
        }
        if let Some(payload) = payload {
            new.set_payload(payload);
        }
        new
    }

    pub fn from_link_state(link_state: &LinkState) -> Self {
        let mut result = [0u8; RawLinkState::IDEAL_SIZE];
        result[RawLinkState::KIND_POS] = link_state.kind.into();
        result[RawLinkState::LAST_SIGNIFICANT_KIND_POS] = link_state.last_significant_kind.into();
        result[RawLinkState::RECRAWL_POS] = link_state.recrawl.into();
        result[RawLinkState::IS_SEED_POS] = link_state.is_seed.into();
        Self::write_timestamp(&mut result, &link_state.timestamp);
        Self::write_depth_descriptor(&mut result, &link_state.depth);
        let mut inner = SmallVec::from_const(result);
        match &link_state.payload {
            None => {}
            Some(value) => {
                inner.extend_from_slice(value);
            }
        }
        Self { inner }
    }

    pub fn from_slice(slice: &[u8]) -> Result<Self, LinkStateError> {
        let new = unsafe { Self::from_slice_unchecked(slice) };
        new.check()?;
        Ok(new)
    }

    pub unsafe fn from_slice_unchecked(slice: &[u8]) -> Self {
        Self {
            inner: SmallVec::from_slice(slice),
        }
    }

    pub fn from_vec(value: Vec<u8>) -> Result<Self, LinkStateError> {
        let new = unsafe { Self::from_vec_unchecked(value) };
        new.check()?;
        Ok(new)
    }

    pub unsafe fn from_vec_unchecked(value: Vec<u8>) -> Self {
        Self {
            inner: SmallVec::from_vec(value),
        }
    }

    fn check(&self) -> Result<(), LinkStateError> {
        Self::read_kind(self)?;
        Self::read_last_significant_kind(self)?;
        Self::read_recrawl(self)?;
        Self::read_is_seed(self)?;
        Self::read_timestamp(self)?;
        Self::read_depth_desc(self)?;
        Ok(())
    }

    #[inline]
    fn write_timestamp(target: &mut [u8], time: &OffsetDateTime) {
        (&mut target[Self::OFFSET_TIME..Self::OFFSET_DEPTH])
            .copy_from_slice(&time.unix_timestamp_nanos().to_be_bytes())
    }

    #[inline]
    fn write_depth_descriptor(target: &mut [u8], depth: &Depth) {
        (&mut target[Self::OFFSET_DEPTH..Self::OFFSET_DEPTH + 8])
            .copy_from_slice(&depth.depth_on_website.to_be_bytes());
        (&mut target[Self::OFFSET_DEPTH + 8..Self::OFFSET_DEPTH + 16])
            .copy_from_slice(&depth.distance_to_seed.to_be_bytes());
        (&mut target[Self::OFFSET_DEPTH + 16..Self::OFFSET_PAYLOAD])
            .copy_from_slice(&depth.total_distance_to_seed.to_be_bytes());
    }

    #[inline]
    pub fn read_kind(buffer: &[u8]) -> Result<LinkStateKind, LinkStateError> {
        if buffer.is_empty() {
            Err(LinkStateError::EmptyBuffer)
        } else {
            Ok(buffer[Self::KIND_POS].into())
        }
    }

    #[inline]
    pub fn read_last_significant_kind(buffer: &[u8]) -> Result<LinkStateKind, LinkStateError> {
        if buffer.len() < Self::OFFSET_TIME {
            Err(LinkStateError::BufferTooSmall(
                Self::OFFSET_TIME,
                buffer.len(),
            ))
        } else {
            Ok(buffer[Self::LAST_SIGNIFICANT_KIND_POS].into())
        }
    }

    #[inline]
    pub fn read_recrawl(buffer: &[u8]) -> Result<RecrawlYesNo, LinkStateError> {
        if buffer.is_empty() {
            Err(LinkStateError::EmptyBuffer)
        } else {
            Ok(buffer[Self::RECRAWL_POS].into())
        }
    }

    #[inline]
    pub fn read_is_seed(buffer: &[u8]) -> Result<IsSeedYesNo, LinkStateError> {
        if buffer.is_empty() {
            Err(LinkStateError::EmptyBuffer)
        } else {
            Ok(buffer[Self::IS_SEED_POS].into())
        }
    }

    #[inline]
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

    #[inline]
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

    #[inline(always)]
    fn fold_merge_linkstate(merge_result: &mut Vec<u8>, key: &[u8], operand: &[u8]) {
        if operand.is_empty() {
            return;
        }
        if merge_result.is_empty() {
            merge_result.extend_from_slice(operand);
            return;
        }

        let upsert_time = Self::read_timestamp(&merge_result);
        let new_time = Self::read_timestamp(operand);

        let upsert_time = if let Ok(upsert_time) = upsert_time {
            upsert_time
        } else {
            if new_time.is_ok() {
                log::error!("Illegal value for {:?}. Does not contain a timestamp in the merge target, but can fallback to new!", key);
                merge_result.clear();
                merge_result.extend_from_slice(operand);
            } else {
                log::error!("Illegal value for {:?}. Does not contain a timestamp in the merge target or the new value!", key);
            }
            return;
        };

        let new_time = if let Ok(new_time) = new_time {
            new_time
        } else {
            log::error!(
                "Illegal value for {:?}. Does not contain a timestamp in the new value!",
                key
            );
            return;
        };

        if upsert_time < new_time {
            let last_significant = merge_result[Self::KIND_POS];

            let recrawl = if RecrawlYesNo::is_significant_raw(operand[Self::RECRAWL_POS]) {
                operand[Self::RECRAWL_POS]
            } else {
                merge_result[Self::RECRAWL_POS]
            };

            let is_seed = if IsSeedYesNo::is_significant_raw(operand[Self::IS_SEED_POS]) {
                operand[Self::IS_SEED_POS]
            } else {
                merge_result[Self::IS_SEED_POS]
            };

            merge_result.clear();
            merge_result.extend_from_slice(operand);

            merge_result[Self::LAST_SIGNIFICANT_KIND_POS] = last_significant;
            merge_result[Self::RECRAWL_POS] = recrawl;
            merge_result[Self::IS_SEED_POS] = is_seed;
        }
    }

    #[cfg(test)]
    pub fn fold_merge_linkstate_test(merge_result: &mut Vec<u8>, key: &[u8], operand: &[u8]) {
        Self::fold_merge_linkstate(merge_result, key, operand)
    }

    #[cfg(test)]
    pub fn merge_linkstate_simulated<I, T>(
        key: impl AsRef<[u8]>,
        existing_val: Option<impl AsRef<[u8]>>,
        operands: I,
    ) -> Option<Vec<u8>>
    where
        I: IntoIterator<Item = T>,
        T: AsRef<[u8]>,
    {
        let mut merge_result = if let Some(first) = existing_val {
            Vec::from(first.as_ref())
        } else {
            Vec::new()
        };

        for operand in operands {
            Self::fold_merge_linkstate(&mut merge_result, key.as_ref(), operand.as_ref());
        }
        Some(merge_result)
    }

    /// Merge action for a rockdb
    pub fn merge_linkstate(
        key: &[u8],
        existing_val: Option<&[u8]>,
        operands: &MergeOperands,
    ) -> Option<Vec<u8>> {
        let mut merge_result = if let Some(first) = existing_val {
            Vec::from(first)
        } else {
            Vec::new()
        };
        for operand in operands {
            Self::fold_merge_linkstate(&mut merge_result, key, operand);
        }
        Some(merge_result)
    }
}

impl LinkStateLike for RawLinkState {
    type Error = LinkStateError;

    fn len(&self) -> usize {
        self.inner.len()
    }

    fn set_kind(&mut self, kind: LinkStateKind) {
        self.inner[Self::KIND_POS] = kind.into();
    }
    fn set_last_significant_kind(&mut self, kind: LinkStateKind) {
        self.inner[Self::LAST_SIGNIFICANT_KIND_POS] = kind.into();
    }

    fn set_recrawl(&mut self, kind: RecrawlYesNo) {
        self.inner[Self::RECRAWL_POS] = kind.into();
    }

    fn set_timestamp(&mut self, time: OffsetDateTime) {
        Self::write_timestamp(&mut self.inner, &time)
    }

    fn set_depth(&mut self, depth: &Depth) {
        Self::write_depth_descriptor(&mut self.inner, depth)
    }

    fn set_payload(&mut self, payload: Option<impl AsRef<[u8]>>) {
        if let Some(payload) = payload {
            self.inner.extend_from_slice(payload.as_ref())
        } else {
            self.inner.truncate(RawLinkState::IDEAL_SIZE)
        }
    }

    fn kind(&self) -> LinkStateKind {
        Self::read_kind(self).unwrap()
    }

    fn last_significant_kind(&self) -> LinkStateKind {
        Self::read_last_significant_kind(self).unwrap()
    }

    fn recrawl(&self) -> RecrawlYesNo {
        Self::read_recrawl(self).unwrap()
    }

    fn timestamp(&self) -> OffsetDateTime {
        Self::read_timestamp(self).unwrap()
    }

    fn depth(&self) -> Depth {
        Self::read_depth_desc(self).unwrap()
    }

    fn payload(&self) -> Option<&[u8]> {
        if self.len() <= Self::IDEAL_SIZE {
            None
        } else {
            Some(&self[Self::OFFSET_PAYLOAD..])
        }
    }

    fn as_bytes(&self) -> Cow<[u8]> {
        Cow::Borrowed(self.as_ref())
    }

    fn as_raw_link_state(&self) -> Cow<RawLinkState> {
        Cow::Borrowed(self)
    }

    fn as_link_state(&self) -> Cow<LinkState> {
        let kind: LinkStateKind = RawLinkState::read_kind(self).unwrap();
        let last_significant_kind: LinkStateKind =
            RawLinkState::read_last_significant_kind(self).unwrap();
        let recrawl = RawLinkState::read_recrawl(self).unwrap();
        let is_seed = RawLinkState::read_is_seed(self).unwrap();
        let timestamp = RawLinkState::read_timestamp(self).unwrap();
        let depth = RawLinkState::read_depth_desc(self).unwrap();
        let payload = RawLinkState::read_optional_payload(self);
        Cow::Owned(LinkState {
            payload,
            depth,
            is_seed,
            timestamp,
            kind,
            last_significant_kind,
            recrawl,
        })
    }

    fn to_raw_link_state(self) -> RawLinkState {
        self
    }

    fn eq_without_special_fields(&self, other: &Self) -> bool {
        self[Self::KIND_POS] == other[Self::KIND_POS]
            && self[Self::RECRAWL_POS..Self::OFFSET_TIME]
                == other[Self::RECRAWL_POS..Self::OFFSET_TIME]
            && self[Self::OFFSET_DEPTH..] == other[Self::OFFSET_DEPTH..]
    }

    fn set_is_seed(&mut self, is_seed: IsSeedYesNo) {
        self.inner[Self::IS_SEED_POS] = is_seed.into();
    }

    fn is_seed(&self) -> IsSeedYesNo {
        Self::read_is_seed(&self.inner).unwrap()
    }
}

impl AsRef<[u8]> for RawLinkState {
    fn as_ref(&self) -> &[u8] {
        &self.inner
    }
}

impl Deref for RawLinkState {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl PartialEq<LinkState> for RawLinkState {
    fn eq(&self, other: &LinkState) -> bool {
        other.eq(self)
    }
}
