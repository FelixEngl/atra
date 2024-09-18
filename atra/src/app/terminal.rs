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

use crate::config::BudgetSetting;
use crate::seed::UnguardedSeed;
use crate::url::{AtraUrlOrigin, UrlWithDepth};
use console::Term;
use std::collections::VecDeque;
use std::sync::RwLock;
use time::Duration;
use tokio::sync::watch::{Receiver, Sender};

pub struct Terminal {
    terminal: Term,
    watched: RwLock<Vec<(Sender<CrawlTaskWatchState>, Receiver<CrawlTaskWatchState>)>>,
}

pub struct CrawlTaskWatchState {
    used: bool,
    worker_id: Option<usize>,
    meta_information: Option<MetaInformation>,
    current: Option<CrawlTarget>,
    buffer: VecDeque<CrawlTarget>,
}

pub struct CrawlTarget {
    target: UrlWithDepth,
}

pub struct MetaInformation {
    client: &'static str,
    seed: UnguardedSeed,
    origin: AtraUrlOrigin,
    has_robots: bool,
    interval: Duration,
    budget_setting: BudgetSetting,
}

impl CrawlTaskWatchState {
    fn set_next(&mut self) {}

    pub fn reserve_for_worker(&mut self, worker_id: Option<usize>) {
        self.reset();
        self.used = true;
        self.worker_id = worker_id;
    }

    pub fn set_meta_information(&mut self, meta_information: MetaInformation) {
        self.meta_information = Some(meta_information);
    }

    pub fn reset(&mut self) {
        self.used = false;
        self.worker_id = None;
        self.meta_information = None;
        self.current = None;
        self.buffer.clear();
    }
}
