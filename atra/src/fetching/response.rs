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

use crate::data::RawVecData;
use crate::fetching::FetchedRequestData;
use crate::url::AtraUri;
use crate::url::UrlWithDepth;
use reqwest::header::HeaderMap;
use reqwest::StatusCode;
use std::str::FromStr;

/// The response for a request
#[derive(Debug)]
pub struct ResponseData {
    /// The bytes of the resource.
    pub content: RawVecData,
    /// The url of the page
    pub url: UrlWithDepth,
    /// The headers of the page request response.
    pub headers: Option<HeaderMap>,
    /// The status code of the page request.
    pub status_code: StatusCode,
    /// The final destination of the page if redirects were performed [Not implemented in the chrome feature].
    pub final_redirect_destination: Option<String>,
}

impl ResponseData {
    #[cfg(test)]
    pub fn reconstruct(
        content: RawVecData,
        url: UrlWithDepth,
        headers: Option<HeaderMap>,
        status_code: StatusCode,
        final_redirect_destination: Option<String>,
    ) -> Self {
        Self {
            content,
            url,
            headers,
            status_code,
            final_redirect_destination,
        }
    }

    pub fn new(page_response: FetchedRequestData, url: UrlWithDepth) -> Self {
        Self {
            content: page_response.content,
            url,
            headers: page_response.headers,
            status_code: page_response.status_code,
            final_redirect_destination: page_response.final_url,
        }
    }

    /// Returns a reference to the dataholder
    pub fn content(&self) -> &RawVecData {
        &self.content
    }

    /// Returns the parsed url
    pub fn get_url_parsed(&self) -> &AtraUri {
        return &self.url.url();
    }

    /// Returns the url used after resolving all redirects
    pub fn get_url_final(&self) -> AtraUri {
        if let Some(ref found) = self.final_redirect_destination {
            AtraUri::from_str(found.as_str()).unwrap_or_else(|_| self.url.url.clone())
        } else {
            self.url.url().clone()
        }
    }
}
