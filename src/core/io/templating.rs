use std::fmt::{Display, Formatter, Write};
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use itertools::{Itertools};
use thiserror::Error;
use time::format_description::{OwnedFormatItem, parse_owned};
use time::OffsetDateTime;

macro_rules! file_name_template_element {
    ($result: ident, $value: literal $($tt:tt)*) => {
        $result.push($crate::core::io::templating::FileNameTemplateElement::Static($value));
        file_name_template_element!($result, $($tt)*);
    };
    ($result: ident, dyn @ $value: tt $($tt:tt)*) => {
        $result.push($crate::core::io::templating::FileNameTemplateElement::Dynamic($value.to_string()));
        file_name_template_element!($result, $($tt)*);
    };
    ($result: ident, timestamp $($tt:tt)*) => {
        $result.push($crate::core::io::templating::FileNameTemplateElement::UnixTimestamp);
        file_name_template_element!($result, $($tt)*);
    };
    ($result: ident, timestamp @ $value: tt $($tt:tt)*) => {
        $result.push($crate::core::io::templating::FileNameTemplateElement::formatted_timestamp($value)?);
        file_name_template_element!($result, $($tt)*);
    };
    ($result: ident, serial $($tt:tt)*) => {
        $result.push($crate::core::io::templating::FileNameTemplateElement::Serial);
        file_name_template_element!($result, $($tt)*);
    };
    ($result: ident, template @ $value: tt $($tt:tt)*) => {
        $result.push($crate::core::io::templating::FileNameTemplateElement::FileNameTemplate($value));
        file_name_template_element!($result, $($tt)*);
    };
    ($result: ident, raw @ $value: tt $($tt:tt)*) => {
        $result.push($value);
        file_name_template_element!($result, $($tt)*);
    };
    ($result: ident, sep @ $value:tt $($tt:tt)*) => {
        $result.push($crate::core::io::templating::FileNameTemplateElement::Static(stringify!($value)));
        file_name_template_element!($result, $($tt)*);
    };
    ($result: ident, $value:tt $($tt:tt)*) => {
        $result.push($crate::core::io::templating::FileNameTemplateElement::Static(stringify!($value)));
        file_name_template_element!($result, $($tt)*);
    };
    ($result: ident,) => {}
}

macro_rules! file_name_template {
    ($($tt:tt)+) => {
        {
            let mut result: Vec<$crate::core::io::templating::FileNameTemplateElement> = Vec::new();
            fn __build(result: &mut Vec<$crate::core::io::templating::FileNameTemplateElement>) -> Result<(), time::error::InvalidFormatDescription> {
                file_name_template_element!(result, $($tt)+);
                Ok(())
            }
            match __build(&mut result) {
                Ok(_) => {
                    result.shrink_to_fit();
                    Ok($crate::core::io::templating::FileNameTemplate::new(std::sync::Arc::new(result)))
                }
                Err(err) => {
                    Err(err)
                }
            }
        }
    }
}

pub(crate) use file_name_template;

#[derive(Debug, Clone, Copy)]
pub struct FileNameTemplateWithSerialProvider<'a, 'b, S> {
    template: &'a FileNameTemplate,
    provider: &'b S
}


impl<T> Display for FileNameTemplateWithSerialProvider<'_, '_, T> where T: SerialProvider {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Ok(_) = self.template.write(f, self.provider) {
            Ok(())
        } else {
            Err(std::fmt::Error)
        }
    }
}

/// A template for a filename
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct FileNameTemplate {
    parts: Arc<Vec<FileNameTemplateElement>>
}

impl FileNameTemplate {
    pub fn new(parts: Arc<Vec<FileNameTemplateElement>>) -> Self {
        Self{ parts }
    }

    /// Writes the template element to `f`. Returns true if some kind of content was written.
    pub fn write(&self, f: &mut dyn Write, serial_provider: &impl SerialProvider) -> Result<bool, TemplateError> {
        let mut wrote_something = false;
        for value in self.parts.iter() {
            wrote_something |= value.write(f, serial_provider)?;
        }
        Ok(wrote_something)
    }

    pub fn with_serial_provider<'a, 'b, S: SerialProvider>(&'a self, serial_provider: &'b S) -> FileNameTemplateWithSerialProvider<'a, 'b, S> {
        FileNameTemplateWithSerialProvider {
            template: self,
            provider: serial_provider
        }
    }
}

impl Display for FileNameTemplate {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Ok(_) = self.write(f, &NoSerial::<u8>::default()) {
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




/// Provides a serial
pub trait SerialProvider: Sync + Send {
    type Serial: Display;

    fn provide_serial(&self) -> Option<Self::Serial>;
}

#[derive(Debug, Copy, Clone)]
pub struct NoSerial<S=u8>{
    _phantom: PhantomData<S>
}

unsafe impl<S> Send for NoSerial<S>{}
unsafe impl<S> Sync for NoSerial<S>{}

impl<S> SerialProvider for NoSerial<S> where S: Display {
    type Serial = S;

    #[inline(always)]
    fn provide_serial(&self) -> Option<Self::Serial> {
        None
    }
}

impl<S> Default for NoSerial<S> {
    fn default() -> Self {
        Self{_phantom: PhantomData}
    }
}


#[derive(Debug, Clone, Default)]
pub struct DefaultSerialProvider {
    state: Arc<AtomicU32>,
}

impl DefaultSerialProvider {
    pub fn get_next_serial(&self) -> u32 {
        unsafe {
            self.state.fetch_update(
                Ordering::SeqCst,
                Ordering::Relaxed,
                |next| Some(next.overflowing_add(1).0)
            ).unwrap_unchecked()
        }
    }
}

impl SerialProvider for DefaultSerialProvider {
    type Serial = u32;
    fn provide_serial(&self) -> Option<Self::Serial> {
        Some(self.get_next_serial())
    }
}


#[derive(Debug, Clone)]
pub enum FileNameTemplateElement {
    Static(&'static str),
    Dynamic(String),
    UnixTimestamp,
    FormattedTimestamp(OwnedFormatItem),
    Serial,
    FileNameTemplate(FileNameTemplate)
}

#[derive(Debug, Error)]
pub enum TemplateError {
    #[error(transparent)]
    Time(#[from] time::error::Format),
    #[error(transparent)]
    Fmt(#[from] std::fmt::Error)
}

impl FileNameTemplateElement {
    pub fn formatted_timestamp(format: impl AsRef<str>) -> Result<Self, time::error::InvalidFormatDescription> {
        Ok(Self::FormattedTimestamp(parse_owned::<2>(format.as_ref())?))
    }

    /// Writes the template element to `f`. Returns true if some kind of content was written.
    pub fn write(&self, f: &mut dyn Write, serial_provider: &impl SerialProvider) -> Result<bool, TemplateError> {
        match self {
            FileNameTemplateElement::Static(value) => {
                write!(f, "{}", value)?
            }
            FileNameTemplateElement::Dynamic(value) => {
                write!(f, "{}", value)?
            }
            FileNameTemplateElement::UnixTimestamp => {
                write!(f, "{}", OffsetDateTime::now_utc().unix_timestamp_nanos())?
            }
            FileNameTemplateElement::FormattedTimestamp(value) => {
                write!(f, "{}", OffsetDateTime::now_utc().format(value)?)?

            }
            FileNameTemplateElement::Serial => {
                if let Some(serial) = serial_provider.provide_serial() {
                    write!(f, "{serial}")?
                } else {
                    return Ok(false)
                }
            }
            FileNameTemplateElement::FileNameTemplate(template) => {
                return template.write(f, serial_provider)
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
    use crate::core::io::templating::DefaultSerialProvider;

    #[test]
    fn can_build(){
        let serial_provider = DefaultSerialProvider::default();

        let template = file_name_template!(
            "test" _ "here" _ dyn@123 _ timestamp _ serial ".exe"
        ).expect("This shoudl work!");
        println!("{}", template.with_serial_provider(
            &serial_provider
        ))
    }
}