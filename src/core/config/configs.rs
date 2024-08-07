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

use camino::Utf8Path;
use config::Config;
use serde::{Deserialize, Serialize};
use crate::core::config::crawl::CrawlConfig;
use crate::core::config::paths::PathsConfig;
use crate::core::config::session::SessionConfig;
use crate::core::config::SystemConfig;

/// A collection of all config used in a crawl.
/// Can be shared across threads
#[derive(Debug, Default, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename(serialize = "Config"))]
pub struct Configs {
    pub system: SystemConfig,
    pub paths: PathsConfig,
    pub crawl: CrawlConfig,
    pub session: SessionConfig,
}

impl Configs {
    #[cfg(test)] pub fn new(system: SystemConfig, paths: PathsConfig, crawl: CrawlConfig, session: SessionConfig) -> Self {
        Self {
            system,
            paths,
            crawl,
            session,
        }
    }
    #[inline]
    pub fn paths(&self) -> &PathsConfig {
        &self.paths
    }
    #[inline]
    pub fn system(&self) -> &SystemConfig {
        &self.system
    }
    #[inline]
    pub fn crawl(&self) -> &CrawlConfig { &self.crawl }
    #[inline]
    pub fn session(&self) -> &SessionConfig { &self.session }

    pub fn load_from<P: AsRef<Utf8Path>>(folder: P) -> Result<Self, config::ConfigError> {
        Config::builder()
            .add_source(config::File::with_name("./config"))
            .add_source(config::File::with_name("./atra").required(false))
            .add_source(config::File::with_name(folder.as_ref().join("atra").as_str()))
            .add_source(config::File::with_name(folder.as_ref().join("config").as_str()))
            .add_source(config::Environment::with_prefix("ATRA").separator("."))
            .build()?.try_deserialize()
    }

    pub fn discover_or_default() -> Result<Self, config::ConfigError> {
        Config::builder()
            .add_source(config::File::with_name("./atra"))
            .add_source(config::File::with_name("./config"))
            .add_source(config::Environment::with_prefix("ATRA").separator("."))
            .build()?.try_deserialize()
    }
}

#[cfg(test)]
mod test {
    use std::fs::File;
    use std::io::{BufReader, BufWriter, Write};
    use std::io::Read;
    use config::{Config};
    use crate::core::config::Configs;

    #[test]
    fn can_create_hierarchical_config(){
        let mut config = Configs::default();
        config.session.crawl_job_id = 99;
        let mut writer = BufWriter::new(File::options().write(true).create(true).open("./atra_test.json").unwrap());
        write!(&mut writer, "{}", serde_json::to_string(&config).unwrap());
        drop(writer);

        let mut s = String::new();
        BufReader::new(File::open("./atra_test.json").unwrap()).read_to_string(&mut s).unwrap();
        std::env::set_var("ATRA.SYSTEM.LOG_TO_FILE", "true");

        let cfg = Config::builder()
            .add_source(config::File::with_name("./atra_test"))
            .add_source(config::Environment::with_prefix("ATRA").separator("."))
            .build()
            .unwrap();

        println!("{cfg}");

        let config2: Configs = cfg.try_deserialize().unwrap();

        std::fs::remove_file("./atra_test.json").unwrap();

        let mut config: Configs = serde_json::from_str(&s).unwrap();
        config.system.log_to_file = true;
        assert_eq!(config, config2);

    }
}
