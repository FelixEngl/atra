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

use reqwest::StatusCode;
use texting_robots::Robot;
use time::ext::NumericalDuration;
use time::{Duration, OffsetDateTime};

/// The cache entry for a robots.txt
#[derive(Debug)]
pub enum CachedRobots {
    HasRobots {
        robot: Robot,
        retrieved_at: OffsetDateTime,
    },
    NoRobots {
        #[allow(dead_code)]
        status_code: StatusCode,
        retrieved_at: OffsetDateTime,
    },
}

impl CachedRobots {
    #[allow(dead_code)]
    pub fn map<R, F>(&self, on_has_robot: F) -> Option<R>
    where
        F: FnOnce(&Robot) -> R,
    {
        match self {
            CachedRobots::HasRobots { robot, .. } => Some(on_has_robot(robot)),
            CachedRobots::NoRobots { .. } => None,
        }
    }

    #[allow(dead_code)]
    pub fn map_or<R, F>(&self, default: R, on_has_robot: F) -> R
    where
        F: FnOnce(&Robot) -> R,
    {
        match self {
            CachedRobots::HasRobots { robot, .. } => on_has_robot(robot),
            CachedRobots::NoRobots { .. } => default,
        }
    }

    #[allow(dead_code)]
    pub fn map_or_else<R, D, F>(&self, default: D, on_has_robot: F) -> R
    where
        D: FnOnce() -> R,
        F: FnOnce(&Robot) -> R,
    {
        match self {
            CachedRobots::HasRobots { robot, .. } => on_has_robot(robot),
            CachedRobots::NoRobots { .. } => default(),
        }
    }

    /// Checks if the url is allowed
    pub fn allowed(&self, url: &str) -> bool {
        self.map_or(true, |it| it.allowed(url))
    }

    /// Returns the sitemaps, if there are any.
    pub fn sitemaps(&self) -> Option<&Vec<String>> {
        match self {
            CachedRobots::HasRobots { robot, .. } => Some(&robot.sitemaps),
            CachedRobots::NoRobots { .. } => None,
        }
    }

    /// Returns the delay, if there is one configured
    pub fn delay(&self) -> Option<Duration> {
        self.map_or(None, |it| {
            it.delay.map(|seconds| (seconds as f64).seconds())
        })
    }

    /// Returns the timestamp when it was retrieved.
    pub fn retrieved_at(&self) -> OffsetDateTime {
        match self {
            CachedRobots::HasRobots { retrieved_at, .. } => retrieved_at,
            CachedRobots::NoRobots { retrieved_at, .. } => retrieved_at,
        }
        .clone()
    }
}
