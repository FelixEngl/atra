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

use crate::io::serial::{NoSerial, SerialProvider};
use crate::io::templating::TemplateError::ArgumentMissing;
use data_encoding::BASE64URL_NOPAD;
use std::borrow::{Borrow, Cow};
use std::collections::HashMap;
use std::fmt::{Display, Formatter, Write};
use std::hash::Hash;
use std::sync::Arc;
use thiserror::Error;
use time::format_description::{parse_owned, OwnedFormatItem};
use time::OffsetDateTime;

macro_rules! file_name_template_element {
    ($result: ident, $value: literal $($tt:tt)*) => {
        if let Ok(res) = $result.as_mut() {
            res.push($crate::io::templating::FileNameTemplateElement::Static($value));
        }
        crate::io::templating::file_name_template_element!($result, $($tt)*);
    };
    ($result: ident, dyn @ $value: tt $($tt:tt)*) => {
        if let Ok(res) = $result.as_mut() {
            res.push($crate::io::templating::FileNameTemplateElement::Dynamic($value.to_string()));
        }
        crate::io::templating::file_name_template_element!($result, $($tt)*);
    };
    ($result: ident, arg @ $value: literal $($tt:tt)*) => {
        if let Ok(res) = $result.as_mut() {
            res.push($crate::io::templating::FileNameTemplateElement::Arg($value, false));
        }
        crate::io::templating::file_name_template_element!($result, $($tt)*);
    };
    ($result: ident, arg! @ $value: literal $($tt:tt)*) => {
        if let Ok(res) = $result.as_mut() {
            res.push($crate::io::templating::FileNameTemplateElement::Arg($value, true));
        }
        crate::io::templating::file_name_template_element!($result, $($tt)*);
    };
    ($result: ident, timestamp $($tt:tt)*) => {
        if let Ok(res) = $result.as_mut() {
            res.push($crate::io::templating::FileNameTemplateElement::UnixTimestamp(false));
        }
        crate::io::templating::file_name_template_element!($result, $($tt)*);
    };
    ($result: ident, timestamp64 $($tt:tt)*) => {
        if let Ok(res) = $result.as_mut() {
            res.push($crate::io::templating::FileNameTemplateElement::UnixTimestamp(true));
        }
        crate::io::templating::file_name_template_element!($result, $($tt)*);
    };
    ($result: ident, timestamp @ $value: tt $($tt:tt)*) => {
        $result = match $result {
            Ok(mut res) => {
                match $crate::io::templating::FileNameTemplateElement::formatted_timestamp($value) {
                    Ok(value) => {
                        res.push($crate::io::templating::FileNameTemplateElement::FormattedTimestamp(value));
                        Ok(res)
                    };
                    Err(err) => {
                        Err(err)
                    }
                }
            }
            Err(res) => {
                Err(res)
            }
        }
        crate::io::templating::file_name_template_element!($result, $($tt)*);
    };
    ($result: ident, serial $($tt:tt)*) => {
        if let Ok(res) = $result.as_mut() {
            res.push($crate::io::templating::FileNameTemplateElement::Serial);
        }
        crate::io::templating::file_name_template_element!($result, $($tt)*);
    };
    ($result: ident, ref $value: ident $($tt:tt)*) => {
        if let Ok(res) = $result.as_mut() {
            res.push($crate::io::templating::FileNameTemplateElement::FileNameTemplate($value.clone()));
        }
        crate::io::templating::file_name_template_element!($result, $($tt)*);
    };
    ($result: ident, raw @ $value: tt $($tt:tt)*) => {
        if let Ok(res) = $result.as_mut() {
            res.push($value);
        }
        crate::io::templating::file_name_template_element!($result, $($tt)*);
    };
    ($result: ident, sep @ $value:tt $($tt:tt)*) => {
        if let Ok(res) = $result.as_mut() {
            res.push($crate::io::templating::FileNameTemplateElement::Static(stringify!($value)));
        }
        crate::io::templating::file_name_template_element!($result, $($tt)*);
    };
    ($result: ident, $value:ident $($tt:tt)*) => {
        if let Ok(res) = $result.as_mut() {
            res.push($crate::io::templating::FileNameTemplateElement::Dynamic((&$value).to_string()));
        }
        crate::io::templating::file_name_template_element!($result, $($tt)*);
    };
    ($result: ident, $value:tt $($tt:tt)*) => {
        if let Ok(res) = $result.as_mut() {
            res.push($crate::io::templating::FileNameTemplateElement::Static(stringify!($value)));
        }
        crate::io::templating::file_name_template_element!($result, $($tt)*);
    };
    ($result: ident,) => {}
}

/// A macro to generate file template names.
macro_rules! file_name_template {
    ($($tt:tt)+) => {
        {
            let mut result: Result<Vec<$crate::io::templating::FileNameTemplateElement>, time::error::InvalidFormatDescription> = Ok(Vec::new());
            crate::io::templating::file_name_template_element!(result, $($tt)+);
            match result {
                Ok(mut result) => {
                    result.shrink_to_fit();
                    Ok($crate::io::templating::FileNameTemplate::new(std::sync::Arc::new(result)))
                }
                Err(err) => {
                    Err(err)
                }
            }
        }
    }
}

pub(crate) use {file_name_template, file_name_template_element};

/// A template for a filename
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct FileNameTemplate {
    parts: Arc<Vec<FileNameTemplateElement>>,
}

impl FileNameTemplate {
    pub fn new(parts: Arc<Vec<FileNameTemplateElement>>) -> Self {
        Self { parts }
    }

    /// Writes the template element to `f`. Returns true if some kind of content was written.
    pub fn write(
        &self,
        f: &mut impl Write,
        serial_provider: &impl SerialProvider,
        args: Option<&FileNameTemplateArgs>,
    ) -> Result<bool, TemplateError> {
        let mut wrote_something = false;
        for value in self.parts.iter() {
            wrote_something |= value.write(f, serial_provider, args)?;
        }
        Ok(wrote_something)
    }
}

impl Display for FileNameTemplate {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Ok(_) = self.write(f, &NoSerial::<u8>::default(), None) {
            Ok(())
        } else {
            Err(std::fmt::Error)
        }
    }
}

impl From<Vec<FileNameTemplateElement>> for FileNameTemplate {
    fn from(value: Vec<FileNameTemplateElement>) -> Self {
        Self::new(Arc::new(value))
    }
}

/// Template args
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct FileNameTemplateArgs(HashMap<String, Cow<'static, str>>);

impl FileNameTemplateArgs {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(HashMap::with_capacity(capacity))
    }

    pub fn insert_str(
        &mut self,
        key: impl AsRef<str>,
        value: &'static str,
    ) -> Option<Cow<'static, str>> {
        self.0
            .insert(key.as_ref().to_string(), Cow::Borrowed(value))
    }

    pub fn insert(&mut self, key: impl AsRef<str>, value: String) -> Option<Cow<'static, str>> {
        self.0.insert(key.as_ref().to_string(), Cow::Owned(value))
    }

    pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&Cow<'static, str>>
    where
        String: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.0.get(key)
    }

    pub fn insert_value(
        &mut self,
        key: impl AsRef<str>,
        value: impl ToString,
    ) -> Option<Cow<'static, str>> {
        self.0
            .insert(key.as_ref().to_string(), Cow::Owned(value.to_string()))
    }
}

impl Extend<(String, Cow<'static, str>)> for FileNameTemplateArgs {
    fn extend<T: IntoIterator<Item = (String, Cow<'static, str>)>>(&mut self, iter: T) {
        self.0.extend(iter)
    }
}

#[derive(Debug, Clone)]
pub enum FileNameTemplateElement {
    Static(&'static str),
    Dynamic(String),
    Arg(&'static str, bool),
    UnixTimestamp(bool),
    FormattedTimestamp(OwnedFormatItem),
    Serial,
    FileNameTemplate(FileNameTemplate),
}

#[derive(Debug, Error)]
pub enum TemplateError {
    #[error(transparent)]
    Time(#[from] time::error::Format),
    #[error(transparent)]
    Fmt(#[from] std::fmt::Error),
    #[error("The required argument value {0:?} is missing!")]
    ArgumentMissing(&'static str),
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
        serial_provider: &impl SerialProvider,
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

#[cfg(test)]
mod test {
    use crate::io::serial::DefaultSerialProvider;
    use crate::io::templating::FileNameTemplateArgs;

    #[test]
    fn can_build() {
        let serial_provider = DefaultSerialProvider::default();

        let template1 = file_name_template!(
            "wasser" _ "<ist>" _ "nass"
        )
        .expect("Why?");

        let mut s = String::new();
        s.push('a');

        let template = file_name_template!(
            s _ "test" _ ref template1 _ arg@"testi" _ "here" _ dyn@123 _ timestamp _ serial ".exe"
        )
        .expect("Why?");

        let mut result = String::new();

        let mut args = FileNameTemplateArgs::new();
        args.insert_str("testi", "<my_testi_value>");

        template
            .write(&mut result, &serial_provider, Some(&args))
            .expect("Success!");

        assert!(result.starts_with("test_wasser_<ist>_nass_<my_testi_value>_here_123_"));
        assert!(result.ends_with("_0.exe"));
    }
}
