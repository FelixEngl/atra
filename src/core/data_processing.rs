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

use camino::Utf8PathBuf;
use encoding_rs::{UTF_8};
use tokio::task::yield_now;
use crate::core::contexts::Context;
use crate::core::decoding::{decode, DecodedData, DecodingError, do_decode};
use crate::core::mime::IS_UTF8;
use crate::core::response::{ResponseData, ResponseDataWithMeta};
use crate::core::page_type::PageType;
use crate::core::VecDataHolder;


/// Process the page to extract the mime type and the decoded data.
pub async fn process<'a>(context: &impl Context, page: &'a ResponseData) -> Result<ProcessedData<'a>, DecodingError> {
    let page = ResponseDataWithMeta::create_from(page, context);
    yield_now().await;

    match &page.data.content {
        VecDataHolder::None => {
            return Ok(
                ProcessedData(
                    page,
                    DecodedData::None
                )
            )
        }
        _ => {}
    };

    let decoded = match &page.page_type {
        PageType::HTML | PageType::JavaScript | PageType::PlainText | PageType::Decodeable =>
            decode(context, &page).await?.map_in_memory(|value| value.to_string()),
        _ if page.check_has_document_type(IS_UTF8) =>
            do_decode(&page, UTF_8).await?.map_in_memory(|value| value.to_string()),
        _ =>
            DecodedData::None
    };
    Ok(
        ProcessedData(
            page,
            decoded
        )
    )
}


/// A tuple containing the preprocessed page data
pub struct ProcessedData<'a>(
    pub ResponseDataWithMeta<'a>,
    pub DecodedData<String, Utf8PathBuf>,
);
