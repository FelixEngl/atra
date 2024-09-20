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
use crate::link_state::state::LinkStateLike;
use crate::link_state::{IsSeedYesNo, LinkStateDBError, RawLinkState, RecrawlYesNo};
use crate::url::UrlWithDepth;
use std::error::Error;
use std::ops::RangeBounds;
use std::time::Duration;

/// Manages the linkstate
pub trait LinkStateManager {
    type Error: Error + Send + Sync;

    /// The number of crawled websites
    fn crawled_websites(&self) -> Result<u64, Self::Error>;

    /// Sets the state of the link
    async fn update_link_state<P>(
        &self,
        url: &UrlWithDepth,
        state: LinkStateKind,
        is_seed: Option<IsSeedYesNo>,
        recrawl: Option<RecrawlYesNo>,
        payload: Option<Option<&P>>,
    ) -> Result<(), Self::Error>
    where
        P: ?Sized + AsRef<[u8]>;

    async fn update_link_state_no_payload(
        &self,
        url: &UrlWithDepth,
        state: LinkStateKind,
        is_seed: Option<IsSeedYesNo>,
        recrawl: Option<RecrawlYesNo>,
    ) -> Result<(), Self::Error> {
        self.update_link_state(url, state, is_seed, recrawl, None::<Option<&[u8]>>).await
    }

    async fn update_link_state_no_meta<P>(
        &self,
        url: &UrlWithDepth,
        state: LinkStateKind,
        payload: Option<Option<&P>>,
    ) -> Result<(), Self::Error>
    where
        P: ?Sized + AsRef<[u8]>,
    {
        self.update_link_state(url, state, None, None, payload)
            .await
    }

    async fn update_link_state_no_meta_and_payload(
        &self,
        url: &UrlWithDepth,
        state: LinkStateKind,
    ) -> Result<(), Self::Error> {
        self.update_link_state(url, state, None, None, None::<Option<&[u8]>>)
            .await
    }

    /// Gets the state of the current url
    async fn get_link_state(&self, url: &UrlWithDepth)
        -> Result<Option<RawLinkState>, Self::Error>;

    /// Checks if there are any crawable links. [max_age] denotes the maximum amount of time since
    /// the last search
    async fn check_if_there_are_any_crawlable_links(&self, max_age: Duration) -> bool;

    /// Checks if there are any recrawlable links
    async fn check_if_there_are_any_recrawlable_links(&self) -> bool;

    /// Returns the recrawlable links.
    async fn collect_recrawlable_links<F: Fn(IsSeedYesNo, UrlWithDepth) -> ()>(&self, collector: F);
    async fn collect_all_links<F: Fn(IsSeedYesNo, UrlWithDepth) -> ()>(&self, collector: F);
}

pub trait LinkStateDB {
    /// Sets the state of [url] to [new_state]
    fn set_state(
        &self,
        url: &UrlWithDepth,
        new_state: &impl LinkStateLike,
    ) -> Result<(), LinkStateDBError>;

    /// Gets the state of [url] or None
    fn get_state(&self, url: &UrlWithDepth) -> Result<Option<RawLinkState>, LinkStateDBError>;

    /// Upserts the state of the [url] with [upsert].
    fn upsert_state(
        &self,
        url: &UrlWithDepth,
        upsert: &impl LinkStateLike,
    ) -> Result<(), LinkStateDBError>;

    /// Basically an [upsert_state] but the update is automatically generated
    fn update_state(
        &self,
        url: &UrlWithDepth,
        new_state: LinkStateKind,
        is_seed: Option<IsSeedYesNo>,
        recrawl: Option<RecrawlYesNo>,
        payload: Option<Option<impl AsRef<[u8]>>>,
    ) -> Result<(), LinkStateDBError> {
        let new_upsert =
            RawLinkState::new_preconfigured_upsert(url, new_state, is_seed, recrawl, payload);
        self.upsert_state(url, &new_upsert)
    }

    fn update_state_no_payload(
        &self,
        url: &UrlWithDepth,
        new_state: LinkStateKind,
        is_seed: Option<IsSeedYesNo>,
        recrawl: Option<RecrawlYesNo>,
    ) -> Result<(), LinkStateDBError> {
        let new_upsert = RawLinkState::new_preconfigured_upsert(
            url,
            new_state,
            is_seed,
            recrawl,
            None::<Option<&[u8]>>,
        );
        self.upsert_state(url, &new_upsert)
    }

    /// Counts the provided number of links state with the provided [LinkStateKind]
    fn count_state(&self, link_state_type: LinkStateKind) -> Result<u64, LinkStateDBError>;

    /// Scans for any [states] in the underlying structure.
    /// This method is async due to its expensive nature.
    async fn scan_for_any_link_state<T: RangeBounds<LinkStateKind>>(&self, states: T) -> bool;

    async fn scan_for_value<F>(&self, scanner: F) -> bool
    where
        F: Fn(&[u8], &[u8]) -> bool;

    /// Collects as long as [collector] returns true.
    /// Is expensive because it does not support async.
    fn collect_values<F>(&self, collector: F)
    where
        F: Fn(u64, &[u8], &[u8]) -> bool;
}
