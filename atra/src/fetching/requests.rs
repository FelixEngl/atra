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

use crate::data::RawVecData;
use reqwest::header::HeaderMap;
use reqwest::StatusCode;
use std::net::SocketAddr;

/// The response of a fetch.
#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct FetchedRequestData {
    /// A dataholder with the body of a fetched request.
    pub content: RawVecData,
    /// The headers of the response. (Always None if a webdriver protocol is used for fetching.).
    pub headers: Option<HeaderMap>,
    /// The status code of the request.
    pub status_code: StatusCode,
    /// The final url destination after any redirects.
    pub final_url: Option<String>,
    /// The remote address
    pub address: Option<SocketAddr>,
    /// Set if there was an error
    pub defect: bool,
}

impl FetchedRequestData {
    #[cfg(test)]
    pub fn new(
        content: RawVecData,
        headers: Option<HeaderMap>,
        status_code: StatusCode,
        final_url: Option<String>,
        address: Option<SocketAddr>,
        defect: bool,
    ) -> Self {
        Self {
            content,
            headers,
            status_code,
            final_url,
            address,
            defect,
        }
    }
}
