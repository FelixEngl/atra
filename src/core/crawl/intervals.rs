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

use std::collections::HashMap;
use std::sync::Arc;
use time::Duration;
use tokio::time::Interval;
use crate::client::Client;
use crate::core::config::CrawlConfig;
use crate::core::robots::information::RobotsInformation;
use crate::core::origin::{AtraOriginProvider, AtraUrlOrigin};
use crate::core::UrlWithDepth;

/// Manages the interval
pub struct InvervalManager<'a, R: RobotsInformation> {
    client: &'a Client,
    configured_robots: Arc<R>,
    registered_intervals: HashMap<AtraUrlOrigin, Interval>,
    default_delay: Option<Duration>,
    no_domain_default: Interval
}

impl<'a, R: RobotsInformation> InvervalManager<'a, R> {
    pub fn new(
        client: &'a Client,
        config: &CrawlConfig,
        configured_robots: Arc<R>,
    ) -> Self {
        Self {
            client,
            configured_robots,
            registered_intervals: HashMap::new(),
            default_delay: config.delay.clone(),
            no_domain_default: if let Some(ref default) = config.delay {
                tokio::time::interval(default.clone().unsigned_abs())
            } else {
                tokio::time::interval(std::time::Duration::from_millis(1000))
            }
        }
    }

    pub async fn wait(&mut self, url: &UrlWithDepth) {
        if let Some(origin) = url.atra_origin() {
            if let Some(interval) = self.registered_intervals.get_mut(&origin) {
                log::trace!("Wait {origin} for {}ms!", interval.period().as_millis());
                interval.tick().await;
                log::trace!("Finished waiting {origin} for {}!", interval.period().as_millis());
            } else {
                let target_duration = if let Some(found) = self.configured_robots.get_or_retrieve_delay(&self.client, url).await {
                    log::trace!("Wait found {found}");
                    found.unsigned_abs()
                } else if let Some(default) = self.default_delay {
                    log::trace!("Wait default {default}");
                    default.unsigned_abs()
                } else {
                    log::warn!("Fallback to 100ms");
                    std::time::Duration::from_millis(100)
                };
                self.registered_intervals.insert(origin.clone(), tokio::time::interval(target_duration));
                self.registered_intervals.get_mut(&origin).unwrap().tick().await;
            }
        } else {
            log::trace!("No host tick.");
            self.no_domain_default.tick().await;
        }
    }
}