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

use serde::{Deserialize, Serialize};

/// The config of the session
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename(serialize = "Session"))]
pub struct SessionConfig {
    /// The name of the service
    #[serde(default = "_default_service_name")]
    pub service: String,
    /// The name of the collection created
    #[serde(default = "_default_collection_name")]
    pub collection: String,
    /// The crawl job id
    #[serde(default)]
    pub crawl_job_id: u64,
    /// Apply some kind of compression to the warc archive?
    #[serde(default)]
    pub warc_compression_level: Option<u32>,
}


fn _default_service_name() -> String {
    "atra".to_string()
}

fn _default_collection_name() -> String {
    "unnamed".to_string()
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            service: "atra".to_string(),
            collection: "unnamed".to_string(),
            crawl_job_id: 0,
            warc_compression_level: None
        }
    }
}

