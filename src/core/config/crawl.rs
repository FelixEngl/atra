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

// Inspired by spider_rs

use std::collections::HashMap;
use std::num::{NonZeroU64};
use case_insensitive_string::CaseInsensitiveString;
use time::Duration;
use reqwest::header::HeaderMap;
use crate::core::header_map_extensions::optional_header_map;
use serde;
use serde::{Deserialize, Serialize};
use strum::{Display};
use strum::EnumString;
use crate::core::extraction::extractor::{Extractor};
use crate::core::UrlWithDepth;

/// The general crawling settings for a single
#[derive(Debug, Clone, Deserialize, Serialize)]
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

    /// The maximum size to download.
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
    #[serde(default)]
    #[serde(with = "optional_header_map")]
    pub headers: Option<HeaderMap>,
    /// Use proxy list for performing network request.
    #[serde(default)]
    pub proxies: Option<Vec<String>>,
    /// Allow all tlds for domain.
    pub tld: bool,
    /// Polite crawling delay
    #[serde(default)]
    pub delay: Option<Duration>,
    /// The budget settings for this crawl
    pub budget: CrawlBudget,
    /// How often can we fail to crawl an entry in the queue until it is dropped? (0 means never drop)
    /// By default 20
    pub max_queue_age: u32,

    /// The max redirections allowed for request. (default: 5 like Google-Bot)
    pub redirect_limit: usize,
    /// The redirect policy type to use.
    #[serde(default)]
    pub redirect_policy: RedirectPolicy,

    /// Dangerously accept invalid certficates
    pub accept_invalid_certs: bool,

    /// A custom configuration of extractors
    pub extractors: Extractor,

    /// If this value is set atra tries to decode and process files that are only downloaded as
    /// blob but do not overstep this provided size. (in Bytes) (default: None/Off)
    pub decode_big_files_up_to: Option<u64>,

    #[cfg(feature = "chrome")]
    /// The settings for a chrome instance
    pub chrome_settings: Option<ChromeSettings>,
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
            extractors: Extractor::default(),
            decode_big_files_up_to: None,
            #[cfg(feature = "chrome")]
            chrome_settings: Default::default()
        }
    }
}

/// The cookie settings for each host.
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct CookieSettings {
    pub default: Option<String>,
    pub per_domain: Option<HashMap<CaseInsensitiveString, String>>
}

impl CookieSettings {
    /// Checks if the domain has some kind of configured cookie
    pub fn get_cookies_for(&self, domain: &CaseInsensitiveString) -> Option<&String> {
        if let Some(ref per_domain) = self.per_domain {
            per_domain.get(domain)
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
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub enum RedirectPolicy {
    #[default]
    /// A loose policy that allows all request up to the redirect limit.
    Loose,
    /// A strict policy only allowing request that match the domain set for crawling.
    Strict,
}


/// The selected user agent
#[derive(Debug, Default, Clone, Deserialize, Serialize, EnumString, Display)]
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
    Custom(String)
}


impl UserAgent {

    const DEFAULT_UA: &'static str = concat!("Crawler/", env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

    /// Returns the useragent string
    pub fn get_user_agent(&self) -> &str {
        match self {
            UserAgent::Spoof => ua_generator::ua::spoof_ua(),
            UserAgent::Default => UserAgent::DEFAULT_UA,
            UserAgent::Custom(user_agent) => user_agent
        }
    }
}

impl AsRef<str> for UserAgent {
    fn as_ref(&self) -> &str {
        self.get_user_agent()
    }
}




/// The budget for each host.
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct CrawlBudget {
    pub default: BudgetSettings,
    pub per_host: Option<HashMap<CaseInsensitiveString, BudgetSettings>>
}

impl CrawlBudget {
    pub fn get_budget_for(&self, host: &CaseInsensitiveString) -> &BudgetSettings {
        match self.per_host {
            None => {
                &self.default
            }
            Some(ref found) => {
                found.get(host).unwrap_or(&self.default)
            }
        }
    }
}


/// The budget for the crawled website
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum BudgetSettings {
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
    }
}

impl BudgetSettings {
    // pub fn is_finite(&self) -> bool {
    //     match self {
    //         BudgetSettings::SeedOnly { .. } => {true}
    //         BudgetSettings::Normal { .. } => {true}
    //         BudgetSettings::Absolute { depth, .. } => {0u64.eq(depth)}
    //     }
    // }

    pub fn get_request_timeout(&self) -> Option<Duration> {
        match self {
            BudgetSettings::SeedOnly { request_timeout, .. } => {
                request_timeout
            }
            BudgetSettings::Normal { request_timeout, .. } => {
                request_timeout
            }
            BudgetSettings::Absolute { request_timeout, .. } => {
                request_timeout
            }
        }.clone()
    }

    pub fn get_recrawl_interval(&self) -> Option<Duration> {
        match self {
            BudgetSettings::SeedOnly { recrawl_interval, .. } => {
                recrawl_interval
            }
            BudgetSettings::Normal { recrawl_interval, .. } => {
                recrawl_interval
            }
            BudgetSettings::Absolute { recrawl_interval, .. } => {
                recrawl_interval
            }
        }.clone()
    }

    /// Returns true, iff the [url] is in the budget
    pub fn is_in_budget(
        &self,
        url: &UrlWithDepth
    ) -> bool {
        match self {
            BudgetSettings::SeedOnly { depth_on_website: depth, .. } => {
                url.depth.distance_to_seed == 0 && (0.eq(depth)  || url.depth.depth_on_website.le(depth))
            }
            BudgetSettings::Normal { depth_on_website: depth, depth: depth_distance, .. } => {
                (0.eq(depth)  || url.depth.depth_on_website.le(depth))
                    && url.depth.distance_to_seed.le(depth_distance)
            }
            BudgetSettings::Absolute { depth, .. } => {
                0.eq(depth) || url.depth.total_distance_to_seed.le(depth)
            }
        }
    }
}


impl Default for BudgetSettings {
    fn default() -> Self {
        Self::Normal {
            depth_on_website: 20,
            depth: 3,
            recrawl_interval: None,
            request_timeout: Some(Duration::seconds(15))
        }
    }
}

/// Chrome specific settings
#[cfg(feature = "chrome")]
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ChromeSettings {
    /// Use stealth mode for requests.
    pub stealth_mode: bool,
    /// Setup network interception for request.
    pub intercept_settings: InterceptSettings,
    /// Overrides default host system timezone with the specified one.
    pub timezone_id: Option<String>,
    /// Overrides default host system locale with the specified one.
    pub locale: Option<String>,
    /// Configure the viewport for chrome.
    pub viewport: Option<Viewport>,
}

/// The intercept settings for chrome
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub enum InterceptSettings {
    /// No intercepting
    #[default]
    Off,
    On {
        /// Setup network interception for request.
        chrome_intercept: bool,

        /// Block all images from rendering in Chrome.
        chrome_intercept_block_visuals: bool,
    }
}


#[cfg(feature = "chrome")]
#[derive(Serialize, Deserialize, Debug, Clone)]
/// View port handling for chrome.
pub struct Viewport {
    /// Device screen Width
    pub width: u32,
    /// Device screen size
    pub height: u32,
    /// Device scale factor
    pub device_scale_factor: Option<f64>,
    /// Emulating Mobile?
    pub emulating_mobile: bool,
    /// Use landscape mode instead of portrait.
    pub is_landscape: bool,
    /// Touch screen device?
    pub has_touch: bool,
}

#[cfg(feature = "chrome")]
impl Default for Viewport {
    fn default() -> Self {
        Viewport {
            width: 800,
            height: 600,
            device_scale_factor: None,
            emulating_mobile: false,
            is_landscape: false,
            has_touch: false,
        }
    }
}

#[cfg(feature = "chrome")]
impl From<Viewport> for chromiumoxide::handler::viewport::Viewport {
    fn from(viewport: spider::configuration::Viewport) -> Self {
        Self {
            width: viewport.width,
            height: viewport.height,
            device_scale_factor: viewport.device_scale_factor,
            emulating_mobile: viewport.emulating_mobile,
            is_landscape: viewport.is_landscape,
            has_touch: viewport.has_touch,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, EnumString, Display, Serialize, Deserialize)]
/// Capture screenshot options for chrome.
pub enum CaptureScreenshotFormat {
    #[serde(rename = "jpeg")]
    /// jpeg format
    Jpeg,
    #[serde(rename = "png")]
    #[default]
    /// png format
    Png,
    #[serde(rename = "webp")]
    /// webp format
    Webp,
}
#[cfg(feature = "chrome")]
impl From<CaptureScreenshotFormat>
for chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat
{
    fn from(value: CaptureScreenshotFormat) -> Self {
        match value {
            CaptureScreenshotFormat::Jpeg => {
                chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat::Jpeg
            }
            CaptureScreenshotFormat::Png => {
                chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat::Png
            }
            CaptureScreenshotFormat::Webp => {
                chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat::Webp
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
/// The view port clip for screenshots.
pub struct ClipViewport {
    #[doc = "X offset in device independent pixels (dip)."]
    #[serde(rename = "x")]
    pub x: f64,
    #[doc = "Y offset in device independent pixels (dip)."]
    #[serde(rename = "y")]
    pub y: f64,
    #[doc = "Rectangle width in device independent pixels (dip)."]
    #[serde(rename = "width")]
    pub width: f64,
    #[doc = "Rectangle height in device independent pixels (dip)."]
    #[serde(rename = "height")]
    pub height: f64,
    #[doc = "Page scale factor."]
    #[serde(rename = "scale")]
    pub scale: f64,
}

#[cfg(feature = "chrome")]
impl From<ClipViewport> for chromiumoxide::cdp::browser_protocol::page::Viewport {
    fn from(viewport: ClipViewport) -> Self {
        Self {
            x: viewport.x,
            y: viewport.y,
            height: viewport.height,
            width: viewport.width,
            scale: viewport.scale,
        }
    }
}
