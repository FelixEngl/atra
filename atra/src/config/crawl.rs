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

// Inspired by spider_rs

use crate::extraction::extractor::Extractor;
use crate::gdbr::identifier::GdbrIdentifierRegistryConfig;
use crate::toolkit::header_map_extensions::optional_header_map;
use crate::url::{AtraUrlOrigin, UrlWithDepth};
use reqwest::header::HeaderMap;
use serde;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
use std::num::NonZeroU64;
use strum::Display;
use strum::EnumString;
use text_processing::configs::StopwordRegistryConfig;
use text_processing::tf_idf::{Idf, Tf};
use thiserror::Error;
use time::Duration;

/// The general crawling settings for a single
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename(serialize = "Crawl"))]
#[serde(default)]
pub struct CrawlConfig {
    /// The user agent used by the crawler
    pub user_agent: UserAgent,
    /// Respect robots.txt file and not scrape not allowed files. This may slow down crawls if
    /// robots.txt file has a delay included. (default: true)
    pub respect_robots_txt: bool,

    /// Respect the nofollow attribute during the link extraction (default: true)
    pub respect_nofollow: bool,
    /// Extract links to embedded data like audio/video files for the crawl-queue (default: false)
    pub crawl_embedded_data: bool,
    /// Extract links to/from javascript files for the crawl-queue (default: true)
    pub crawl_javascript: bool,
    /// Try to extract links from tags with onclick attribute for the crawl-queue (default: false)
    pub crawl_onclick_by_heuristic: bool,
    /// Tries to apply a gdpr filter if one is defined.
    pub apply_gdbr_filter_if_possible: bool,
    /// Only store html-files in the warc
    pub store_only_html_in_warc: bool,
    /// Store the big file hints also in the warc
    pub store_big_file_hints_in_warc: bool,

    /// The maximum size to download. (in byte)
    pub max_file_size: Option<NonZeroU64>,

    /// The maximum age of a cached robots.txt. If None, it never gets too old.
    pub max_robots_age: Option<Duration>,
    /// Prevent including the sitemap links with the crawl.
    pub ignore_sitemap: bool,
    /// Allow sub-domains.
    pub subdomains: bool,

    /// Cache the page following HTTP caching rules.
    pub cache: bool,
    /// Use cookies
    pub use_cookies: bool,
    /// Domain bound cookie config
    /// Cookie string to use for network requests ex: "foo=bar; Domain=blog.spider"
    pub cookies: Option<CookieSettings>,

    /// Headers to include with requests.
    #[serde(with = "optional_header_map")]
    pub headers: Option<HeaderMap>,
    /// Use proxy list for performing network request.
    pub proxies: Option<Vec<String>>,
    /// Allow all tlds for domain.
    pub tld: bool,
    /// Polite crawling delay
    pub delay: Option<Duration>,
    /// The budget settings for this crawl
    pub budget: CrawlBudget,
    /// How often can we fail to crawl an entry in the queue until it is dropped? (0 means never drop)
    /// By default 20
    pub max_queue_age: u32,

    /// The max redirections allowed for request. (default: 5 like Google-Bot)
    pub redirect_limit: usize,
    /// The redirect policy type to use.
    pub redirect_policy: RedirectPolicy,

    /// Dangerously accept invalid certficates
    pub accept_invalid_certs: bool,

    /// A custom configuration of extractors
    pub link_extractors: Extractor,

    /// If this value is set Atra tries to decode and process files that are only downloaded as
    /// blob but do not overstep this provided size. (in Bytes) (default: None/Off)
    pub decode_big_files_up_to: Option<u64>,

    /// Used to configure the stopword registry if needed.
    pub stopword_registry: Option<StopwordRegistryConfig>,

    /// Used to configure the gdbr feature
    pub gbdr: Option<GdbrIdentifierRegistryConfig<Tf, Idf>>,
}

impl Default for CrawlConfig {
    fn default() -> Self {
        Self {
            respect_robots_txt: true,
            ignore_sitemap: false,
            user_agent: UserAgent::default(),
            respect_nofollow: true,
            crawl_embedded_data: false,
            crawl_javascript: true,
            crawl_onclick_by_heuristic: false,
            store_only_html_in_warc: true,
            store_big_file_hints_in_warc: false,
            apply_gdbr_filter_if_possible: true,
            headers: None,
            delay: None,
            cache: false,
            proxies: None,
            tld: false,
            accept_invalid_certs: false,
            use_cookies: true,
            redirect_policy: RedirectPolicy::default(),
            redirect_limit: 5,
            budget: CrawlBudget::default(),
            subdomains: false,
            max_robots_age: None,
            cookies: None,
            max_file_size: None,
            max_queue_age: 20,
            link_extractors: Extractor::default(),
            decode_big_files_up_to: None,
            stopword_registry: None,
            gbdr: None,
        }
    }
}

/// The cookie settings for each host.
#[derive(Debug, Default, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct CookieSettings {
    pub default: Option<String>,
    pub per_host: Option<HashMap<AtraUrlOrigin, String>>,
}

impl CookieSettings {
    /// Checks if the domain has some kind of configured cookie
    pub fn get_cookies_for<Q: ?Sized>(&self, domain: &Q) -> Option<&String>
    where
        AtraUrlOrigin: Borrow<Q>,
        Q: Hash + Eq,
    {
        if let Some(ref per_domain) = self.per_host {
            per_domain.get(domain).or_else(|| self.default.as_ref())
        } else {
            if let Some(ref default) = self.default {
                Some(default)
            } else {
                None
            }
        }
    }
}

/// Redirect policy configuration for request
#[derive(Debug, Default, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub enum RedirectPolicy {
    #[default]
    /// A loose policy that allows all request up to the redirect limit.
    Loose,
    /// A strict policy only allowing request that match the domain set for crawling.
    Strict,
}

/// The selected user agent
#[derive(Debug, Default, Clone, Deserialize, Serialize, EnumString, Display, Eq, PartialEq)]
pub enum UserAgent {
    /// Spoofs the user agent with a random useragent every time called.
    #[strum(ascii_case_insensitive = true)]
    Spoof,
    /// Uses the default user agent
    #[default]
    #[strum(ascii_case_insensitive = true)]
    Default,
    /// Uses a custom user agent
    #[strum(default, ascii_case_insensitive = true)]
    Custom(String),
}

impl UserAgent {
    const DEFAULT_UA: &'static str = concat!(
        "Crawler/",
        env!("CARGO_PKG_NAME"),
        "/",
        env!("CARGO_PKG_VERSION")
    );

    /// Returns the useragent string
    pub fn get_user_agent(&self) -> &str {
        match self {
            UserAgent::Spoof => ua_generator::ua::spoof_ua(),
            UserAgent::Default => UserAgent::DEFAULT_UA,
            UserAgent::Custom(user_agent) => user_agent,
        }
    }
}

impl AsRef<str> for UserAgent {
    fn as_ref(&self) -> &str {
        self.get_user_agent()
    }
}

/// The budget for each host.
#[derive(Debug, Default, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct CrawlBudget {
    pub default: BudgetSetting,
    pub per_host: Option<HashMap<AtraUrlOrigin, BudgetSetting>>,
}

impl CrawlBudget {
    pub fn get_budget_for<Q: ?Sized>(&self, origin: &Q) -> &BudgetSetting
    where
        AtraUrlOrigin: Borrow<Q>,
        Q: Hash + Eq,
    {
        match self.per_host {
            None => &self.default,
            Some(ref found) => found.get(origin).unwrap_or(&self.default),
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
