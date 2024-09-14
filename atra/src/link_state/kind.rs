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

use crate::link_state::LinkState;
use crate::url::UrlWithDepth;
use num_enum::{FromPrimitive, IntoPrimitive};
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumIs};
use time::OffsetDateTime;

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
pub enum LinkStateKind {
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

impl LinkStateKind {
    pub fn is_significant_raw(value: u8) -> bool {
        value < 4
    }

    pub fn is_significant(&self) -> bool {
        self < &LinkStateKind::InternalError
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
