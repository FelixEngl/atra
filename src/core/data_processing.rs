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
use crate::core::contexts::Context;
use crate::core::decoding::{decode, DecodedData, DecodingError, do_decode};
use crate::core::format::AtraFileInformation;
use crate::core::format::mime_typing::MimeType;
use crate::core::response::{ResponseData};
use crate::core::format::supported::{AtraSupportedFileFormat};
use crate::core::VecDataHolder;


/// Process the page to extract the mime type and the decoded data.
pub async fn process<'a>(context: &impl Context, page: &'a ResponseData, identified_type: &AtraFileInformation) -> Result<DecodedData<String, Utf8PathBuf>, DecodingError> {
    match &page.content {
        VecDataHolder::None => {
            return Ok(DecodedData::None)
        }
        _ => {}
    };
    let decoded = match identified_type.format {
        AtraSupportedFileFormat::HTML | AtraSupportedFileFormat::JavaScript | AtraSupportedFileFormat::PlainText | AtraSupportedFileFormat::Decodeable => decode(context, &page, &identified_type).await?.map_in_memory(|value| value.to_string()),
        _ if identified_type.check_has_document_type(MimeType::IS_UTF8) => do_decode(&page, UTF_8).await?.map_in_memory(|value| value.to_string()),
        _ => DecodedData::None
    };
    Ok(decoded)
}
