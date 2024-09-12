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

use crate::contexts::traits::{SupportsConfigs, SupportsFileSystemAccess};
use crate::fetching::ResponseData;
use crate::format::file_format_detection::DetectedFileFormat;
use crate::format::mime::MimeType;
use crate::format::mime_ext;
use file_format::{FileFormat, Kind};
use mime::Mime;
use serde::{Deserialize, Serialize};
use std::cmp::max;
use std::io::Read;
use std::str::FromStr;
use strum::Display;
// https://gonze.com/playlists/playlist-format-survey.html#M3U

/// The inferred processable, type for a complete page for this crawler.
/// Does not give detailed information about the real type.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize, Display)]
pub enum InterpretedProcessibleFileFormat {
    HTML,
    PDF,
    JavaScript,

    /// Plaintext
    PlainText,
    StructuredPlainText,
    ProgrammingLanguage,

    MP3Url,

    JSON,

    XML,
    SVG,

    RTF,
    OOXML,
    ODF,
    IMAGE,

    Decodeable,

    /// Usually a binary format. But can be anything that can not be decoded by normal means. (Like a ZIP-File)
    Unsupported,
    Unknown, // todo: Add identifier for binary
}

impl InterpretedProcessibleFileFormat {
    pub fn supports_decoding(&self) -> bool {
        !matches!(
            self,
            Self::Unsupported | Self::Unknown | Self::IMAGE | Self::RTF | Self::OOXML | Self::ODF
        )
    }

    pub fn fallback_mime_type_for_warc(&self) -> &Mime {
        match self {
            InterpretedProcessibleFileFormat::HTML => &mime::TEXT_HTML,
            InterpretedProcessibleFileFormat::PDF => &mime::APPLICATION_PDF,
            InterpretedProcessibleFileFormat::JavaScript => &mime::APPLICATION_JAVASCRIPT,
            InterpretedProcessibleFileFormat::PlainText => &mime::TEXT_PLAIN,
            InterpretedProcessibleFileFormat::JSON => &mime::APPLICATION_JSON,
            InterpretedProcessibleFileFormat::XML => &mime_ext::APPLICATION_XML,
            InterpretedProcessibleFileFormat::RTF => &mime_ext::APPLICATION_RTF,
            InterpretedProcessibleFileFormat::OOXML => &mime_ext::APPLICATION_OOXML_STAR,
            InterpretedProcessibleFileFormat::ODF => &mime_ext::APPLICATION_ODF_STAR,
            InterpretedProcessibleFileFormat::IMAGE => &mime::IMAGE_STAR,
            InterpretedProcessibleFileFormat::SVG => &mime::IMAGE_SVG,
            InterpretedProcessibleFileFormat::MP3Url => &mime_ext::AUDIO_MP3_URL,
            InterpretedProcessibleFileFormat::StructuredPlainText
            | InterpretedProcessibleFileFormat::ProgrammingLanguage => &mime::TEXT_STAR,
            InterpretedProcessibleFileFormat::Unsupported
            | InterpretedProcessibleFileFormat::Decodeable
            | InterpretedProcessibleFileFormat::Unknown => &mime::APPLICATION_OCTET_STREAM,
        }
    }
}

fn html_heuristic(to_check: &[u8]) -> bool {
    #[inline(always)]
    fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    let doctype = find_subsequence(to_check, b"<!DOCTYPE html");
    if let Some(ref doctype) = doctype {
        if 0usize.eq(doctype) {
            return true;
        }
    }
    let html_start = find_subsequence(to_check, b"<html");
    let html_end = find_subsequence(to_check, b"</html>");

    if let Some(end) = html_end {
        if let Some(start) = html_start {
            if start < end {
                true
            } else {
                false
            }
        } else {
            false
        }
    } else if let Some(start) = html_start {
        if start == 0 {
            true
        } else if let Some(doctype) = doctype {
            doctype < start
        } else {
            false
        }
    } else {
        false
    }
}

macro_rules! supports_method {
    ($(
        $typ: ident: $pattern:pat $(if $guard:expr)? $(,)?
    )+) => {
        fn mime_2_supported_file_format(mime: &Mime) -> Option<InterpretedProcessibleFileFormat> {
            let typ = mime.type_().as_str().to_lowercase();
            let sub_typ = mime.subtype().as_str().to_lowercase();
            let suffix = mime.suffix().map(|value| value.as_str().to_lowercase());
            match (typ.as_str(), sub_typ.as_str(), suffix.as_deref()) {
                $($pattern $(if $guard)? => return Some(InterpretedProcessibleFileFormat::$typ),)+
                _ => None
            }
        }
    };
}

macro_rules! supports_fileending_method {
    ($(
        $typ: ident: $pattern:pat $(if $guard:expr)? $(,)?
    )+) => {
        fn extension_2_supported_file_format(response: &ResponseData) -> Option<InterpretedProcessibleFileFormat>{
            if let Some(file_endings) = response.url.url().get_file_endings() {
                let last = *file_endings.last()?;
                match last {
                    $(
                    $pattern $(if $guard)? => Some(InterpretedProcessibleFileFormat::$typ),
                    )+
                    _ => None
                }
            } else {
                None
            }
        }
    };
}

impl InterpretedProcessibleFileFormat {
    supports_fileending_method! {
        HTML: "html" | "xhtml" | "htm"
        PDF: "pdf"
        RTF: "rtf"
        JavaScript: "js"
        PlainText: "txt"
        JSON: "json"
        XML: "xml"
        OOXML: "xslx" | "docx" | "pptx"
        ODF: "odt"|"ods"|"odp"|"odg"|"odc"|"odf"|"odi"|"odm"|"ott"|"ots"|"otp"|"otg"|"otf"|"oth"|"oti"|"otc"
        StructuredPlainText: "csv"
        ProgrammingLanguage: "css"
    }

    supports_method! {
        HTML: ("text", "html", _)
        PDF: (_, "pdf", _)
        RTF: (_, "rdf", _)
        JavaScript: (_, "javascript", _)
        PlainText: ("text", "plain", _)
        JSON: (_, "json", _) | (_, _, Some("json"))
        XML: (_, "xml", _) | (_, _, Some("xml"))
        ProgrammingLanguage: (_, "css", _)
        StructuredPlainText: (_, "csv", _)
    }

    /// Tries to guess the supported file type.
    pub fn guess<C>(
        page: &ResponseData,
        mime: Option<&MimeType>,
        file_format: Option<&DetectedFileFormat>,
        context: &C,
    ) -> InterpretedProcessibleFileFormat
    where
        C: SupportsFileSystemAccess + SupportsConfigs,
    {
        let mut is_text = false;

        if let Some(detected) = file_format {
            let most_probable = detected.most_probable_file_format();

            match most_probable {
                FileFormat::HypertextMarkupLanguage => {
                    return InterpretedProcessibleFileFormat::HTML
                }
                // TODO: Calendar format
                FileFormat::Icalendar
                | FileFormat::Vcalendar
                | FileFormat::Vcard
                | FileFormat::WebVideoTextTracks => {
                    return InterpretedProcessibleFileFormat::StructuredPlainText
                }

                // TODO: Programming Language
                FileFormat::LuaScript
                | FileFormat::PythonScript
                | FileFormat::RubyScript
                | FileFormat::ShellScript
                | FileFormat::ToolCommandLanguageScript
                | FileFormat::MsDosBatch
                | FileFormat::PerlScript
                | FileFormat::Latex
                | FileFormat::ClojureScript
                | FileFormat::WebassemblyText => {
                    return InterpretedProcessibleFileFormat::ProgrammingLanguage
                }

                FileFormat::RichTextFormat => return InterpretedProcessibleFileFormat::RTF,
                FileFormat::PortableDocumentFormat => return InterpretedProcessibleFileFormat::PDF,
                FileFormat::ScalableVectorGraphics => return InterpretedProcessibleFileFormat::SVG,
                FileFormat::ExtensibleMarkupLanguage => {
                    return InterpretedProcessibleFileFormat::XML
                }
                FileFormat::OfficeOpenXmlDocument
                | FileFormat::OfficeOpenXmlDrawing
                | FileFormat::OfficeOpenXmlPresentation
                | FileFormat::OfficeOpenXmlSpreadsheet => {
                    return InterpretedProcessibleFileFormat::OOXML
                }
                FileFormat::OpendocumentDatabase
                | FileFormat::OpendocumentFormula
                | FileFormat::OpendocumentFormulaTemplate
                | FileFormat::OpendocumentGraphics
                | FileFormat::OpendocumentGraphicsTemplate
                | FileFormat::OpendocumentPresentation
                | FileFormat::OpendocumentPresentationTemplate
                | FileFormat::OpendocumentSpreadsheet
                | FileFormat::OpendocumentSpreadsheetTemplate
                | FileFormat::OpendocumentText
                | FileFormat::OpendocumentTextMaster
                | FileFormat::OpendocumentTextMasterTemplate
                | FileFormat::OpendocumentTextTemplate => {
                    return InterpretedProcessibleFileFormat::ODF
                }

                FileFormat::MayaAscii
                | FileFormat::Model3dAscii
                | FileFormat::BmfontAscii
                | FileFormat::DrawingExchangeFormatAscii
                | FileFormat::PolygonAscii
                | FileFormat::UniversalSceneDescriptionAscii
                | FileFormat::StereolithographyAscii => {
                    return InterpretedProcessibleFileFormat::PlainText
                }
                FileFormat::MayaBinary
                | FileFormat::Model3dBinary
                | FileFormat::BmfontBinary
                | FileFormat::DrawingExchangeFormatBinary
                | FileFormat::PolygonBinary
                | FileFormat::UniversalSceneDescriptionBinary => {
                    return InterpretedProcessibleFileFormat::Unsupported
                }
                FileFormat::Mp3Url => return InterpretedProcessibleFileFormat::MP3Url,
                FileFormat::Empty => return InterpretedProcessibleFileFormat::Unsupported,
                // FileFormat::PlainText => {/* Plaintext has to be handles below due to HTML etc. */}
                other => {
                    if other.media_type().contains("+xml") {
                        return InterpretedProcessibleFileFormat::XML;
                    } else {
                        match other.kind() {
                            Kind::Image => return InterpretedProcessibleFileFormat::IMAGE,
                            // TODO: Zip Files etc.
                            Kind::Executable
                            | Kind::Font
                            | Kind::Archive
                            | Kind::Package
                            | Kind::Compressed
                            | Kind::Disk
                            | Kind::Video => return InterpretedProcessibleFileFormat::Unsupported,
                            _ => {}
                        }
                    }
                }
            }
        }

        if let Some(mimes) = mime.map(|value| value.iter()) {
            for mime in mimes {
                if let Some(by_mime) = Self::mime_2_supported_file_format(mime) {
                    return by_mime;
                }
                if mime.subtype() == mime::TEXT {
                    is_text = true;
                }
            }
        }

        if let Some(found) = Self::extension_2_supported_file_format(page) {
            return found;
        }

        if let Some(file_format) = file_format {
            let mime = Mime::from_str(file_format.most_probable_file_format().media_type())
                .expect("The mimes in file_format are always valid!");
            if let Some(found) = Self::mime_2_supported_file_format(&mime) {
                return found;
            }
        }

        fn guess_if_html<const HAYSTACK_SIZE: usize>(
            context: &impl SupportsFileSystemAccess,
            page: &ResponseData,
        ) -> bool {
            if let Ok(Some(mut reader)) = page.content.cursor(context) {
                let mut haystack = [0u8; HAYSTACK_SIZE];
                match reader.read(&mut haystack) {
                    Ok(_) => html_heuristic(&haystack),
                    Err(_) => false,
                }
            } else {
                false
            }
        }

        // Grabbing straws

        if is_text {
            if guess_if_html::<512>(context, page) {
                Self::HTML
            } else {
                Self::Decodeable
            }
        } else if let Ok(Some(reader)) = page.content.cursor(context) {
            let mut result = Vec::new();
            if context.configs().system.max_file_size_in_memory < 512 {
                if guess_if_html::<512>(context, page) {
                    Self::HTML
                } else {
                    Self::Unknown
                }
            } else {
                match reader
                    .take(max(context.configs().system.max_file_size_in_memory, 512))
                    .read_to_end(&mut result)
                {
                    Ok(_) => {
                        if html_heuristic(&result) {
                            Self::HTML
                        } else {
                            Self::Unknown
                        }
                    }
                    Err(_) => Self::Unknown,
                }
            }
        } else {
            Self::Unknown
        }
    }
}
