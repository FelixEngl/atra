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

use std::borrow::Cow;
use crate::contexts::traits::{SupportsConfigs, SupportsFileSystemAccess};
use crate::format::file_format_detection::{infer_file_formats, DetectedFileFormat};
use crate::format::mime::{determine_mime_information, MimeType};
use crate::format::supported::InterpretedProcessibleFileFormat;
use crate::format::FileContentReader;
use crate::toolkit::extension_extractor::extract_file_extensions_from_file_name;
use crate::url::UrlWithDepth;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use mime::Mime;
use warc::media_type::MediaType;

/// Holds the file information.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct AtraFileInformation {
    pub format: InterpretedProcessibleFileFormat,
    pub mime: Option<MimeType>,
    pub detected: Option<DetectedFileFormat>,
}

impl AtraFileInformation {
    #[cfg(test)]
    pub fn new(
        format: InterpretedProcessibleFileFormat,
        mime: Option<MimeType>,
        detected: Option<DetectedFileFormat>,
    ) -> Self {
        Self {
            format,
            mime,
            detected,
        }
    }

    /// Determines the file format for some data.
    /// Does not change the
    pub(crate) fn determine<C, D>(context: &C, data: &mut FileFormatData<D>) -> Self
    where
        C: SupportsConfigs + SupportsFileSystemAccess,
        D: FileContentReader,
    {
        let mime = determine_mime_information(data);

        let detected = infer_file_formats(data, mime.as_ref());

        let format = InterpretedProcessibleFileFormat::guess(
            data,
            mime.as_ref(),
            detected.as_ref(),
            context,
        );

        Self {
            format,
            detected,
            mime,
        }
    }

    #[cfg(test)]
    pub fn is_decodeable(&self) -> bool {
        self.format.supports_decoding()
            || self
                .mime
                .as_ref()
                .is_some_and(|value| value.get_param_values(mime::CHARSET).is_some())
    }

    pub fn get_best_mime_type(&self) -> Cow<Mime> {
        if let Some(ref mimes) = self.mime {
            if let Some(mime) = mimes.iter().next() {
                return Cow::Borrowed(mime);
            }
        }

        if let Some(ref detected) = self.detected {
            if let Ok(mime) = detected.most_probable_file_format().media_type().parse() {
                return Cow::Owned(mime);
            }
        }

        Cow::Borrowed(self.format.fallback_mime_type_for_warc())
    }

    pub fn get_best_media_type_for_warc(&self) -> MediaType {
        if let Some(ref mimes) = self.mime {
            if let Some(mime) = mimes.iter().next() {
                return MediaType::from_mime(mime);
            }
        }

        if let Some(ref detected) = self.detected {
            if let Ok(mime) = detected.most_probable_file_format().media_type().parse() {
                return MediaType::from_mime(&mime);
            }
        }

        MediaType::from_mime(self.format.fallback_mime_type_for_warc())
    }
}

impl Display for AtraFileInformation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FileFormat({}, {}, {})",
            self.format,
            self.mime
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default(),
            self.detected
                .as_ref()
                .map(|value| value.most_probable_file_format().to_string())
                .unwrap_or_default()
        )
    }
}

/// The data used to identify a file format.
/// This struct is NOT threadsafe, and can not be cloned as
/// the content reader is probably mutable.
#[derive(Debug)]
pub struct FileFormatData<'a, T> {
    pub(crate) headers: Option<&'a HeaderMap>,
    pub(crate) content: &'a mut T,
    pub(crate) url: Option<&'a UrlWithDepth>,
    pub(crate) file_name: Option<&'a str>,
}

impl<'a, T> FileFormatData<'a, T> {
    /// Returns the possible file endings, prefers a file name over the
    /// url.
    pub fn get_possible_file_endings(&self) -> Option<Vec<&str>> {
        if let Some(file_name) = self.file_name {
            extract_file_extensions_from_file_name(file_name)
        } else if let Some(url) = self.url {
            url.get_file_endings()
        } else {
            None
        }
    }
}

impl<'a, T> FileFormatData<'a, T>
where
    T: FileContentReader,
{
    pub fn new(
        headers: Option<&'a HeaderMap>,
        content: &'a mut T,
        url: Option<&'a UrlWithDepth>,
        file_name: Option<&'a str>,
    ) -> Self {
        Self {
            headers,
            content,
            url,
            file_name,
        }
    }

    delegate::delegate! {
        to self.content {
            pub fn can_read(&self) -> bool;
        }
    }
}
