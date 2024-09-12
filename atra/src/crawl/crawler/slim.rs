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

use crate::crawl::crawler::result::{CrawlResult, CrawlResultMeta};
use crate::data::RawData;
use crate::warc_ext::WarcSkipInstruction;
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

/// The header information of a [CrawlResult]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct SlimCrawlResult {
    /// The meta for any kind of entry.
    pub meta: CrawlResultMeta,
    /// The information where the data is stored.
    pub stored_data_hint: StoredDataHint,
}

/// A hint where the data is stored
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum StoredDataHint {
    /// Stored externally on the filesystem
    External(Utf8PathBuf),
    /// Stored in a warc file
    Warc(WarcSkipInstruction),
    /// The data is stored in memory
    InMemory(Vec<u8>),
    /// The data is associated by some external means.
    Associated,
    /// There is no data
    None,
}

impl SlimCrawlResult {
    pub fn new(crawl_result: &CrawlResult, stored_data_hint: StoredDataHint) -> Self {
        Self {
            meta: crawl_result.meta.clone(),
            stored_data_hint,
        }
    }

    /// Inflates the [SlimCrawlResult] to a normal [CrawlResult].
    /// You may provide an associated [body] if necessary
    pub fn inflate(self, body: Option<Vec<u8>>) -> CrawlResult {
        let content = match self.stored_data_hint {
            StoredDataHint::External(value) => RawData::from_external(value),
            StoredDataHint::Warc(_) => {
                // TODO: what about big files???
                RawData::from_vec(body.expect("A warc file has to be loaded beforehand."))
            }
            StoredDataHint::InMemory(value) => RawData::from_vec(value),
            StoredDataHint::Associated | StoredDataHint::None => {
                if let Some(body) = body {
                    RawData::from_vec(body)
                } else {
                    RawData::None
                }
            }
        };

        CrawlResult {
            meta: self.meta,
            content,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::crawl::crawler::result::test::create_test_data;
    use crate::crawl::crawler::slim::{SlimCrawlResult, StoredDataHint};
    use crate::url::UrlWithDepth;
    use crate::warc_ext::{WarcSkipInstruction, WarcSkipPointer, WarcSkipPointerWithPath};
    use camino::Utf8PathBuf;

    #[test]
    fn serde_test() {
        let ptr = StoredDataHint::Warc(WarcSkipInstruction::new_single(
            WarcSkipPointerWithPath::new(
                Utf8PathBuf::from("test.warc".to_string()),
                WarcSkipPointer::new(12589, 1, 2),
            ),
            123,
            false,
        ));

        let x = bincode::serialize(&ptr).unwrap();
        let y = bincode::deserialize::<StoredDataHint>(&x).unwrap();
        assert_eq!(ptr, y);

        let x = create_test_data(
            UrlWithDepth::from_seed("https://www.google.de").unwrap(),
            None,
        );
        let slim = SlimCrawlResult::new(&x, ptr);
        let data = bincode::serialize(&slim).unwrap();
        println!("{:?}", data);
        let slim2 = bincode::deserialize::<SlimCrawlResult>(&data).unwrap();
        assert_eq!(slim2, slim)
    }
}