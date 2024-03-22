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

use std::collections::HashSet;
use encoding_rs::Encoding;
use reqwest::header::HeaderMap;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use crate::core::extraction::ExtractedLink;
use crate::core::{VecDataHolder};
use crate::core::response::{ResponseData};
use crate::core::page_type::PageType;
use crate::core::UrlWithDepth;
use crate::core::header_map_extensions::optional_header_map;
use crate::core::serde_util::status_code;

/// The result page of a finished crawl, optimized for memory and serialisation etc.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CrawlResult {
    /// Timestamp
    pub created_at: OffsetDateTime,
    /// The url of the page
    pub url: UrlWithDepth,
    /// The status code of the page request.
    #[serde(with="status_code")]
    pub status_code: StatusCode,
    /// The identified type of the page
    pub page_type: PageType,
    /// The encoding recognized for the data
    pub recognized_encoding: Option<&'static Encoding>,
    /// The headers of the page request response.
    #[serde(with = "optional_header_map")]
    pub headers: Option<HeaderMap>,
    /// The bytes of the resource.
    pub content: VecDataHolder,
    /// The final destination of the page if redirects were performed [Not implemented in the chrome feature].
    pub final_redirect_destination: Option<String>,
    /// The outgoing links found, they are guaranteed to be unique.
    pub links: Option<Vec<ExtractedLink>>,
}

impl CrawlResult {
    pub fn new(
        created_at: OffsetDateTime,
        page: ResponseData,
        links: Option<HashSet<ExtractedLink>>,
        recognized_encoding: Option<&'static Encoding>,
        page_type: PageType,
    ) -> Self {
        let links = links.map(|value| {
            let mut result = Vec::from_iter(value);
            result.shrink_to_fit();
            result
        });
        Self {
            created_at,
            content: page.content,
            url: page.url,
            headers: page.headers,
            status_code: page.status_code,
            final_redirect_destination: page.final_redirect_destination,
            links,
            page_type,
            recognized_encoding,
        }
    }

    #[allow(dead_code)]
    pub fn into_inner(self) -> (ResponseData, Option<HashSet<ExtractedLink>>, Option<&'static Encoding>, PageType) {
        let page = ResponseData::reconstruct(
            self.content,
            self.url,
            self.headers,
            self.status_code,
            self.final_redirect_destination,
        );
        let links = self.links.map(|it| HashSet::from_iter(it));
        (page, links, self.recognized_encoding, self.page_type)
    }
}


#[cfg(test)]
pub(crate) mod test {
    use std::collections::HashSet;
    use reqwest::header::HeaderMap;
    use reqwest::StatusCode;
    use time::OffsetDateTime;
    use crate::core::crawl::result::CrawlResult;
    use crate::core::response::ResponseData;
    use crate::core::{UrlWithDepth, VecDataHolder};
    use crate::core::extraction::ExtractedLink;
    use crate::core::extraction::extractor::SubExtractor;
    use crate::core::extraction::marker::{ExtractorMeta, SubExtractorMeta};
    use crate::core::page_type::PageType;

    pub fn create_testdata_with_on_seed(content: Option<VecDataHolder>) -> CrawlResult {
        create_test_data(
            UrlWithDepth::from_seed("https://www.google.de/").unwrap(),
            content
        )
    }

    pub fn create_test_data(seed: UrlWithDepth, content: Option<VecDataHolder>) -> CrawlResult {
        let mut header = HeaderMap::new();
        header.append(
            reqwest::header::CONTENT_TYPE, reqwest::header::HeaderValue::from_str("text/html").unwrap()
        );
        header.append(
            reqwest::header::CONTENT_ENCODING, reqwest::header::HeaderValue::from_str("utf-8").unwrap()
        );

        let mut links = HashSet::new();
        links.insert(ExtractedLink::OnSeed {
            url: UrlWithDepth::with_base(&seed, "https://www.google.de/1").unwrap(),
            extraction_method: ExtractorMeta{extractor: SubExtractor::HtmlV1, meta: SubExtractorMeta::None}
        });
        links.insert(ExtractedLink::OnSeed {
            url: UrlWithDepth::with_base(&seed, "https://www.google.de/2").unwrap(),
            extraction_method: ExtractorMeta{extractor: SubExtractor::HtmlV1, meta: SubExtractorMeta::None}
        });
        links.insert(ExtractedLink::OnSeed {
            url: UrlWithDepth::with_base(&seed, "https://www.ebay.de/2").unwrap(),
            extraction_method: ExtractorMeta{extractor: SubExtractor::HtmlV1, meta: SubExtractorMeta::None}
        });


        CrawlResult::new(
            OffsetDateTime::now_utc(),
            ResponseData::reconstruct(
                content.unwrap_or_else(|| VecDataHolder::from_vec(b"<html><body>hello world, this is a test file \r\n WARC/1.1\r\n or whetever!</body></html>".to_vec())),
                seed,
                Some(header),
                StatusCode::OK,
                None
            ),
            Some(links),
            Some(encoding_rs::UTF_8),
            PageType::HTML
        )
    }
}