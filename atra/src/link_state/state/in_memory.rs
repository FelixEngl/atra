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

use crate::link_state::state::raw::RawLinkState;
use crate::link_state::state::traits::LinkStateLike;
use crate::link_state::{IsSeedYesNo, LinkStateError, LinkStateKind, RecrawlYesNo};
use crate::url::{Depth, UrlWithDepth};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::convert::Infallible;
use time::OffsetDateTime;

/// The state of a link
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash)]
pub struct LinkState {
    pub kind: LinkStateKind,
    pub last_significant_kind: LinkStateKind,
    pub recrawl: RecrawlYesNo,
    pub is_seed: IsSeedYesNo,
    pub timestamp: OffsetDateTime,
    pub depth: Depth,
    pub payload: Option<Vec<u8>>,
}

impl LinkState {
    pub fn new(
        typ: LinkStateKind,
        last_significant_typ: LinkStateKind,
        recrawl: RecrawlYesNo,
        is_seed: IsSeedYesNo,
        timestamp: OffsetDateTime,
        depth: Depth,
        payload: Option<Vec<u8>>,
    ) -> Self {
        Self {
            kind: typ,
            last_significant_kind: last_significant_typ,
            recrawl,
            is_seed,
            timestamp,
            depth,
            payload,
        }
    }

    pub fn with_payload(
        kind: LinkStateKind,
        last_significant_typ: LinkStateKind,
        recrawl: RecrawlYesNo,
        is_seed: IsSeedYesNo,
        timestamp: OffsetDateTime,
        depth: Depth,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            kind,
            last_significant_kind: last_significant_typ,
            recrawl,
            is_seed,
            timestamp,
            depth,
            payload: Some(payload),
        }
    }

    pub fn without_payload(
        typ: LinkStateKind,
        last_significant_typ: LinkStateKind,
        recrawl: RecrawlYesNo,
        is_seed: IsSeedYesNo,
        timestamp: OffsetDateTime,
        depth: Depth,
    ) -> Self {
        Self {
            kind: typ,
            last_significant_kind: last_significant_typ,
            recrawl,
            is_seed,
            timestamp,
            depth,
            payload: None,
        }
    }

    pub fn update_in_place(&mut self, update: Self) {
        let lst = if self.kind.is_significant() {
            self.kind
        } else {
            self.last_significant_kind
        };
        self.kind = update.kind;
        self.last_significant_kind = lst;
        self.recrawl = update.recrawl;
        self.is_seed = update.is_seed;
        self.timestamp = update.timestamp;
        self.payload = update.payload;
        self.depth = update.depth;
    }

    pub fn create_update(
        typ: LinkStateKind,
        url: &UrlWithDepth,
        recrawl: RecrawlYesNo,
        is_seed: IsSeedYesNo,
        payload: Option<Vec<u8>>,
    ) -> LinkState {
        LinkState::new(
            typ,
            LinkStateKind::Unset,
            recrawl,
            is_seed,
            OffsetDateTime::now_utc(),
            url.depth().clone(),
            payload,
        )
    }

    pub fn from_slice(slice: &[u8]) -> Result<Self, LinkStateError> {
        Ok(RawLinkState::from_slice(slice)?.into())
    }
}

impl From<RawLinkState> for LinkState {
    fn from(value: RawLinkState) -> Self {
        match value.as_link_state() {
            Cow::Owned(value) => value,
            _ => unreachable!(),
        }
    }
}

impl<'a> From<&'a RawLinkState> for LinkState {
    fn from(value: &'a RawLinkState) -> Self {
        match value.as_link_state() {
            Cow::Owned(value) => value,
            _ => unreachable!(),
        }
    }
}

impl LinkStateLike for LinkState {
    type Error = Infallible;

    fn kind(&self) -> LinkStateKind {
        self.kind
    }

    fn last_significant_kind(&self) -> LinkStateKind {
        self.last_significant_kind
    }

    fn recrawl(&self) -> RecrawlYesNo {
        self.recrawl
    }

    fn timestamp(&self) -> OffsetDateTime {
        self.timestamp
    }

    fn depth(&self) -> Depth {
        self.depth
    }

    fn payload(&self) -> Option<&[u8]> {
        Some(self.payload.as_ref()?.as_ref())
    }

    fn set_kind(&mut self, kind: LinkStateKind) {
        self.kind = kind;
    }

    fn set_last_significant_kind(&mut self, last_significant_kind: LinkStateKind) {
        self.last_significant_kind = last_significant_kind;
    }

    fn set_recrawl(&mut self, recrawl: RecrawlYesNo) {
        self.recrawl = recrawl;
    }

    fn set_timestamp(&mut self, time: OffsetDateTime) {
        self.timestamp = time;
    }

    fn set_depth(&mut self, depth: &Depth) {
        self.depth = depth.clone();
    }

    fn set_payload(&mut self, payload: Option<impl AsRef<[u8]>>) {
        self.payload = payload.map(|value| value.as_ref().to_vec());
    }

    fn as_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(RawLinkState::from_link_state(self).to_vec())
    }

    fn to_raw_link_state(self) -> RawLinkState {
        RawLinkState::from_link_state(&self)
    }

    fn len(&self) -> usize {
        if let Some(ref payload) = self.payload {
            RawLinkState::IDEAL_SIZE + payload.len()
        } else {
            RawLinkState::IDEAL_SIZE
        }
    }

    fn as_link_state(&self) -> Cow<LinkState> {
        Cow::Borrowed(self)
    }

    fn as_raw_link_state(&self) -> Cow<RawLinkState> {
        Cow::Owned(RawLinkState::from_link_state(&self))
    }

    fn eq_without_special_fields(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.recrawl == other.recrawl
            && self.is_seed == other.is_seed
            && self.depth == other.depth
            && self.payload == other.payload
    }

    fn set_is_seed(&mut self, is_seed: IsSeedYesNo) {
        self.is_seed = is_seed
    }

    fn is_seed(&self) -> IsSeedYesNo {
        self.is_seed
    }
}

impl PartialEq<RawLinkState> for LinkState {
    fn eq(&self, other: &RawLinkState) -> bool {
        other.kind() == self.kind
            && other.last_significant_kind() == self.last_significant_kind
            && other.recrawl() == self.recrawl
            && other.timestamp() == self.timestamp
            && other.depth() == self.depth
            && other.payload() == self.payload()
    }
}
