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
use crate::extraction::ExtractedLink;
use crate::fetching::ResponseData;
use crate::format::AtraFileInformation;
use crate::toolkit::header_map_extensions::optional_header_map;
use crate::toolkit::serde_ext::status_code;
use crate::toolkit::LanguageInformation;
use crate::url::UrlWithDepth;
use encoding_rs::Encoding;
use reqwest::header::HeaderMap;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use time::OffsetDateTime;

/// A container for the meta data
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CrawlResultMeta {
    /// Timestamp
    pub created_at: OffsetDateTime,
    /// The url of the page
    pub url: UrlWithDepth,
    /// The status code of the page request.
    #[serde(with = "status_code")]
    pub status_code: StatusCode,
    /// The file format of the data
    pub file_information: AtraFileInformation,
    /// The encoding recognized for the data
    pub recognized_encoding: Option<&'static Encoding>,
    /// The headers of the page request response.
    #[serde(with = "optional_header_map")]
    pub headers: Option<HeaderMap>,
    /// The final destination of the page if redirects were performed [Not implemented in the chrome feature].
    pub final_redirect_destination: Option<String>,
    /// The outgoing links found, they are guaranteed to be unique.
    pub links: Option<Vec<ExtractedLink>>,
    /// The language identified by atra.
    pub language: Option<LanguageInformation>,
}

impl CrawlResultMeta {
    pub fn new(
        created_at: OffsetDateTime,
        url: UrlWithDepth,
        status_code: StatusCode,
        file_information: AtraFileInformation,
        recognized_encoding: Option<&'static Encoding>,
        headers: Option<HeaderMap>,
        final_redirect_destination: Option<String>,
        links: Option<Vec<ExtractedLink>>,
        language: Option<LanguageInformation>,
    ) -> Self {
        Self {
            created_at,
            url,
            status_code,
            file_information,
            recognized_encoding,
            headers,
            final_redirect_destination,
            links,
            language,
        }
    }
}

// page_type = AtraFileFormat::format

/// The result page of a finished crawl, optimized for memory and serialisation etc.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CrawlResult {
    /// The meta for any kind of entry.
    pub meta: CrawlResultMeta,
    /// The bytes of the resource.
    pub content: RawVecData,
}

impl CrawlResult {
    pub fn new(
        created_at: OffsetDateTime,
        page: ResponseData,
        links: Option<HashSet<ExtractedLink>>,
        recognized_encoding: Option<&'static Encoding>,
        file_information: AtraFileInformation,
        language: Option<LanguageInformation>,
    ) -> Self {
        let links = links.map(|value| {
            let mut result = Vec::from_iter(value);
            result.shrink_to_fit();
            result
        });
        Self {
            meta: CrawlResultMeta::new(
                created_at,
                page.url,
                page.status_code,
                file_information,
                recognized_encoding,
                page.headers,
                page.final_redirect_destination,
                links,
                language,
            ),
            content: page.content,
        }
    }
}

#[cfg(test)]
pub mod test {
    use crate::crawl::CrawlResult;
    use crate::data::RawVecData;
    use crate::extraction::extractor_method::ExtractorMethod;
    use crate::extraction::marker::ExtractorMethodHint;
    use crate::extraction::ExtractedLink;
    use crate::fetching::ResponseData;
    use crate::format::supported::InterpretedProcessibleFileFormat;
    use crate::format::AtraFileInformation;
    use crate::toolkit::LanguageInformation;
    use crate::url::UrlWithDepth;
    use reqwest::header::HeaderMap;
    use reqwest::StatusCode;
    use std::collections::HashSet;
    use time::OffsetDateTime;

    pub fn create_testdata_with_on_seed(content: Option<RawVecData>) -> CrawlResult {
        create_test_data(
            UrlWithDepth::from_url("https://www.google.de/").unwrap(),
            content,
        )
    }

    pub fn create_test_data(seed: UrlWithDepth, content: Option<RawVecData>) -> CrawlResult {
        let mut header = HeaderMap::new();
        header.append(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_str("text/html").unwrap(),
        );
        header.append(
            reqwest::header::CONTENT_ENCODING,
            reqwest::header::HeaderValue::from_str("utf-8").unwrap(),
        );

        let mut links = HashSet::new();
        links.insert(ExtractedLink::OnSeed {
            url: UrlWithDepth::with_base(&seed, "https://www.google.de/1").unwrap(),
            extraction_method: ExtractorMethodHint::new_without_meta(ExtractorMethod::HtmlV1),
        });
        links.insert(ExtractedLink::OnSeed {
            url: UrlWithDepth::with_base(&seed, "https://www.google.de/2").unwrap(),
            extraction_method: ExtractorMethodHint::new_without_meta(ExtractorMethod::HtmlV1),
        });
        links.insert(ExtractedLink::OnSeed {
            url: UrlWithDepth::with_base(&seed, "https://www.ebay.de/2").unwrap(),
            extraction_method: ExtractorMethodHint::new_without_meta(ExtractorMethod::HtmlV1),
        });

        CrawlResult::new(
            OffsetDateTime::now_utc(),
            ResponseData::new(
                content.unwrap_or_else(|| RawVecData::from_vec(b"<html><body>hello world, this is a test file \r\n WARC/1.1\r\n or whatever!</body></html>".to_vec())),
                seed,
                Some(header),
                StatusCode::OK,
                None,
            ),
            Some(links),
            Some(encoding_rs::UTF_8),
            AtraFileInformation::new(
                InterpretedProcessibleFileFormat::HTML,
                None,
                None
            ),
            Some(LanguageInformation::DEU)
        )
    }

    pub fn create_test_data_unknown(seed: UrlWithDepth, content: RawVecData) -> CrawlResult {
        let mut links = HashSet::new();
        links.insert(ExtractedLink::OnSeed {
            url: UrlWithDepth::with_base(&seed, "https://www.google.de/1").unwrap(),
            extraction_method: ExtractorMethodHint::new_without_meta(ExtractorMethod::HtmlV1),
        });
        links.insert(ExtractedLink::OnSeed {
            url: UrlWithDepth::with_base(&seed, "https://www.google.de/2").unwrap(),
            extraction_method: ExtractorMethodHint::new_without_meta(ExtractorMethod::HtmlV1),
        });
        links.insert(ExtractedLink::OnSeed {
            url: UrlWithDepth::with_base(&seed, "https://www.ebay.de/2").unwrap(),
            extraction_method: ExtractorMethodHint::new_without_meta(ExtractorMethod::HtmlV1),
        });

        CrawlResult::new(
            OffsetDateTime::now_utc(),
            ResponseData::new(content, seed, None, StatusCode::OK, None),
            Some(links),
            None,
            AtraFileInformation::new(InterpretedProcessibleFileFormat::HTML, None, None),
            Some(LanguageInformation::ENG),
        )
    }
}
