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

use regex::RegexSet;
use std::error::Error;
use std::str::FromStr;
use thiserror::Error;

/// A blacklist helps to check if an url is valid or not.
pub trait BlackList: Clone + Default {
    /// The current version of the blacklist, if it returns None the blacklist was constructed without any manager
    /// backing.
    fn version(&self) -> Option<u64>;

    /// Checks the [url] and returns true if this blacklist has a match for it.
    fn has_match_for(&self, url: &str) -> bool;
}

/// A simple type for a blacklist to initialize it.
pub trait BlackListType<SelfT = Self>: BlackList {
    type Error: Error;

    /// Creates a new blacklist from some kind of iterator over strings
    fn new<S, I>(version: u64, src: I) -> Result<SelfT, Self::Error>
    where
        S: AsRef<str>,
        I: IntoIterator<Item = S>;
}

/// The poly version for all default blacklists.
#[derive(Clone, Debug)]
pub enum PolyBlackList {
    Regex(RegexBlackList),
    Empty(EmptyBlackList),
}

#[derive(Debug, Error)]
pub enum PolyBlackListError {
    #[error("An error during the regex blacklist creation occured.")]
    Regex(#[from] <RegexBlackList as BlackListType>::Error),
}

impl Default for PolyBlackList {
    fn default() -> Self {
        Self::Empty(EmptyBlackList::default())
    }
}

impl BlackList for PolyBlackList {
    delegate::delegate! {
        to match &self {
            Self::Regex(a) => a,
            Self::Empty(a) => a,
        } {
            fn version(&self) -> Option<u64>;
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

impl BlackListType for PolyBlackList {
    type Error = PolyBlackListError;

    fn new<S, I>(version: u64, src: I) -> Result<Self, Self::Error>
    where
        S: AsRef<str>,
        I: IntoIterator<Item = S>,
    {
        let mut peekable = src.into_iter().peekable();
        Ok(if peekable.peek().is_none() {
            Self::Empty(EmptyBlackList(Some(version)))
        } else {
            Self::Regex(RegexBlackList::new(version, peekable)?)
        })
    }
}

/// An empty blacklist that never matches anything
#[derive(Debug, Clone, Copy, Default)]
pub struct EmptyBlackList(Option<u64>);

impl BlackList for EmptyBlackList {
    fn version(&self) -> Option<u64> {
        self.0
    }

    #[inline(always)]
    fn has_match_for(&self, _: &str) -> bool {
        false
    }
}

impl BlackListType for EmptyBlackList {
    type Error = std::convert::Infallible;

    fn new<S, I>(version: u64, _: I) -> Result<Self, Self::Error>
    where
        S: AsRef<str>,
        I: IntoIterator<Item = S>,
    {
        Ok(EmptyBlackList(Some(version)))
    }
}

/// A regex based blacklist, may match something
#[derive(Debug, Clone)]
pub struct RegexBlackList {
    version: Option<u64>,
    inner: RegexSet,
}

impl BlackList for RegexBlackList {
    fn version(&self) -> Option<u64> {
        self.version
    }

    fn has_match_for(&self, url: &str) -> bool {
        self.inner.is_match(url)
    }
}

impl BlackListType for RegexBlackList {
    type Error = regex::Error;

    fn new<S, I>(version: u64, src: I) -> Result<Self, Self::Error>
    where
        S: AsRef<str>,
        I: IntoIterator<Item = S>,
    {
        Ok(Self {
            version: Some(version),
            inner: RegexSet::new(src)?,
        })
    }
}

impl FromStr for RegexBlackList {
    type Err = regex::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            version: None,
            inner: RegexSet::new(std::iter::once(s))?,
        })
    }
}

impl Default for RegexBlackList {
    fn default() -> Self {
        Self {
            version: None,
            inner: RegexSet::empty(),
        }
    }
}
