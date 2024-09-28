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

use crate::data::{Decoded, RawVecData};
use crate::fetching::ResponseData;
use crate::format::AtraFileInformation;
use crate::toolkit::LanguageInformation;
use crate::url::UrlWithDepth;
use camino::Utf8PathBuf;

/// A reference to all contents available to extract the data.
#[derive(Debug, Copy, Clone)]
pub struct ExtractorData<'a> {
    pub url: &'a UrlWithDepth,
    pub file_name: Option<&'a str>,
    pub raw_data: &'a RawVecData,
    pub file_info: &'a AtraFileInformation,
    pub decoded: &'a Decoded<String, Utf8PathBuf>,
    pub language: Option<&'a LanguageInformation>,
}

impl<'a> ExtractorData<'a> {
    pub fn new_from_response(
        data: &'a ResponseData,
        file_info: &'a AtraFileInformation,
        decoded: &'a Decoded<String, Utf8PathBuf>,
        language: Option<&'a LanguageInformation>,
    ) -> Self {
        Self {
            url: &data.url,
            file_name: None,
            raw_data: &data.content,
            file_info,
            decoded,
            language,
        }
    }

    pub fn new(
        url: &'a UrlWithDepth,
        file_name: Option<&'a str>,
        raw_data: &'a RawVecData,
        file_info: &'a AtraFileInformation,
        decoded: &'a Decoded<String, Utf8PathBuf>,
        language: Option<&'a LanguageInformation>,
    ) -> Self {
        Self {
            url,
            file_name,
            raw_data,
            file_info,
            decoded,
            language,
        }
    }
}
