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

use std::num::NonZeroUsize;
use serde::{Deserialize, Serialize};
use crate::web_graph::DEFAULT_CACHE_SIZE_WEB_GRAPH;
use ubyte::ByteUnit;

/// The default cache size for the robots cache
pub const DEFAULT_CACHE_SIZE_ROBOTS: NonZeroUsize = unsafe{NonZeroUsize::new_unchecked(32)};

/// The default size of a fetched side that can be stored in memory (in byte)
pub const DEFAULT_MAX_SIZE_IN_MEMORY_DOWNLOAD: u64 =
    ByteUnit::Megabyte(100).as_u64();


/// Config of the system, basically caches etc.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename(serialize = "System"))]
pub struct SystemConfig {

    /// The cache size of the robots manager
    #[serde(default = "_default_cache_size_robots")]
    pub robots_cache_size: NonZeroUsize,

    /// The cache size of the webgraph manager
    #[serde(default = "_default_cache_size_web_graph")]
    pub web_graph_cache_size: NonZeroUsize,

    /// Max size of the download in memory. (default: 100MB)
    /// If set to 0 nothing will be stored in memory.
    #[serde(default = "_default_max_in_memory")]
    pub max_file_size_in_memory: u64,

    /// The log level of the crawler
    #[serde(default = "_default_log_level")]
    pub log_level: log::LevelFilter,

    /// Log to a file?
    #[serde(default)]
    pub log_to_file: bool
}


const fn _default_log_level() -> log::LevelFilter { log::LevelFilter::Info }
const fn _default_cache_size_robots() -> NonZeroUsize {
    DEFAULT_CACHE_SIZE_ROBOTS
}
const fn _default_cache_size_web_graph() -> NonZeroUsize { DEFAULT_CACHE_SIZE_WEB_GRAPH }
const fn _default_max_in_memory() -> u64 {
    DEFAULT_MAX_SIZE_IN_MEMORY_DOWNLOAD
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            robots_cache_size: _default_cache_size_robots(),
            max_file_size_in_memory: _default_max_in_memory(),
            web_graph_cache_size: _default_cache_size_web_graph(),
            log_level: _default_log_level(),
            log_to_file: false
        }
    }
}