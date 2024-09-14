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

use crate::blacklist::manage::ManagedBlacklist;
use crate::blacklist::traits::ManageableBlacklist;
use thiserror::Error;

pub mod manager_impl;

/// A manager for a blacklist
#[allow(dead_code)]
pub trait BlacklistManager {
    type Blacklist: ManageableBlacklist;

    async fn get_blacklist(&self) -> ManagedBlacklist<Self::Blacklist>;

    async fn current_version(&self) -> u64;

    async fn add(&self, value: String) -> Result<bool, BlacklistError>;

    async fn apply_patch<I: IntoIterator<Item = String>>(&self, patch: I);

    async fn get_patch(&self, since_version: u64) -> Option<Vec<String>>;

    async fn is_empty(&self) -> bool;
}

/// Blacklist error
#[derive(Debug, Copy, Clone, Error)]
pub enum BlacklistError {
    /// A blacklist entry can not contain a newline.
    #[error("Tried to add something with a new line separator to the queue.")]
    NewLinesNotAllowed,
    /// A blacklist entry can not not be empty.
    #[error("Tried to add an empty string to the queue")]
    EmptyStringsNotAllowed,
}
