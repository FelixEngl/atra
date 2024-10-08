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

use crate::client::traits::AtraClient;
use crate::robots::{CachedRobots, RobotsError};
use crate::url::UrlWithDepth;
use std::error::Error;
use std::sync::Arc;
use time::Duration;

/// The basics that share all robots manager
pub trait RobotsManager: Sync + Send {
    /// A faster version of `get_or_retrieve` where no client is needed.
    /// Returns None if there is no robots.txt in any cache level.
    async fn get<E: Error>(
        &self,
        agent: &str,
        url: &UrlWithDepth,
        max_age: Option<&Duration>,
    ) -> Result<Option<Arc<CachedRobots>>, RobotsError<E>>;

    /// Uses a mutex internally, therefore you should cache the returned value in your task.
    /// If nothing is in any cache it downloads the robots.txt with the client.
    async fn get_or_retrieve<C: AtraClient>(
        &self,
        client: &C,
        agent: &str,
        url: &UrlWithDepth,
        max_age: Option<&Duration>,
    ) -> Result<Arc<CachedRobots>, RobotsError<C::Error>>;
}
