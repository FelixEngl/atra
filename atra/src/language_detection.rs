use crate::decoding::DecodedData;
use crate::format::supported::InterpretedProcessibleFileFormat;
use crate::format::AtraFileInformation;
use crate::isolang_ext::ToIsoLang;
use crate::response::ResponseData;
use camino::Utf8PathBuf;
use isolang::Language;
use isolang::Language::Deu;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::min;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use ubyte::ByteUnit;
use whatlang::Script::Latin;
use whatlang::{Info, Script};
use xml::reader::{ParserConfig2, XmlEvent};
use xml::EventReader;
use crate::contexts::traits::SupportsConfigs;

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct LanguageInformation {
    script: Script,
    lang: Language,
    confidence: f64,
}

impl LanguageInformation {

    pub const DEU: LanguageInformation = LanguageInformation::with_confidence(Latin, Deu);
    pub const ENG: LanguageInformation = LanguageInformation::with_confidence(Latin, Language::Eng);

    pub fn script(&self) -> Script {
        self.script
    }

    pub fn lang(&self) -> Language {
        self.lang
    }

    pub fn confidence(&self) -> f64 {
        self.confidence
    }

    pub const fn new(script: Script, lang: Language, confidence: f64) -> Self {
        Self { script, lang, confidence }
    }

    pub const fn with_confidence(
        script: Script,
        lang: Language
    ) -> Self {
        Self::new(script, lang, 1f64)
    }
}

impl Eq for LanguageInformation {}

impl PartialEq for LanguageInformation {
    fn eq(&self, other: &Self) -> bool {
        self.lang == other.lang && self.script == other.script
    }
}

impl From<Info> for LanguageInformation {
    fn from(value: Info) -> Self {
        Self {
            script: value.script(),
            lang: value.lang().to_isolang(),
            confidence: value.confidence()
        }
    }
}

pub fn detect_language<'a>(
    context: &impl SupportsConfigs,
    _page: &'a ResponseData,
    file_type: &AtraFileInformation,
    decoded: &DecodedData<String, Utf8PathBuf>
) -> Result<Option<LanguageInformation>, std::io::Error> {
    const MAX_IN_MEMORY_FOR_LANG: u64 = 1u64 * ByteUnit::MB.as_u64();

    fn create_limited_sample_file_reader(context: &impl SupportsConfigs, path: impl AsRef<Path>) -> std::io::Result<impl Read> {
        let max_bytes = if let Some(mfs) = context.configs().crawl.max_file_size {
            min(mfs.get(), MAX_IN_MEMORY_FOR_LANG)
        } else {
            MAX_IN_MEMORY_FOR_LANG
        };
        Ok(BufReader::new(File::options().read(true).open(path)?.take(max_bytes)))
    }

    fn read_sample_file(context: &impl SupportsConfigs, path: impl AsRef<Path>) -> std::io::Result<Vec<u8>> {
        let mut reader = create_limited_sample_file_reader(context, path)?;
        let mut v = Vec::new();
        reader.read_to_end(&mut v)?;
        Ok(v)
    }

    match file_type.format {
        InterpretedProcessibleFileFormat::HTML => {
            match decoded {
                DecodedData::InMemory { data, .. } => {
                    let text = scraper::html::Html::parse_document(data.as_str()).root_element().text().collect::<String>();
                    Ok(whatlang::detect(&text).map(From::from))
                }
                _ => Ok(None)
            }
        }
        InterpretedProcessibleFileFormat::PlainText
        | InterpretedProcessibleFileFormat::StructuredPlainText
        | InterpretedProcessibleFileFormat::Decodeable => {
            match decoded {
                DecodedData::InMemory { data, .. } => {
                    Ok(whatlang::detect(&data).map(From::from))
                }
                DecodedData::OffMemory { reference,.. } => {
                    let data = read_sample_file(context, reference)?;
                    let text = String::from_utf8_lossy(&data);
                    Ok(whatlang::detect(&text).map(From::from))
                }
                DecodedData::None => Ok(None)
            }
        }
        InterpretedProcessibleFileFormat::JSON => {
            fn extract_string(value: Value) -> String {
                let mut result = String::new();
                let mut q = VecDeque::new();
                q.push_back(value);
                while let Some(value) = q.pop_back() {
                    match value {
                        Value::String(s) => {
                            result.push(' ');
                            result.push_str(&s);
                        }
                        Value::Array(values) => {
                            q.extend(values);
                        }
                        Value::Object(obj) => {
                            q.extend(obj.into_iter().map(|(_, v)| v))
                        }
                        _ => {}
                    }
                }
                result
            }

            match decoded {
                DecodedData::InMemory { data, .. } => {
                    if let Ok(deser) = serde_json::from_str::<Value>(data.as_str()) {
                        Ok(whatlang::detect(&extract_string(deser)).map(From::from))
                    } else {
                        Ok(whatlang::detect(data).map(From::from))
                    }
                }
                DecodedData::OffMemory { reference, .. } => {
                    let data = read_sample_file(context, reference)?;
                    let text = String::from_utf8_lossy(&data);
                    if let Ok(deser) = serde_json::from_str::<Value>(&text) {
                        Ok(whatlang::detect(&extract_string(deser)).map(From::from))
                    } else {
                        Ok(whatlang::detect(&text).map(From::from))
                    }
                }
                DecodedData::None => Ok(None)
            }
        }
        InterpretedProcessibleFileFormat::XML => {
            fn analyze_xml<R: Read>(s: EventReader<R>) -> Option<LanguageInformation> {
                let mut collected = String::with_capacity(MAX_IN_MEMORY_FOR_LANG as usize);
                for event in s {
                    if let Ok(event) = event {
                        match event {
                            XmlEvent::Characters(s) => {
                                collected.push(' ');
                                collected.push_str(&s);
                            }
                            _ => {}
                        }
                    }
                }
                whatlang::detect(&collected).map(From::from)
            }

            let cfg = ParserConfig2::new()
                .ignore_invalid_encoding_declarations(true)
                .ignore_comments(true);

            match decoded {
                DecodedData::InMemory { data, .. } => {
                    Ok(analyze_xml(EventReader::new_with_config(data.as_bytes(), cfg)))
                }
                DecodedData::OffMemory { reference, .. } => {
                    let reader = create_limited_sample_file_reader(context, reference)?;
                    Ok(analyze_xml(EventReader::new_with_config(reader, cfg)))
                }
                DecodedData::None => Ok(None)
            }
        }
        InterpretedProcessibleFileFormat::RTF => {

            fn analyze_rdf(s: &str) -> Option<LanguageInformation> {
                if let Ok(value) = rtf_parser::document::RtfDocument::try_from(s) {
                    whatlang::detect(&value.get_text()).map(From::from)
                } else {
                    whatlang::detect(s).map(From::from)
                }
            }

            match decoded {
                DecodedData::InMemory { data, .. } => {
                    Ok(analyze_rdf(data))
                }
                DecodedData::OffMemory { reference, .. } => {
                    let data = read_sample_file(context, reference)?;
                    let text = String::from_utf8_lossy(&data);
                    Ok(analyze_rdf(&text))
                }
                DecodedData::None => Ok(None)
            }
        }
        InterpretedProcessibleFileFormat::OOXML => {
            // todo: Needs unpacking and handling
            Ok(None)
        }
        InterpretedProcessibleFileFormat::ODF => {
            // todo: Needs unpacking and handling
            Ok(None)
        }
        _ => Ok(None)
    }
}