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

use crate::crawl::crawler::intervals::InvervalManager;
use crate::robots::information::RobotsInformation;
use crate::url::UrlWithDepth;
use case_insensitive_string::CaseInsensitiveString;
use sitemap::reader::SiteMapEntity;
use sitemap::structs::{SiteMapEntry, UrlEntry};
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Cursor;
use crate::client::traits::{AtraClient, AtraResponse};

/// Holds the parsed side maps
#[derive(Debug)]
pub struct ParsedSiteMapEntries {
    pub urls: Vec<UrlEntry>,
    #[allow(dead_code)]
    pub sitemaps: Vec<SiteMapEntry>,
}

/// Retrieves and parses sitemaps form [url]
/// todo: use
pub async fn retrieve_and_parse<'a, Client: AtraClient, R: RobotsInformation>(
    client: &Client,
    url: &UrlWithDepth,
    configured_robots: &R,
    interval: &mut InvervalManager<'a, impl AtraClient, impl RobotsInformation>,
    external_sitemaps: Option<&HashMap<CaseInsensitiveString, Vec<String>>>,
) -> ParsedSiteMapEntries {
    let mut sitemap_urls: Vec<Cow<str>> = Vec::new();
    if let Ok(robot) = configured_robots.get_or_retrieve(client, url).await {
        if let Some(sitemaps) = robot.sitemaps() {
            for value in sitemaps.iter() {
                sitemap_urls.push(Cow::Owned(value.to_string()))
            }
        }
    }

    if let Some(external_sitemap_urls) = external_sitemaps {
        if let Some(ref domain) = url.domain() {
            if let Some(sitemaps) = external_sitemap_urls.get(domain) {
                for sitemap_url in sitemaps {
                    sitemap_urls.push(Cow::Borrowed(sitemap_url.as_str()))
                }
            }
        }
    }

    let mut urls: Vec<UrlEntry> = Vec::new();
    let mut sitemaps: Vec<SiteMapEntry> = Vec::new();

    for sitemap_url in sitemap_urls {
        interval.wait(url).await;
        if let Ok(result) = client.get(sitemap_url.as_ref()).await {
            if let Ok(text) = result.text().await {
                let parser = sitemap::reader::SiteMapReader::new(Cursor::new(text));
                for entity in parser {
                    match entity {
                        SiteMapEntity::Url(url_entry) => {
                            urls.push(url_entry);
                        }
                        SiteMapEntity::SiteMap(sitemap_entry) => {
                            sitemaps.push(sitemap_entry);
                        }
                        SiteMapEntity::Err(error) => {
                            log::info!("Was not able to process sitemap entry {}", error)
                        }
                    }
                }
            }
        }
    }

    return ParsedSiteMapEntries { urls, sitemaps };
}
