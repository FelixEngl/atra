//Copyright 2024 Felix Engl
//
//Licensed under the Apache License, Version 2.0 (the "License");
//you may not use this file except in compliance with the License.
//You may obtain a copy of the License at
//
//    http://www.apache.org/licenses/LICENSE-2.0
//
//Unless required by applicable law or agreed to in writing, software
//distributed under the License is distributed on an "AS IS" BASIS,
//WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//See the License for the specific language governing permissions and
//limitations under the License.

use std::fmt::{Debug};
use std::str::FromStr;
use ini::{Ini, SectionSetter};


pub trait IniExt {
    /// Returns None if the value is not found or can not be parsed
    fn get<T: FromStr>(&self, section: Option<impl Into<String> + Clone>, field_name: &str) -> Option<T> where T::Err: Debug;

    /// Returns the value or alt
    fn get_or<T: FromStr>(&self, section: Option<impl Into<String> + Clone>, field_name: &str, alt: T) -> T where T::Err: Debug {
        self.get_or_else(section, field_name, || alt)
    }

    /// Returns the value or default
    fn get_or_default<T: FromStr + Default>(&self, section: Option<impl Into<String> + Clone>, field_name: &str) -> T where T::Err: Debug {
        self.get_or_else(section, field_name, T::default)
    }

    /// Returns the value or alt
    fn get_or_else<T: FromStr, F: FnOnce() -> T>(&self, section: Option<impl Into<String> + Clone>, field_name: &str, alt: F) -> T where T::Err: Debug;

    /// Returns None if not in config, otherwise returns
    fn get_optional_or<T: FromStr>(&self, section: Option<impl Into<String> + Clone>, field_name: &str, alt: T) -> Option<T> where T::Err: Debug {
        self.get_optional_or_else(section, field_name, || alt)
    }

    fn get_optional_or_default<T: FromStr + Default>(&self, section: Option<impl Into<String> + Clone>, field_name: &str) -> Option<T> where T::Err: Debug {
        self.get_optional_or_else(section, field_name, T::default)
    }

    fn get_optional_or_else<T: FromStr, F: FnOnce() -> T>(&self, section: Option<impl Into<String> + Clone>, field_name: &str, alt: F) -> Option<T> where T::Err: Debug;

}

impl IniExt for Ini {
    fn get<T: FromStr>(&self, section: Option<impl Into<String> + Clone>, field_name: &str) -> Option<T> where T::Err: Debug {
        let value = self.get_from(section.clone(), field_name)?;
        match T::from_str(value) {
            Ok(value) => { Some(value) }
            Err(err) => {
                if let Some(section) = section {
                    log::error!("Failed to parse the value for {field_name} at section {}: {err:?}", section.into())
                } else {
                    log::error!("Failed to parse the value for {field_name}: {err:?}")
                }
                None
            }
        }
    }

    fn get_or_else<T: FromStr, F: FnOnce() -> T>(&self, section: Option<impl Into<String> + Clone>, field_name: &str, alt: F) -> T where T::Err: Debug {
        match self.get_from(section.clone(), field_name) {
            None => { alt() }
            Some(value) => {
                match T::from_str(value) {
                    Ok(value) => { value }
                    Err(err) => {
                        if let Some(section) = section {
                            log::error!("Failed to parse the value for {field_name} at section {}: {err:?}", section.into())
                        } else {
                            log::error!("Failed to parse the value for {field_name}: {err:?}")
                        }
                        alt()
                    }
                }
            }
        }
    }



    fn get_optional_or_else<T: FromStr, F: FnOnce() -> T>(&self, section: Option<impl Into<String> + Clone>, field_name: &str, alt: F) -> Option<T> where T::Err: Debug {
        let value = self.get_from(section.clone(), field_name)?;
        match T::from_str(value) {
            Ok(value) => { Some(value) }
            Err(err) => {
                if let Some(section) = section {
                    log::error!("Failed to parse the value for {field_name} at section {}: {err:?}", section.into())
                } else {
                    log::error!("Failed to parse the value for {field_name}: {err:?}")
                }
                Some(alt())
            }
        }
    }
}


/// Extensions for a section setter
pub trait SectionSetterExt<'a> {

    /// Sets the value in a mapping manner
    fn set_mapping<T, R: Into<String>, M: FnOnce(T) -> R>(&'a mut self, key: &str, value: T, mapping: M) -> &'a mut SectionSetter<'a>;

    /// Sets an optional value if provided
    fn set_optional(&'a mut self, key: &str, value: Option<impl Into<String>>) -> &'a mut SectionSetter<'a>;

    /// The the optional value or [alt]
    #[inline]
    fn set_optional_or<T: Into<String>>(&'a mut self, key: &str, value: Option<T>, alt: T) -> &'a mut SectionSetter<'a> {
        self.set_optional_or_else(key, value, || alt)
    }

    /// Sets the optional value or default
    #[inline]
    fn set_optional_default<T: Into<String> + Default>(&'a mut self, key: &str, value: Option<T>) -> &'a mut SectionSetter<'a> {
        self.set_optional_or_else(key, value, T::default)
    }

    /// Sets the optional value or the result of [alt]
    fn set_optional_or_else<T: Into<String>, F: FnOnce() -> T>(&'a mut self, key: &str, value: Option<T>, alt: F) -> &'a mut SectionSetter<'a>;

    /// Sets the optional but maps the value beforehand
    fn set_optional_mapping<T, R: Into<String>, M: FnOnce(T) -> R>(&'a mut self, key: &str, value: Option<T>, mapping: M) -> &'a mut SectionSetter<'a>;
}

impl<'a> SectionSetterExt<'a> for SectionSetter<'a> {
    #[inline]
    fn set_mapping<T, R: Into<String>, M: FnOnce(T) -> R>(&'a mut self, key: &str, value: T, mapping: M) -> &'a mut SectionSetter<'a> {
        self.set(key, mapping(value))
    }

    fn set_optional(&'a mut self, key: &str, value: Option<impl Into<String>>) -> &'a mut SectionSetter<'a> {
        match value {
            None => {self}
            Some(value) => {
                self.set(key, value)
            }
        }
    }

    fn set_optional_or_else<T: Into<String>, F: FnOnce() -> T>(&'a mut self, key: &str, value: Option<T>, alt: F) -> &'a mut SectionSetter<'a> {
        match value {
            None => {
                self.set(key, alt())
            }
            Some(value) => {
                self.set(key, value)
            }
        }
    }

    fn set_optional_mapping<T, R: Into<String>, M: FnOnce(T) -> R>(&'a mut self, key: &str, value: Option<T>, mapping: M) -> &'a mut SectionSetter<'a> {
        match value {
            None => {self}
            Some(value) => {self.set(key, mapping(value))}
        }
    }
}


pub trait FromIni {
    fn from_ini(ini: &Ini) -> Self;
}

pub trait IntoIni {
    fn insert_into(&self, ini: &mut Ini);

    fn to_ini(self) -> Ini where Self: Sized {
        let mut init = Ini::new();
        self.insert_into(&mut init);
        init
    }
}