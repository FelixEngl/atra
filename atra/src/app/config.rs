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

use crate::config::Config as AtraConfig;
use camino::Utf8Path;
use config::Config;

/// Try to load the config from the [`path`]
pub fn try_load_from_path<P: AsRef<Utf8Path>>(path: P) -> Result<AtraConfig, config::ConfigError> {
    Config::builder()
        .add_source(config::File::with_name("./config").required(false))
        .add_source(config::File::with_name("./atra").required(false))
        .add_source(config::File::with_name(path.as_ref().join("atra").as_str()).required(false))
        .add_source(config::File::with_name(path.as_ref().join("config").as_str()).required(false))
        .add_source(config::Environment::with_prefix("ATRA").separator("."))
        .build()?
        .try_deserialize()
}

/// Tries to find a config at the default configs
pub fn discover_or_default() -> Result<AtraConfig, config::ConfigError> {
    match Config::builder()
        .add_source(config::File::with_name("./config").required(false))
        .add_source(config::File::with_name("./atra").required(false))
        .add_source(config::File::with_name("atra_data/config").required(false))
        .add_source(config::File::with_name("atra_data/atra").required(false))
        .add_source(config::Environment::with_prefix("ATRA").separator("."))
        .build()
    {
        Ok(value) => value.try_deserialize(),
        Err(_) => Ok(Default::default()),
    }
}

/// Try to discover the config at default paths
pub fn discover() -> Result<AtraConfig, config::ConfigError> {
    Config::builder()
        .add_source(config::File::with_name("./config").required(false))
        .add_source(config::File::with_name("./atra").required(false))
        .add_source(config::File::with_name("atra_data/config").required(false))
        .add_source(config::File::with_name("atra_data/atra").required(false))
        .add_source(config::Environment::with_prefix("ATRA").separator("."))
        .build()?
        .try_deserialize()
}

#[cfg(test)]
mod test {
    use crate::app::config::try_load_from_path;
    use crate::config::Config as AtraConfig;
    use config::Config;
    use std::fs::File;
    use std::io::Read;
    use std::io::{BufReader, BufWriter, Write};

    #[test]
    fn test_loading() {
        let loaded = try_load_from_path("testdata/configs/sub").unwrap();
        println!("{}", loaded.system.robots_cache_size)
    }

    #[test]
    fn can_create_hierarchical_config() {
        let mut config = AtraConfig::default();
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

        let config2: AtraConfig = cfg.try_deserialize().unwrap();

        std::fs::remove_file("./atra_test.json").unwrap();

        let mut config: AtraConfig = serde_json::from_str(&s).unwrap();
        config.system.log_to_file = true;
        assert_eq!(config, config2);
    }
}
