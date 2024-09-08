pub mod supported;
pub mod file_format_detection;
pub mod mime;
pub(crate) mod mime_serialize;
pub mod mime_ext;

use serde::{Deserialize, Serialize};
use crate::core::format::supported::InterpretedProcessibleFileFormat;
use crate::core::contexts::Context;
use crate::core::format::file_format_detection::{DetectedFileFormat, infer_file_formats};
use crate::core::response::ResponseData;
use crate::core::format::mime::{determine_mime_information, MimeType};
use crate::warc::media_type::MediaType;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct AtraFileInformation {
    pub format: InterpretedProcessibleFileFormat,
    pub mime: Option<MimeType>,
    pub detected: Option<DetectedFileFormat>,
}

impl AtraFileInformation {


    pub fn new(
        format: InterpretedProcessibleFileFormat,
        mime: Option<MimeType>,
        detected: Option<DetectedFileFormat>,
    ) -> Self {
        Self { format, mime, detected }
    }

    pub fn determine(context: &impl Context, page: &ResponseData) -> Self {
        let mime = determine_mime_information(page);

        let detected = infer_file_formats(
            page,
            mime.as_ref(),
            context
        );

        let format = InterpretedProcessibleFileFormat::guess(
            page,
            mime.as_ref(),
            detected.as_ref(),
            context
        );

        Self {
            format,
            detected,
            mime
        }
    }

    pub fn is_decodeable(&self) -> bool {
        self.format.supports_decoding() || self.mime.as_ref().is_some_and(|value| value.get_param_values(mime::CHARSET).is_some())
    }

    pub fn get_best_media_type_for_warc(&self) -> MediaType {
        if let Some(ref mimes) = self.mime {
            if let Some(mime) = mimes.iter().next(){
                return MediaType::from_mime(mime)
            }
        }

        if let Some(ref detected) = self.detected {
            if let Some(mime) = detected.most_probable_file_format().media_type().parse() {
                return MediaType::from_mime(&mime)
            }
        }

        MediaType::from_mime(self.format.fallback_mime_type_for_warc())
    }
}




