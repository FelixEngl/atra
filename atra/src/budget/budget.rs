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

use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
use serde::{Deserialize, Serialize};
use strum::Display;
use thiserror::Error;
use time::Duration;
use crate::toolkit::in_memory_domain_manager::Content;
use crate::url::{AtraUrlOrigin, UrlWithDepth};

/// The budget for each host.
#[derive(Debug, Default, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct CrawlBudget {
    pub default: BudgetSetting,
    pub per_host: Option<HashMap<AtraUrlOrigin, BudgetSetting>>,
}

impl From<Content<BudgetSetting>> for CrawlBudget {
    fn from(value: Content<BudgetSetting>) -> Self {
        let (default, per_host) = value.into();
        CrawlBudget {
            default,
            per_host
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct BudgetSettingsDef {
    /// The max depth to crawl on a website.
    depth_on_website: Option<u64>,
    /// The maximum depth of websites, outgoing from the seed.
    depth: Option<u64>,
    /// Crawl interval (if none crawl only once)
    recrawl_interval: Option<Duration>,
    /// Request max timeout per page. By default the request times out in 15s. Set to None to disable.
    request_timeout: Option<Duration>,
}

impl From<BudgetSetting> for BudgetSettingsDef {
    fn from(value: BudgetSetting) -> Self {
        match value {
            BudgetSetting::SeedOnly {
                depth_on_website,
                request_timeout,
                recrawl_interval,
            } => Self {
                depth_on_website: Some(depth_on_website),
                depth: None,
                recrawl_interval,
                request_timeout,
            },
            BudgetSetting::Normal {
                depth_on_website,
                depth,
                request_timeout,
                recrawl_interval,
            } => Self {
                depth_on_website: Some(depth_on_website),
                depth: Some(depth),
                recrawl_interval,
                request_timeout,
            },
            BudgetSetting::Absolute {
                depth,
                request_timeout,
                recrawl_interval,
            } => Self {
                depth_on_website: None,
                depth: Some(depth),
                recrawl_interval,
                request_timeout,
            },
        }
    }
}

#[derive(Debug, Error)]
#[error("The budget is missing any depth field. It needs at least one!")]
struct BudgetSettingsDeserializationError;

impl TryFrom<BudgetSettingsDef> for BudgetSetting {
    type Error = BudgetSettingsDeserializationError;

    fn try_from(value: BudgetSettingsDef) -> Result<Self, Self::Error> {
        match value {
            BudgetSettingsDef {
                depth: Some(depth),
                depth_on_website: Some(depth_on_website),
                request_timeout,
                recrawl_interval,
            } => Ok(BudgetSetting::Normal {
                depth,
                depth_on_website,
                request_timeout,
                recrawl_interval,
            }),
            BudgetSettingsDef {
                depth_on_website: Some(depth_on_website),
                request_timeout,
                recrawl_interval,
                ..
            } => Ok(BudgetSetting::SeedOnly {
                depth_on_website,
                request_timeout,
                recrawl_interval,
            }),
            BudgetSettingsDef {
                depth: Some(depth),
                request_timeout,
                recrawl_interval,
                ..
            } => Ok(BudgetSetting::Absolute {
                depth,
                request_timeout,
                recrawl_interval,
            }),
            _ => Err(BudgetSettingsDeserializationError),
        }
    }
}

/// The budget for the crawled website
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq, Display)]
#[serde(try_from = "BudgetSettingsDef", into = "BudgetSettingsDef")]
pub enum BudgetSetting {
    /// Only crawls the seed domains
    SeedOnly {
        /// The max depth to crawl on a website. (0 indicates to crawl everything)
        depth_on_website: u64,
        /// Crawl interval (if none crawl only once)
        recrawl_interval: Option<Duration>,
        /// Request max timeout per page. By default the request times out in 15s. Set to None to disable.
        request_timeout: Option<Duration>,
    },
    /// Crawls the seed and follows external links
    Normal {
        /// The max depth to crawl on a website.
        depth_on_website: u64,
        /// The maximum depth of websites, outgoing from the seed.
        depth: u64,
        /// Crawl interval (if none crawl only once)
        recrawl_interval: Option<Duration>,
        /// Request max timeout per page. By default the request times out in 15s. Set to None to disable.
        request_timeout: Option<Duration>,
    },
    /// Crawls the seed and follows external links, but only follows until a specific amout of jumps is reached.
    Absolute {
        /// The absolute number of jumps, outgoing from the seed, 0 indicates infinite.
        depth: u64,
        /// Crawl interval (if none crawl only once)
        recrawl_interval: Option<Duration>,
        /// Request max timeout per page. By default the request times out in 15s. Set to None to disable.
        request_timeout: Option<Duration>,
    },
}

impl BudgetSetting {
    // pub fn is_finite(&self) -> bool {
    //     match self {
    //         BudgetSettings::SeedOnly { .. } => {true}
    //         BudgetSettings::Normal { .. } => {true}
    //         BudgetSettings::Absolute { depth, .. } => {0u64.eq(depth)}
    //     }
    // }

    pub fn get_request_timeout(&self) -> Option<Duration> {
        match self {
            BudgetSetting::SeedOnly {
                request_timeout, ..
            } => request_timeout,
            BudgetSetting::Normal {
                request_timeout, ..
            } => request_timeout,
            BudgetSetting::Absolute {
                request_timeout, ..
            } => request_timeout,
        }
            .clone()
    }

    pub fn get_recrawl_interval(&self) -> Option<&Duration> {
        match self {
            BudgetSetting::SeedOnly {
                recrawl_interval, ..
            } => recrawl_interval,
            BudgetSetting::Normal {
                recrawl_interval, ..
            } => recrawl_interval,
            BudgetSetting::Absolute {
                recrawl_interval, ..
            } => recrawl_interval,
        }
            .as_ref()
    }

    /// Returns true, iff the [url] is in the budget
    pub fn is_in_budget(&self, url: &UrlWithDepth) -> bool {
        let url_depth = url.depth();
        match self {
            BudgetSetting::SeedOnly {
                depth_on_website: depth,
                ..
            } => {
                url_depth.distance_to_seed == 0
                    && (0.eq(depth) || url_depth.depth_on_website.lt(depth))
            }
            BudgetSetting::Normal {
                depth_on_website: depth,
                depth: depth_distance,
                ..
            } => {
                (0.eq(depth) || url_depth.depth_on_website.lt(depth))
                    && url_depth.distance_to_seed.le(depth_distance)
            }
            BudgetSetting::Absolute { depth, .. } => {
                0.eq(depth) || url_depth.total_distance_to_seed.lt(depth)
            }
        }
    }
}

impl Default for BudgetSetting {
    fn default() -> Self {
        Self::Normal {
            depth_on_website: 20,
            depth: 3,
            recrawl_interval: None,
            request_timeout: Some(Duration::seconds(15)),
        }
    }
}
