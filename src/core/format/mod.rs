pub mod supported;
pub mod file_format_detection;
pub mod mime;
pub mod mime_typing;

use encoding_rs::Encoding;
use scraper::Html;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use crate::core::format::supported::AtraSupportedFileFormat;
use crate::core::contexts::Context;
use crate::core::format::file_format_detection::{DetectedFileFormat, infer_file_formats};
use crate::core::format::mime::{EncodingSupplier, MimeDescriptor, TypedMime};
use crate::core::format::mime_typing::MimeType;
use crate::core::response::ResponseData;
use crate::static_selectors;
use std::slice::Iter;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct AtraFileInformation {
    pub format: AtraSupportedFileFormat,
    pub mime: MimeDescriptor,
    pub detected: Option<DetectedFileFormat>
}

impl AtraFileInformation {


    pub fn new(format: AtraSupportedFileFormat, mime: MimeDescriptor, detected: Option<DetectedFileFormat>) -> Self {
        Self { format, mime, detected }
    }

    pub fn determine(context: &impl Context, page: &ResponseData) -> Self {
        static_selectors! {
            [
                META_CONTENT = "meta[http-equiv=\"Content-Type\"][content]"
            ]
        }

        fn parse_page_raw(page: &[u8]) -> MimeDescriptor {
            Html::parse_document(&String::from_utf8_lossy(page))
                .select(&META_CONTENT)
                .filter_map(|value| value.attr("content").map(|value|value.parse::<MimeDescriptor>().unwrap()))
                .flatten()
                .collect()
        }

        let mime =
            if let Some(result) = page.headers.as_ref().map(|value| MimeDescriptor::from(value)) {
                if result.check_has_document_type(MimeType::IS_HTML) {
                    if let Some(dat) = page.content.as_in_memory() {
                        result.into_iter().chain(parse_page_raw(dat.as_slice())).collect()
                    } else {
                        log::debug!("Unable to parse the html because of its size: {:?}!", page.content);
                        MimeDescriptor::Empty
                    }
                } else {
                    result
                }
            } else {
                if let Some(dat) = page.content.as_in_memory() {
                    parse_page_raw(dat.as_slice())
                } else {
                    log::debug!("Unable to parse the html because of its size!");
                    MimeDescriptor::Empty
                }
            };

        let detected = infer_file_formats(
            page,
            &mime,
            context
        );

        let format = AtraSupportedFileFormat::infer(
            page,
            &mime,
            context
        );



        Self {
            format,
            detected,
            mime
        }
    }

    pub fn determine_decoding_by_mime(&self) -> SmallVec<[&'static Encoding; 8]> {
        // todo: currently only based on type
        self.mime
            .iter()
            .map(|value| value.get_encoding())
            .flatten()
            .collect()
    }

    delegate::delegate! {
        to self.mime {
            // pub fn map<F, R>(&self, f: F) -> Option<SmallVec<[R; 8]>> where F: Fn(&TypedMime) -> R;
            #[allow(dead_code)] pub fn check_if<F>(&self, check: F) -> Option<bool> where F: Fn(&TypedMime) -> bool;
            pub fn check_has_document_type<const N: usize>(&self, types: [MimeType; N]) -> bool;
            #[allow(dead_code)] pub fn iter(&self) -> Iter<TypedMime>;
        }
    }

}




