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

use std::fmt::Write;
use data_encoding::BASE64URL_NOPAD;
use time::format_description::{parse_owned, OwnedFormatItem};
use time::OffsetDateTime;
use crate::io::serial::SerialProvider;
use crate::io::templating::{FileNameTemplate, FileNameTemplateArgs, TemplateError};
use crate::io::templating::TemplateError::ArgumentMissing;

#[derive(Debug, Clone)]
pub enum FileNameTemplateElement {
    Static(&'static str),
    Dynamic(String),
    Arg(&'static str, bool),
    UnixTimestamp(bool),
    FormattedTimestamp(OwnedFormatItem),
    Serial,
    CustomSerial(SerialProvider),
    FileNameTemplate(FileNameTemplate),
}


impl FileNameTemplateElement {
    pub fn formatted_timestamp(
        format: impl AsRef<str>,
    ) -> Result<Self, time::error::InvalidFormatDescription> {
        Ok(Self::FormattedTimestamp(parse_owned::<2>(format.as_ref())?))
    }

    /// Writes the template element to `f`. Returns true if some kind of content was written.
    pub fn write(
        &self,
        f: &mut impl Write,
        serial_provider: &SerialProvider,
        args: Option<&FileNameTemplateArgs>,
    ) -> Result<bool, TemplateError> {
        match self {
            FileNameTemplateElement::Static(value) => write!(f, "{}", value)?,
            FileNameTemplateElement::Dynamic(value) => write!(f, "{}", value)?,
            FileNameTemplateElement::UnixTimestamp(base64) => {
                if *base64 {
                    BASE64URL_NOPAD.encode_write(
                        &OffsetDateTime::now_utc()
                            .unix_timestamp_nanos()
                            .to_be_bytes(),
                        f,
                    )?;
                } else {
                    write!(f, "{}", OffsetDateTime::now_utc().unix_timestamp_nanos())?
                }
            }
            FileNameTemplateElement::FormattedTimestamp(value) => {
                write!(f, "{}", OffsetDateTime::now_utc().format(value)?)?
            }
            FileNameTemplateElement::Serial => {
                if let Some(serial) = serial_provider.provide_serial() {
                    write!(f, "{serial}")?
                } else {
                    return Ok(false);
                }
            }
            FileNameTemplateElement::CustomSerial(provider) => {
                if let Some(serial) = provider.provide_serial() {
                    write!(f, "{serial}")?
                } else {
                    return Ok(false);
                }
            }
            FileNameTemplateElement::FileNameTemplate(template) => {
                return template.write(f, serial_provider, args)
            }
            FileNameTemplateElement::Arg(key, required) => {
                if let Some(template_args) = args {
                    if let Some(found) = template_args.get(*key) {
                        write!(f, "{found}")?
                    } else {
                        return if *required {
                            Err(ArgumentMissing(*key))
                        } else {
                            Ok(false)
                        };
                    }
                } else {
                    return if *required {
                        Err(ArgumentMissing(*key))
                    } else {
                        Ok(false)
                    };
                }
            }
        }
        Ok(true)
    }

    pub fn write_current(
        &self,
        f: &mut impl Write,
        serial_provider: &SerialProvider,
        args: Option<&FileNameTemplateArgs>,
    ) -> Result<bool, TemplateError> {
        match self {
            FileNameTemplateElement::Static(value) => write!(f, "{}", value)?,
            FileNameTemplateElement::Dynamic(value) => write!(f, "{}", value)?,
            FileNameTemplateElement::UnixTimestamp(base64) => {
                if *base64 {
                    BASE64URL_NOPAD.encode_write(
                        &OffsetDateTime::now_utc()
                            .unix_timestamp_nanos()
                            .to_be_bytes(),
                        f,
                    )?;
                } else {
                    write!(f, "{}", OffsetDateTime::now_utc().unix_timestamp_nanos())?
                }
            }
            FileNameTemplateElement::FormattedTimestamp(value) => {
                write!(f, "{}", OffsetDateTime::now_utc().format(value)?)?
            }
            FileNameTemplateElement::Serial => {
                if let Some(serial) = serial_provider.current_serial() {
                    write!(f, "{serial}")?
                } else {
                    return Ok(false);
                }
            }
            FileNameTemplateElement::CustomSerial(provider) => {
                if let Some(serial) = provider.current_serial() {
                    write!(f, "{serial}")?
                } else {
                    return Ok(false);
                }
            }
            FileNameTemplateElement::FileNameTemplate(template) => {
                return template.write_current(f, serial_provider, args)
            }
            FileNameTemplateElement::Arg(key, required) => {
                if let Some(template_args) = args {
                    if let Some(found) = template_args.get(*key) {
                        write!(f, "{found}")?
                    } else {
                        return if *required {
                            Err(ArgumentMissing(*key))
                        } else {
                            Ok(false)
                        };
                    }
                } else {
                    return if *required {
                        Err(ArgumentMissing(*key))
                    } else {
                        Ok(false)
                    };
                }
            }
        }
        Ok(true)
    }
}

impl From<FileNameTemplate> for FileNameTemplateElement {
    fn from(value: FileNameTemplate) -> Self {
        FileNameTemplateElement::FileNameTemplate(value)
    }
}

impl From<OwnedFormatItem> for FileNameTemplateElement {
    fn from(value: OwnedFormatItem) -> Self {
        FileNameTemplateElement::FormattedTimestamp(value)
    }
}

impl From<String> for FileNameTemplateElement {
    fn from(value: String) -> Self {
        FileNameTemplateElement::Dynamic(value)
    }
}

impl From<&'static str> for FileNameTemplateElement {
    fn from(value: &'static str) -> Self {
        FileNameTemplateElement::Static(value.into())
    }
}