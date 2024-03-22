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

use ini::Ini;
use serde::{Deserialize, Serialize};
use crate::core::ini_ext::{FromIni, IniExt, IntoIni, SectionSetterExt};

/// The config of the session
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionConfig {
    /// The name of the service
    pub service_name: String,
    /// The name of the collection created
    pub collection_name: String,
    /// The crawl job id
    pub crawl_job_id: u64,
    /// Apply some kind of compression to the warc archive?
    pub warc_compression_level: Option<u32>,
}

impl FromIni for SessionConfig {
    fn from_ini(ini: &Ini) -> Self {
        Self {
            service_name: ini.get_or(Some("Service"), "name", "atra".to_string()),
            collection_name: ini.get_or(Some("Service"), "collection", "unnamed".to_string()),
            crawl_job_id: ini.get_or::<u64>(Some("Service"), "job_id", 0),
            warc_compression_level: ini.get::<u32>(Some("Service"), "warc_compression_level")
        }
    }
}

impl IntoIni for SessionConfig {
    fn insert_into(&self, ini: &mut Ini) {
        ini.with_section(Some("Service"))
            .set("name", &self.service_name)
            .set("collection", &self.collection_name)
            .set_mapping("job_id", self.crawl_job_id, |value| value.to_string())
            .set_optional_mapping("warc_compression_level", self.warc_compression_level, |value| value.to_string());
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            service_name: "atra".to_string(),
            collection_name: "unnamed".to_string(),
            crawl_job_id: 0,
            warc_compression_level: None
        }
    }
}