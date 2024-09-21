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

use psl::Domain;
use serde::{Deserialize, Deserializer, Serialize};
use std::borrow::Borrow;
use std::convert::Infallible;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::str::FromStr;

/// Basically a case insensitive string like the package case_insensitive_string
/// but it assures the lowercase by converting every stringlike to a lowercase representation
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Hash, Default)]
#[repr(transparent)]
#[serde(transparent)]
pub struct CaseInsensitiveString {
    inner: String,
}

impl CaseInsensitiveString {
    #[inline]
    pub fn new<S>(value: S) -> Self
    where
        S: AsRef<str>,
    {
        Self {
            inner: value.as_ref().to_lowercase(),
        }
    }
}

impl<T> From<T> for CaseInsensitiveString
where
    T: ToCaseInsensitive,
{
    #[inline]
    fn from(value: T) -> Self {
        value.to_case_insensitive()
    }
}

impl CaseInsensitiveString {
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.inner.as_bytes()
    }
}

impl Deref for CaseInsensitiveString {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl AsRef<str> for CaseInsensitiveString {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.inner
    }
}

impl AsRef<[u8]> for CaseInsensitiveString {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.inner.as_bytes()
    }
}

impl Borrow<str> for CaseInsensitiveString {
    #[inline]
    fn borrow(&self) -> &str {
        self.deref()
    }
}

impl Borrow<[u8]> for CaseInsensitiveString {
    #[inline]
    fn borrow(&self) -> &[u8] {
        self.as_ref()
    }
}

impl FromStr for CaseInsensitiveString {
    type Err = Infallible;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(s))
    }
}

impl Display for CaseInsensitiveString {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl<'de> Deserialize<'de> for CaseInsensitiveString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let result = String::deserialize(deserializer)?;
        Ok(Self::new(result))
    }
}

pub trait ToCaseInsensitive {
    fn to_case_insensitive(&self) -> CaseInsensitiveString;
}

impl ToCaseInsensitive for String {
    fn to_case_insensitive(&self) -> CaseInsensitiveString {
        CaseInsensitiveString::new(self)
    }
}

impl<'a> ToCaseInsensitive for &'a str {
    fn to_case_insensitive(&self) -> CaseInsensitiveString {
        CaseInsensitiveString::new(*self)
    }
}

impl<'a> ToCaseInsensitive for &'a String {
    fn to_case_insensitive(&self) -> CaseInsensitiveString {
        CaseInsensitiveString::new(*self)
    }
}

impl<'a> ToCaseInsensitive for &'a [u8] {
    fn to_case_insensitive(&self) -> CaseInsensitiveString {
        CaseInsensitiveString::new(String::from_utf8_lossy(self))
    }
}

impl<'a> ToCaseInsensitive for Domain<'a> {
    fn to_case_insensitive(&self) -> CaseInsensitiveString {
        self.as_bytes().to_case_insensitive()
    }
}
