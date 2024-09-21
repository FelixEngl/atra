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

pub mod file_format_detection;
pub mod mime;
pub mod mime_ext;
pub(crate) mod mime_serialize;
pub mod supported;

use crate::contexts::traits::{SupportsConfigs, SupportsFileSystemAccess};
use crate::fetching::ResponseData;
use crate::format::file_format_detection::{infer_file_formats, DetectedFileFormat};
use crate::format::mime::{determine_mime_information, MimeType};
use crate::format::supported::InterpretedProcessibleFileFormat;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use warc::media_type::MediaType;

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

    pub fn determine<C: SupportsConfigs + SupportsFileSystemAccess>(
        context: &C,
        page: &ResponseData,
    ) -> Self {
        let mime = determine_mime_information(page);

        let detected = infer_file_formats(page, mime.as_ref(), context);

        let format = InterpretedProcessibleFileFormat::guess(
            page,
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
