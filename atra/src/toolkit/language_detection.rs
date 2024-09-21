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

use std::cmp::min;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use camino::Utf8PathBuf;
use isolang::Language;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ubyte::ByteUnit;
use whatlang::{Info, Script};
use xml::reader::{ParserConfig2, XmlEvent};
use xml::EventReader;

use crate::contexts::traits::SupportsConfigs;
use crate::data::Decoded;
use crate::fetching::ResponseData;
use crate::format::supported::InterpretedProcessibleFileFormat;
use crate::format::AtraFileInformation;
use crate::toolkit::isolang_ext::ToIsoLang;

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct LanguageInformation {
    script: Script,
    lang: Language,
    confidence: f64,
}

impl LanguageInformation {
    #[cfg(test)]
    pub const DEU: LanguageInformation =
        LanguageInformation::with_confidence(Script::Latin, Language::Deu);
    #[cfg(test)]
    pub const ENG: LanguageInformation =
        LanguageInformation::with_confidence(Script::Latin, Language::Eng);

    #[cfg(test)]
    pub fn script(&self) -> Script {
        self.script
    }

    pub fn lang(&self) -> Language {
        self.lang
    }

    pub fn confidence(&self) -> f64 {
        self.confidence
    }

    #[cfg(test)]
    pub const fn new(script: Script, lang: Language, confidence: f64) -> Self {
        Self {
            script,
            lang,
            confidence,
        }
    }

    #[cfg(test)]
    pub const fn with_confidence(script: Script, lang: Language) -> Self {
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
            confidence: value.confidence(),
        }
    }
}

pub fn detect_language<'a>(
    context: &impl SupportsConfigs,
    _page: &'a ResponseData,
    file_type: &AtraFileInformation,
    decoded: &Decoded<String, Utf8PathBuf>,
) -> Result<Option<LanguageInformation>, std::io::Error> {
    const MAX_IN_MEMORY_FOR_LANG: u64 = 1u64 * ByteUnit::MB.as_u64();

    fn create_limited_sample_file_reader(
        context: &impl SupportsConfigs,
        path: impl AsRef<Path>,
    ) -> std::io::Result<impl Read> {
        let max_bytes = if let Some(mfs) = context.configs().crawl.max_file_size {
            min(mfs.get(), MAX_IN_MEMORY_FOR_LANG)
        } else {
            MAX_IN_MEMORY_FOR_LANG
        };
        Ok(BufReader::new(
            File::options().read(true).open(path)?.take(max_bytes),
        ))
    }

    fn read_sample_file(
        context: &impl SupportsConfigs,
        path: impl AsRef<Path>,
    ) -> std::io::Result<Vec<u8>> {
        let mut reader = create_limited_sample_file_reader(context, path)?;
        let mut v = Vec::new();
        reader.read_to_end(&mut v)?;
        Ok(v)
    }

    match file_type.format {
        InterpretedProcessibleFileFormat::HTML => match decoded {
            Decoded::InMemory { data, .. } => {
                let text = scraper::html::Html::parse_document(data.as_str())
                    .root_element()
                    .text()
                    .collect::<String>();
                Ok(whatlang::detect(&text).map(From::from))
            }
            _ => Ok(None),
        },
        InterpretedProcessibleFileFormat::PlainText
        | InterpretedProcessibleFileFormat::StructuredPlainText
        | InterpretedProcessibleFileFormat::Decodeable => match decoded {
            Decoded::InMemory { data, .. } => Ok(whatlang::detect(&data).map(From::from)),
            Decoded::OffMemory { reference, .. } => {
                let data = read_sample_file(context, reference)?;
                let text = String::from_utf8_lossy(&data);
                Ok(whatlang::detect(&text).map(From::from))
            }
            Decoded::None => Ok(None),
        },
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
                        Value::Object(obj) => q.extend(obj.into_iter().map(|(_, v)| v)),
                        _ => {}
                    }
                }
                result
            }

            match decoded {
                Decoded::InMemory { data, .. } => {
                    if let Ok(deser) = serde_json::from_str::<Value>(data.as_str()) {
                        Ok(whatlang::detect(&extract_string(deser)).map(From::from))
                    } else {
                        Ok(whatlang::detect(data).map(From::from))
                    }
                }
                Decoded::OffMemory { reference, .. } => {
                    let data = read_sample_file(context, reference)?;
                    let text = String::from_utf8_lossy(&data);
                    if let Ok(deser) = serde_json::from_str::<Value>(&text) {
                        Ok(whatlang::detect(&extract_string(deser)).map(From::from))
                    } else {
                        Ok(whatlang::detect(&text).map(From::from))
                    }
                }
                Decoded::None => Ok(None),
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
                Decoded::InMemory { data, .. } => Ok(analyze_xml(EventReader::new_with_config(
                    data.as_bytes(),
                    cfg,
                ))),
                Decoded::OffMemory { reference, .. } => {
                    let reader = create_limited_sample_file_reader(context, reference)?;
                    Ok(analyze_xml(EventReader::new_with_config(reader, cfg)))
                }
                Decoded::None => Ok(None),
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
                Decoded::InMemory { data, .. } => Ok(analyze_rdf(data)),
                Decoded::OffMemory { reference, .. } => {
                    let data = read_sample_file(context, reference)?;
                    let text = String::from_utf8_lossy(&data);
                    Ok(analyze_rdf(&text))
                }
                Decoded::None => Ok(None),
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
        _ => Ok(None),
    }
}
