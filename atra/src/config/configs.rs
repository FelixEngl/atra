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

use crate::config::crawl::CrawlConfig;
use crate::config::paths::PathsConfig;
use crate::config::session::SessionConfig;
use crate::config::SystemConfig;
use camino::Utf8Path;
use config::Config;
use serde::{Deserialize, Serialize};

/// A collection of all config used in a crawl.
/// Can be shared across threads
#[derive(Debug, Default, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename(serialize = "Config"))]
pub struct Configs {
    pub system: SystemConfig,
    pub paths: PathsConfig,
    pub session: SessionConfig,
    pub crawl: CrawlConfig,
}

impl Configs {
    #[cfg(test)]
    pub fn new(
        system: SystemConfig,
        paths: PathsConfig,
        crawl: CrawlConfig,
        session: SessionConfig,
    ) -> Self {
        Self {
            system,
            paths,
            crawl,
            session,
        }
    }

    pub fn load_from<P: AsRef<Utf8Path>>(folder: P) -> Result<Self, config::ConfigError> {
        Config::builder()
            .add_source(config::File::with_name("./config"))
            .add_source(config::File::with_name("./atra").required(false))
            .add_source(config::File::with_name(
                folder.as_ref().join("atra").as_str(),
            ))
            .add_source(config::File::with_name(
                folder.as_ref().join("config").as_str(),
            ))
            .add_source(config::Environment::with_prefix("ATRA").separator("."))
            .build()?
            .try_deserialize()
    }

    pub fn discover_or_default() -> Result<Self, config::ConfigError> {
        match Config::builder()
            .add_source(config::File::with_name("./atra"))
            .add_source(config::File::with_name("./atra_data/atra"))
            .add_source(config::File::with_name("./config"))
            .add_source(config::File::with_name("./atra_data/config"))
            .add_source(config::Environment::with_prefix("ATRA").separator("."))
            .build()
        {
            Ok(value) => value.try_deserialize(),
            Err(_) => Ok(Default::default()),
        }
    }

    pub fn discover() -> Result<Self, config::ConfigError> {
        Config::builder()
            .add_source(config::File::with_name("atra_data/config"))
            .add_source(config::Environment::with_prefix("ATRA").separator("."))
            .build()?
            .try_deserialize()
    }
}

#[cfg(test)]
mod test {
    use crate::config::Configs;
    use config::Config;
    use std::fs::File;
    use std::io::Read;
    use std::io::{BufReader, BufWriter, Write};

    #[test]
    fn can_create_hierarchical_config() {
        let mut config = Configs::default();
        config.session.crawl_job_id = 99;
        let mut writer = BufWriter::new(
            File::options()
                .write(true)
                .create(true)
                .open("./atra_test.json")
                .unwrap(),
        );
        write!(&mut writer, "{}", serde_json::to_string(&config).unwrap()).unwrap();
        drop(writer);

        let mut s = String::new();
        BufReader::new(File::open("./atra_test.json").unwrap())
            .read_to_string(&mut s)
            .unwrap();
        unsafe {
            std::env::set_var("ATRA.SYSTEM.LOG_TO_FILE", "true");
        }

        let cfg = Config::builder()
            .add_source(config::File::with_name("./atra_test"))
            .add_source(config::Environment::with_prefix("ATRA").separator("."))
            .build()
            .unwrap();

        let config2: Configs = cfg.try_deserialize().unwrap();

        std::fs::remove_file("./atra_test.json").unwrap();

        let mut config: Configs = serde_json::from_str(&s).unwrap();
        config.system.log_to_file = true;
        assert_eq!(config, config2);
    }
}
