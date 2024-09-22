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

mod args;
mod elements;
mod errors;
mod recovery;
mod template;

pub use args::*;
pub use elements::*;
pub use errors::*;
pub use recovery::*;
pub use template::*;

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
    ($result: ident, serial(start=$serial:expr) $($tt:tt)*) => {
        if let Ok(res) = $result.as_mut() {
            res.push($crate::io::templating::FileNameTemplateElement::CustomSerial($crate::io::serial::SerialProvider::with_initial_state($serial)));
        }
        crate::io::templating::file_name_template_element!($result, $($tt)*);
    };
    ($result: ident, serial(kind=$serial:ident) $($tt:tt)*) => {
        if let Ok(res) = $result.as_mut() {
            res.push($crate::io::templating::FileNameTemplateElement::CustomSerial($crate::io::serial::SerialProviderKind::$serial));
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
                    Ok($crate::io::templating::FileNameTemplate::new(result))
                }
                Err(err) => {
                    Err(err)
                }
            }
        }
    }
}

pub(crate) use {file_name_template, file_name_template_element};

#[cfg(test)]
mod test {
    use crate::io::serial::SerialProvider;
    use crate::io::templating::FileNameTemplateArgs;

    #[test]
    fn can_build() {
        let serial_provider = SerialProvider::default();

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
