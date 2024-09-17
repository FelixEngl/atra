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

use num_enum::{FromPrimitive, IntoPrimitive};
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumIs};

/// The default value for unset markers
pub const UNSET: u8 = u8::MAX - 1;

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
    Unset = UNSET,
    /// An unknown type
    #[num_enum(catch_all)]
    Unknown(u8),
}

impl LinkStateKind {
    pub fn is_significant_raw(value: u8) -> bool {
        value <= 3u8
    }

    pub fn is_significant(&self) -> bool {
        *self <= Self::ProcessedAndStored
    }
}

macro_rules! yes_no_kind {
    ($name: ident) => {
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
        pub enum $name {
            No = 0u8,
            Yes = 1u8,
            /// The value if unset, usually only used for updates.
            Unset = UNSET,
            #[num_enum(catch_all)]
            Unknown(u8),
        }

        impl $name {
            pub fn is_significant_raw(value: u8) -> bool {
                value <= 1u8
            }

            pub fn is_significant(&self) -> bool {
                *self <= Self::Yes
            }
        }

        impl From<Option<bool>> for $name {
            #[inline]
            fn from(value: Option<bool>) -> Self {
                if let Some(value) = value {
                    value.into()
                } else {
                    $name::Unset
                }
            }
        }

        impl From<bool> for $name {
            #[inline]
            fn from(value: bool) -> Self {
                (value as u8).into()
            }
        }
    };
}

yes_no_kind!(RecrawlYesNo);
yes_no_kind!(IsSeedYesNo);

#[cfg(test)]
mod test {
    use crate::link_state::kind::RecrawlYesNo;
    use crate::link_state::LinkStateKind;

    #[test]
    pub fn recrawl_kind_conversion_correct() {
        assert_eq!(RecrawlYesNo::No, false.into());
        assert_eq!(RecrawlYesNo::No, Some(false).into());
        assert_eq!(RecrawlYesNo::Yes, true.into());
        assert_eq!(RecrawlYesNo::Yes, Some(true).into());
        assert_eq!(RecrawlYesNo::Unset, None.into());
        assert_eq!(RecrawlYesNo::Unknown(123), 123.into());
    }

    #[test]
    pub fn recrawl_kind_comparisons_work() {
        assert!(RecrawlYesNo::is_significant_raw(RecrawlYesNo::No.into()));
        assert!(RecrawlYesNo::is_significant_raw(RecrawlYesNo::Yes.into()));
        assert!(!RecrawlYesNo::is_significant_raw(
            RecrawlYesNo::Unset.into()
        ));
        assert!(!RecrawlYesNo::is_significant_raw(
            RecrawlYesNo::Unknown(123).into()
        ));
    }

    #[test]
    pub fn link_state_comparisons_work() {
        assert!(LinkStateKind::is_significant_raw(
            LinkStateKind::Discovered.into()
        ));
        assert!(LinkStateKind::is_significant_raw(
            LinkStateKind::ReservedForCrawl.into()
        ));
        assert!(LinkStateKind::is_significant_raw(
            LinkStateKind::Crawled.into()
        ));
        assert!(LinkStateKind::is_significant_raw(
            LinkStateKind::ProcessedAndStored.into()
        ));
        assert!(!LinkStateKind::is_significant_raw(
            LinkStateKind::InternalError.into()
        ));
        assert!(!LinkStateKind::is_significant_raw(
            LinkStateKind::Unset.into()
        ));
        assert!(!LinkStateKind::is_significant_raw(
            LinkStateKind::Unknown(123).into()
        ));
    }
}
