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

use std::error::Error;
use std::fmt::Debug;

/// A blacklist helps to check if an url is valid or not.
pub trait Blacklist {
    /// The current version of the blacklist.
    fn version(&self) -> u64;

    /// Checks the [url] and returns true if this blacklist has a match for it.
    fn has_match_for(&self, url: &str) -> bool;
}

/// A simple type for a blacklist to initialize it.
pub trait BlacklistType<SelfT = Self>: Blacklist {
    type Error: Error;

    /// Creates a new blacklist from some kind of iterator over strings
    fn new<S, I>(version: u64, src: I) -> Result<SelfT, Self::Error>
    where
        S: AsRef<str>,
        I: IntoIterator<Item = S>;
}

/// A marker interface for a manageable list.
pub trait ManageableBlacklist: Blacklist + BlacklistType + Debug + Sized {}
