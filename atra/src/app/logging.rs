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

use crate::config::Configs;
use log::LevelFilter;
use log4rs::append::console::ConsoleAppender;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Logger, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::Config;

/// Configure the logging
pub fn configure_logging(configs: &Configs) {
    // todo: improve by adding custom logging
    // see: https://docs.rs/log4rs/latest/log4rs/
    // https://docs.rs/log4rs/latest/log4rs/encode/pattern/index.html

    let config = Config::builder();

    let config = if configs.system.log_to_file {
        println!("Logging to file!");
        let file_logger = FileAppender::builder()
            .encoder(Box::new(PatternEncoder::new(
                "{l}@Thread{I} - {d} - {m}{n}",
            )))
            .build(configs.paths.root_path().join("out.log"))
            .unwrap();
        config.appender(Appender::builder().build("out", Box::new(file_logger)))
    } else {
        let console_logger = ConsoleAppender::builder()
            .encoder(Box::new(PatternEncoder::new(
                "{l}@Thread{I} - {d} - {m}{n}",
            )))
            .build();
        config.appender(Appender::builder().build("out", Box::new(console_logger)))
    };

    let config = config
        .logger(Logger::builder().build("atra", configs.system.log_level))
        .build(Root::builder().appender("out").build(LevelFilter::Warn))
        .unwrap();

    let _ = log4rs::init_config(config).unwrap();
}
