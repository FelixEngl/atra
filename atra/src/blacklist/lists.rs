// Copyright 2024 Felix Engl
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

use crate::blacklist::traits::{Blacklist, BlacklistType};
use crate::blacklist::ManageableBlacklist;
use regex::RegexSet;
use std::str::FromStr;
use thiserror::Error;

/// The poly version for all default blacklists.
#[derive(Clone, Debug)]
pub enum PolyBlackList {
    Regex(RegexBlackList),
    Empty(EmptyBlackList),
}

#[derive(Debug, Error)]
pub enum PolyBlackListError {
    #[error("An error during the regex blacklist creation occured.")]
    Regex(#[from] <RegexBlackList as BlacklistType>::Error),
}

impl Default for PolyBlackList {
    fn default() -> Self {
        Self::Empty(EmptyBlackList::default())
    }
}

impl ManageableBlacklist for PolyBlackList {}

impl Blacklist for PolyBlackList {
    delegate::delegate! {
        to match &self {
            Self::Regex(a) => a,
            Self::Empty(a) => a,
        } {
            fn version(&self) -> u64;
            fn has_match_for(&self, url: &str) -> bool;
        }
    }
}

impl From<RegexBlackList> for PolyBlackList {
    fn from(value: RegexBlackList) -> Self {
        Self::Regex(value)
    }
}

impl From<EmptyBlackList> for PolyBlackList {
    fn from(value: EmptyBlackList) -> Self {
        Self::Empty(value)
    }
}

impl BlacklistType for PolyBlackList {
    type Error = PolyBlackListError;

    fn new<S, I>(version: u64, src: I) -> Result<Self, Self::Error>
    where
        S: AsRef<str>,
        I: IntoIterator<Item = S>,
    {
        let mut peekable = src.into_iter().peekable();
        Ok(if peekable.peek().is_none() {
            Self::Empty(EmptyBlackList(version))
        } else {
            Self::Regex(RegexBlackList::new(version, peekable)?)
        })
    }
}

/// An empty blacklist that never matches anything
#[derive(Debug, Clone, Copy, Default)]
pub struct EmptyBlackList(u64);

impl ManageableBlacklist for EmptyBlackList {}

impl Blacklist for EmptyBlackList {
    fn version(&self) -> u64 {
        self.0
    }

    #[inline(always)]
    fn has_match_for(&self, _: &str) -> bool {
        false
    }
}

impl BlacklistType for EmptyBlackList {
    type Error = std::convert::Infallible;

    fn new<S, I>(version: u64, _: I) -> Result<Self, Self::Error>
    where
        S: AsRef<str>,
        I: IntoIterator<Item = S>,
    {
        Ok(EmptyBlackList(version))
    }
}

/// A regex based blacklist, may match something
#[derive(Debug, Clone)]
pub struct RegexBlackList {
    version: u64,
    inner: RegexSet,
}

impl ManageableBlacklist for RegexBlackList {}

impl Blacklist for RegexBlackList {
    fn version(&self) -> u64 {
        self.version
    }

    fn has_match_for(&self, url: &str) -> bool {
        self.inner.is_match(url)
    }
}

impl BlacklistType for RegexBlackList {
    type Error = regex::Error;

    fn new<S, I>(version: u64, src: I) -> Result<Self, Self::Error>
    where
        S: AsRef<str>,
        I: IntoIterator<Item = S>,
    {
        Ok(Self {
            version,
            inner: RegexSet::new(src)?,
        })
    }
}

impl FromStr for RegexBlackList {
    type Err = regex::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            version: 0,
            inner: RegexSet::new(std::iter::once(s))?,
        })
    }
}

impl Default for RegexBlackList {
    fn default() -> Self {
        Self {
            version: 0,
            inner: RegexSet::empty(),
        }
    }
}
