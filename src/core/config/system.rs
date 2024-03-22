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
use ini::Ini;
use serde::{Deserialize, Serialize};
use crate::core::ini_ext::{FromIni, IniExt, IntoIni, SectionSetterExt};
use crate::core::web_graph::DEFAULT_CACHE_SIZE_WEB_GRAPH;
use crate::core::system::{DEFAULT_CACHE_SIZE_ROBOTS, DEFAULT_MAX_SIZE_IN_MEMORY_DOWNLOAD};


/// Config of the system, basically paths etc.
#[derive(Debug, Serialize, Deserialize, Clone)]
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


impl FromIni for SystemConfig {
    fn from_ini(ini: &Ini) -> Self {
        Self {
            robots_cache_size: ini.get::<usize>(Some("Caches"), "robots")
                .map(|value| NonZeroUsize::new(value))
                .flatten()
                .unwrap_or(DEFAULT_CACHE_SIZE_ROBOTS),
            max_file_size_in_memory: ini.get_or::<u64>(Some("Memory"), "max_file_size_in_memory", DEFAULT_MAX_SIZE_IN_MEMORY_DOWNLOAD),
            web_graph_cache_size: ini.get::<usize>(Some("Caches"), "web_graph")
                .map(|value| NonZeroUsize::new(value))
                .flatten()
                .unwrap_or(DEFAULT_CACHE_SIZE_WEB_GRAPH),
            log_level: ini.get_or::<log::LevelFilter>(Some("Logging"), "log_level", log::LevelFilter::Info),
            log_to_file: ini.get_or(Some("Logging"), "log_to_file", false),
        }
    }
}

impl IntoIni for SystemConfig {
    fn insert_into(&self, ini: &mut Ini) {
        ini.with_section(Some("Caches"))
            .set_mapping("robots", self.robots_cache_size, |value| value.get().to_string())
            .set_mapping("web_graph", self.web_graph_cache_size, |value| value.get().to_string());

        ini.with_section(Some("Memory"))
            .set_mapping("max_file_size_in_memory", self.max_file_size_in_memory, |value| value.to_string());

        ini.with_section(Some("Logging"))
            .set_mapping("log_level", self.log_level, |it| it.to_string())
            .set_mapping("log_to_file", self.log_to_file, |it| it.to_string())
        ;
    }
}


impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            robots_cache_size: DEFAULT_CACHE_SIZE_ROBOTS,
            max_file_size_in_memory: DEFAULT_MAX_SIZE_IN_MEMORY_DOWNLOAD,
            web_graph_cache_size: DEFAULT_CACHE_SIZE_WEB_GRAPH,
            log_level: log::LevelFilter::Info,
            log_to_file: false
        }
    }
}

#[cfg(test)]
mod test {
    use crate::core::config::SystemConfig;
    use crate::core::ini_ext::IntoIni;

    #[test]
    fn can_make_init(){
        let config = SystemConfig::default();
        let ini = config.to_ini();
        ini.write_to_file("system-config.ini").unwrap();
    }
}