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

use encoding_rs::Encoding;
use reqwest::header::HeaderMap;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use crate::core::crawl::result::CrawlResult;
use crate::core::extraction::ExtractedLink;
use crate::core::io::paths::DataFilePathBuf;
use crate::core::page_type::PageType;
use crate::core::{DataHolder, UrlWithDepth};
use crate::core::serde_util::*;
use crate::core::header_map_extensions::optional_header_map;
use crate::core::warc::WarcSkipInstruction;

/// The header information of a [CrawlResult]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct SlimCrawlResult {
    /// Timestamp
    pub created_at: OffsetDateTime,
    /// The url of the page
    pub url: UrlWithDepth,
    /// The status code of the page request.
    #[serde(with="status_code")]
    pub status_code: StatusCode,
    /// The identified type of the page
    pub page_type: PageType,
    /// The information where the data is stored.
    pub stored_data_hint: StoredDataHint,
    /// The encoding recognized for the data
    pub recognized_encoding: Option<&'static Encoding>,
    /// The headers of the page request response.
    #[serde(with = "optional_header_map")]
    pub headers: Option<HeaderMap>,
    /// The final destination of the page if redirects were performed [Not implemented in the chrome feature].
    pub final_redirect_destination: Option<String>,
    /// The outgoing links found, they are guaranteed to be unique.
    pub links: Option<Vec<ExtractedLink>>,
}

/// A hint where the data is stored
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum StoredDataHint {
    /// Stored externally on the filesystem
    External(DataFilePathBuf),
    /// Stored in a warc file
    Warc(WarcSkipInstruction),
    /// The data is stored in memory
    InMemory(Vec<u8>),
    /// The data is associated by some external means.
    Associated,
    /// There is no data
    None
}

impl SlimCrawlResult {
    pub fn new(crawl_result: &CrawlResult, stored_data_hint: StoredDataHint) -> Self {
        Self {
            headers: crawl_result.headers.clone(),
            url: crawl_result.url.clone(),
            status_code: crawl_result.status_code,
            links: crawl_result.links.clone(),
            page_type: crawl_result.page_type,
            recognized_encoding: crawl_result.recognized_encoding,
            created_at: crawl_result.created_at,
            final_redirect_destination: crawl_result.final_redirect_destination.clone(),
            stored_data_hint
        }
    }

    /// Inflates the [SlimCrawlResult] to a normal [CrawlResult].
    /// You may provide an associated [body] if necessary
    pub fn inflate(self, body: Option<Vec<u8>>) -> CrawlResult {
        let content = match self.stored_data_hint {
            StoredDataHint::External(value) => {
                DataHolder::from_external(value)
            }
            StoredDataHint::Warc(_) => {
                // TODO: what about big files???
                DataHolder::from_vec(body.expect("A warc file has to be loaded beforehand."))
            }
            StoredDataHint::InMemory(value) => {
                DataHolder::from_vec(value)
            }
            StoredDataHint::Associated | StoredDataHint::None => {
                if let Some(body) = body {
                    DataHolder::from_vec(body)
                } else {
                    DataHolder::None
                }
            }
        };

        CrawlResult {
            headers: self.headers,
            url: self.url,
            status_code: self.status_code,
            links: self.links,
            page_type: self.page_type,
            recognized_encoding: self.recognized_encoding,
            created_at: self.created_at,
            final_redirect_destination: self.final_redirect_destination,
            content,
        }
    }
}

#[cfg(test)]
mod test {
    use camino::Utf8PathBuf;
    use crate::core::crawl::result::test::create_test_data;
    use crate::core::crawl::slim::{SlimCrawlResult, StoredDataHint};
    use crate::core::io::paths::DataFilePathBuf;
    use crate::core::UrlWithDepth;
    use crate::core::warc::WarcSkipInstruction;
    use crate::core::warc::writer::{WarcSkipPointer, WarcSkipPointerWithOffsets};

    #[test]
    fn serde_test(){
        let ptr = StoredDataHint::Warc(WarcSkipInstruction::new_single(
            WarcSkipPointerWithOffsets::new(
                WarcSkipPointer::new(DataFilePathBuf::new(Utf8PathBuf::from("test.warc".to_string())), 12589),
                1,
                2
            ),
        123,
            false
        ));

        let x = bincode::serialize(&ptr).unwrap();
        let y = bincode::deserialize::<StoredDataHint>(&x).unwrap();
        assert_eq!(ptr, y);

        let x = create_test_data(UrlWithDepth::from_seed("https://www.google.de").unwrap(), None);
        let slim = SlimCrawlResult::new(&x, ptr);
        let data = bincode::serialize(&slim).unwrap();
        println!("{:?}", data);
        let slim2 = bincode::deserialize::<SlimCrawlResult>(&data).unwrap();
        assert_eq!(slim2, slim)
    }
}