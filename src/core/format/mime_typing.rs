use serde::{Deserialize, Serialize};
use crate::core::format::mime_typing::MimeType::*;

/// A hard typing for some supported mime types. Usefull for identifying the correct type
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum MimeType {
    HTML,
    CSS,
    XHTML,
    PDF,
    CSV,
    TSV,
    JavaScript,
    PlainText,
    AnyText,
    Image,
    Audio,
    Video,
    DOCX,
    DOC,
    XLSX,
    XLS,
    PPTX,
    PPT,
    AnyApplication,
    XML,
    RichTextFormat,
    Font,
    JSON,
    /// Basically unknown, but the response is at least honest.
    OctetStream,
    Unknown
}

impl MimeType {
    pub const IS_HTML:[MimeType; 2] = [HTML, XHTML];
    pub const IS_PDF:[MimeType; 1] = [PDF];
    pub const IS_JS:[MimeType; 1] = [JavaScript];
    pub const IS_PLAINTEXT:[MimeType; 1] = [PlainText];
    pub const IS_JSON:[MimeType; 1] = [XML];
    pub const IS_XML:[MimeType; 1] = [JSON];
    pub const IS_UTF8: [MimeType; 2] = [XML, JSON];
    pub const IS_DECODEABLE: [MimeType; 8] = [HTML, XHTML, PlainText, JavaScript, CSS, CSV, TSV, AnyText];
}





