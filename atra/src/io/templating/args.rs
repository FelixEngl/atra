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

use std::borrow::{Borrow, Cow};
use std::collections::HashMap;
use std::hash::Hash;

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
