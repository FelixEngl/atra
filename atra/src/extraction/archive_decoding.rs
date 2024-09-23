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

use std::collections::HashSet;
use std::io::{Read, Seek};
use zip::result::{ZipError, ZipResult};
use crate::contexts::traits::{SupportsConfigs, SupportsGdbrRegistry};
use crate::data::RawData;
use crate::extraction::LinkExtractionError;
use crate::format::{FileFormatData, ZipFileContent};
use crate::toolkit::LanguageInformation;
use crate::url::UrlWithDepth;

/// Extract data fom
pub async fn extract_from_zip<C, R>(
    root_url: &UrlWithDepth,
    reader: R,
    context: &C,
) -> Result<(), LinkExtractionError> where
    C: SupportsGdbrRegistry + SupportsConfigs,
    R: Read + Seek
{
    let mut zip_reader = zip::read::ZipArchive::new(reader)?;
    let extracted_result = HashSet::new();
    for idx in 0..zip_reader.len() {
        match zip_reader.by_index_raw(idx) {
            Ok(file) => {
                if file.is_file() {

                    let content = ZipFileContent::new(
                        zip_reader.clone(),

                    );

                    let data = FileFormatData::new(
                        None,
                        &content,
                        None
                    );
                }
            }
            Err(error) => {
                match error {
                    ZipError::Io(err) => {
                        log::warn!("IOErrow while reading {}: {err}", root_url);
                    }
                    ZipError::InvalidArchive(err) => {
                        log::trace!("The archive {err} of {} is invalid.", root_url)
                    }
                    ZipError::UnsupportedArchive(err) => {
                        log::trace!("The archive {err} of {} is unsupported.", root_url)
                    }
                    ZipError::FileNotFound => {
                        log::trace!("The entry {name} in {} does not exist.", )
                    }
                    ZipError::InvalidPassword => {
                        log::trace!("Failed to decrypt {name} in {}", root_url)
                    }
                    unknown => {
                        log::trace!("Had an unkown error while unzipping {}: {unknown}", root_url)
                    }
                }
            }
        }
    }

    todo!()
}