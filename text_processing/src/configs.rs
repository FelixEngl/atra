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

use std::ops::Deref;
use isolang::Language;
use rust_stemmers::Algorithm;
use serde::{Deserialize, Serialize};
use crate::stopword_registry::StopWordRepository;

/// The config for a stopword registry
#[derive(Debug, Clone, Serialize, Deserialize, Eq, Default)]
#[serde(transparent)]
pub struct StopwordRegistryConfig {
    pub registries: Vec<StopWordRepository>
}

impl PartialEq for StopwordRegistryConfig {
    fn eq(&self, other: &Self) -> bool {
        self.registries.len() == other.registries.len()
            && self.registries.iter().all(|value| other.registries.contains(value))
    }
}

impl Deref for StopwordRegistryConfig {
    type Target = [StopWordRepository];

    fn deref(&self) -> &Self::Target {
        &self.registries
    }
}


/// The config for the text processing used by other modules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenizerConfig {
    /// If set to true the text is normalized
    pub normalize_text: bool,
    pub stopword_language: Option<Language>,
    pub stemmer: Option<Algorithm>
}
