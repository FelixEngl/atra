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
use file_format::Kind;
use crate::core::contexts::Context;
use crate::core::decoding::{decode, DecodedData, DecodingError};
use crate::core::format::AtraFileInformation;
use crate::core::format::file_format_detection::DetectedFileFormat;
use crate::core::response::{ResponseData};
use crate::core::VecDataHolder;


/// Decode the data
pub async fn process<'a>(context: &impl Context, page: &'a ResponseData, identified_type: &AtraFileInformation) -> Result<DecodedData<String, Utf8PathBuf>, DecodingError> {
    match &page.content {
        VecDataHolder::None => {
            return Ok(DecodedData::None)
        }
        _ => {}
    };

    if let Some(ref detected) = identified_type.detected {
        match detected {
            DetectedFileFormat::Unambiguous(value) | DetectedFileFormat::Ambiguous(value, _, _) => {
                if value.kind() == Kind::Document {

                } else {

                }
            }
        }
    }

    if identified_type.format.supports_decoding() {
        Ok(decode(context, &page, &identified_type).await?.map_in_memory(|value| value.to_string()))
    } else {
        log::debug!("Decoding for {} not supported!", page.url.url);
        Ok(DecodedData::None)
    }

    // let decoded = match identified_type.format {
    //      =>
    //     _ if identified_type.check_has_document_type(MimeType::IS_UTF8) => do_decode(&page, UTF_8)?.map_in_memory(|value| value.to_string()),
    //     _ => DecodedData::None
    // };
}
