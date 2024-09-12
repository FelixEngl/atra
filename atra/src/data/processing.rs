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

use crate::contexts::traits::{SupportsConfigs, SupportsFileSystemAccess};
use crate::data::{Decoded, RawVecData};
use crate::decoding::{decode, DecodingError};
use crate::format::AtraFileInformation;
use crate::fetching::ResponseData;
use camino::Utf8PathBuf;

/// Decode the data
pub async fn process<'a, C>(
    context: &C,
    page: &'a ResponseData,
    identified_type: &AtraFileInformation,
) -> Result<Decoded<String, Utf8PathBuf>, DecodingError>
where
    C: SupportsFileSystemAccess + SupportsConfigs,
{
    match &page.content {
        RawVecData::None => return Ok(Decoded::None),
        _ => {}
    };

    if identified_type.format.supports_decoding() {
        Ok(decode(context, &page, &identified_type)
            .await?
            .map_in_memory(|value| value.to_string()))
    } else {
        log::debug!("Decoding for {} not supported!", page.url.url);
        Ok(Decoded::None)
    }
}
