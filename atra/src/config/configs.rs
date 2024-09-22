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

use crate::config::crawl::CrawlConfig;
use crate::config::paths::PathsConfig;
use crate::config::session::SessionConfig;
use crate::config::SystemConfig;
use serde::{Deserialize, Serialize};

/// A collection of all config used in a crawl.
/// Can be shared across threads
#[derive(Debug, Default, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename(serialize = "Config"))]
pub struct Config {
    pub system: SystemConfig,
    pub paths: PathsConfig,
    pub session: SessionConfig,
    pub crawl: CrawlConfig,
}

impl Config {
    #[cfg(test)]
    pub fn new(
        system: SystemConfig,
        paths: PathsConfig,
        session: SessionConfig,
        crawl: CrawlConfig,
    ) -> Self {
        Self {
            system,
            paths,
            crawl,
            session,
        }
    }
}
