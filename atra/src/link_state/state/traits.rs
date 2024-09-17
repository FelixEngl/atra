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

use crate::link_state::state::in_memory::LinkState;
use crate::link_state::{IsSeedYesNo, LinkStateKind, RawLinkState, RecrawlYesNo};
use crate::url::Depth;
use std::borrow::Cow;
use time::OffsetDateTime;

pub trait LinkStateLike {
    type Error: std::error::Error;

    fn len(&self) -> usize;

    fn set_kind(&mut self, kind: LinkStateKind);

    fn set_last_significant_kind(&mut self, kind: LinkStateKind);

    fn set_recrawl(&mut self, kind: RecrawlYesNo);
    fn set_is_seed(&mut self, is_seed: IsSeedYesNo);

    fn set_timestamp(&mut self, time: OffsetDateTime);

    fn set_depth(&mut self, depth: &Depth);

    fn set_payload(&mut self, payload: Option<impl AsRef<[u8]>>);

    fn kind(&self) -> LinkStateKind;

    fn last_significant_kind(&self) -> LinkStateKind;

    fn recrawl(&self) -> RecrawlYesNo;
    fn is_seed(&self) -> IsSeedYesNo;

    fn timestamp(&self) -> OffsetDateTime;

    fn depth(&self) -> Depth;

    fn payload(&self) -> Option<&[u8]>;

    fn as_bytes(&self) -> Cow<[u8]>;

    fn to_raw_link_state(self) -> RawLinkState;
    fn as_raw_link_state(&self) -> Cow<RawLinkState>;
    fn as_link_state(&self) -> Cow<LinkState>;

    fn eq_without_special_fields(&self, other: &Self) -> bool;
}
