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

use crate::link_state::kind::LinkStateKind;
use crate::link_state::LinkStateError;
use crate::url::Depth;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use time::OffsetDateTime;

/// The state of a link
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash)]
pub struct LinkState {
    pub kind: LinkStateKind,
    pub last_significant_typ: LinkStateKind,
    pub timestamp: OffsetDateTime,
    pub depth: Depth,
    pub payload: Option<Vec<u8>>,
}

impl LinkState {
    pub fn new(
        typ: LinkStateKind,
        last_significant_typ: LinkStateKind,
        timestamp: OffsetDateTime,
        depth: Depth,
        payload: Option<Vec<u8>>,
    ) -> Self {
        Self {
            kind: typ,
            last_significant_typ,
            timestamp,
            depth,
            payload,
        }
    }

    pub fn with_payload(
        kind: LinkStateKind,
        last_significant_typ: LinkStateKind,
        timestamp: OffsetDateTime,
        depth: Depth,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            kind,
            last_significant_typ,
            timestamp,
            depth,
            payload: Some(payload),
        }
    }

    pub fn without_payload(
        typ: LinkStateKind,
        last_significant_typ: LinkStateKind,
        timestamp: OffsetDateTime,
        depth: Depth,
    ) -> Self {
        Self {
            kind: typ,
            last_significant_typ,
            timestamp,
            depth,
            payload: None,
        }
    }

    pub fn update_in_place(&mut self, update: Self) {
        let lst = if self.kind.is_significant() {
            self.kind
        } else {
            self.last_significant_typ
        };
        self.kind = update.kind;
        self.last_significant_typ = lst;
        self.timestamp = update.timestamp;
        self.payload = update.payload;
        self.depth = update.depth;
    }
}

impl LinkState {
    pub const KIND_POS: usize = 0;
    pub const LAST_SIGNIFICANT_TYP_POS: usize = 1;
    pub const OFFSET_TIME: usize = Self::KIND_POS + 2;
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
        result[Self::KIND_POS] = self.kind.into();
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

    pub fn read_kind(buffer: &[u8]) -> Result<LinkStateKind, LinkStateError> {
        if buffer.is_empty() {
            Err(LinkStateError::EmptyBuffer)
        } else {
            Ok(buffer[Self::KIND_POS].into())
        }
    }

    pub fn read_last_significant_typ(buffer: &[u8]) -> Result<LinkStateKind, LinkStateError> {
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
        let typ: LinkStateKind = Self::read_kind(buffer)?;
        let last_significant_typ: LinkStateKind = Self::read_last_significant_typ(buffer)?;
        let timestamp = Self::read_timestamp(buffer)?;
        let depth = Self::read_depth_desc(buffer)?;
        let payload = Self::read_optional_payload(buffer);

        Ok(Self {
            payload,
            depth,
            timestamp,
            kind: typ,
            last_significant_typ,
        })
    }
}
