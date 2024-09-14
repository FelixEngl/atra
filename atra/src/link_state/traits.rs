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
use crate::link_state::state::LinkState;
use crate::link_state::LinkStateDBError;
use crate::url::UrlWithDepth;
use std::error::Error;
use std::ops::RangeBounds;
use std::time::Duration;

/// Manages the linkstate
#[allow(dead_code)]
pub trait LinkStateManager {
    type Error: Error + Send + Sync;

    /// The number of crawled websites
    fn crawled_websites(&self) -> Result<u64, Self::Error>;

    /// Sets the state of the link
    async fn update_link_state(
        &self,
        url: &UrlWithDepth,
        state: LinkStateKind,
    ) -> Result<(), Self::Error>;

    /// Sets the state of the link with a payload
    async fn update_link_state_with_payload(
        &self,
        url: &UrlWithDepth,
        state: LinkStateKind,
        payload: Vec<u8>,
    ) -> Result<(), Self::Error>;

    /// Gets the state of the current url
    async fn get_link_state(&self, url: &UrlWithDepth) -> Result<Option<LinkState>, Self::Error>;

    /// Checks if there are any crawable links. [max_age] denotes the maximum amount of time since
    /// the last search
    async fn check_if_there_are_any_crawlable_links(&self, max_age: Duration) -> bool;
}

#[allow(dead_code)]
pub trait LinkStateDB {
    /// Sets the state of [url] to [new_state]
    fn set_state(&self, url: &UrlWithDepth, new_state: &LinkState) -> Result<(), LinkStateDBError>;

    /// Gets the state of [url] or None
    fn get_state(&self, url: &UrlWithDepth) -> Result<Option<LinkState>, LinkStateDBError>;

    /// Upserts the state of the [url] with [upsert].
    fn upsert_state(&self, url: &UrlWithDepth, upsert: &LinkState) -> Result<(), LinkStateDBError>;

    /// Basically an [upsert_state] but the update is automatically generated
    fn update_state(
        &self,
        url: &UrlWithDepth,
        new_state: LinkStateKind,
    ) -> Result<(), LinkStateDBError> {
        self.upsert_state(url, &new_state.into_update(url, None))
    }

    /// Counts the provided number of links state with the provided [LinkStateKind]
    fn count_state(&self, link_state_type: LinkStateKind) -> Result<u64, LinkStateDBError>;

    /// Basically an [upsert_state] but the update is automatically generated
    fn update_state_with_payload(
        &self,
        url: &UrlWithDepth,
        new_state: LinkStateKind,
        payload: Vec<u8>,
    ) -> Result<(), LinkStateDBError> {
        self.upsert_state(url, &new_state.into_update(url, Some(payload)))
    }

    /// Scans for any [states] in the underlying structure.
    /// This method is async due to its expensive nature.
    async fn scan_for_any_link_state<T: RangeBounds<LinkStateKind>>(&self, states: T) -> bool;
}
