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

use serde::{Deserialize, Serialize};
use crate::core::extraction::extractor_method::ExtractorMethod;
use crate::core::extraction::html::LinkOrigin;

/// Holds information about the used extraction information
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExtractorMethodHint {
    pub used_method: ExtractorMethod,
    #[serde(default)]
    pub meta: Option<ExtractorMethodMeta>
}


impl ExtractorMethodHint {
    pub fn new(used_method: ExtractorMethod, meta: Option<ExtractorMethodMeta>) -> Self {
        Self { used_method, meta }
    }

    pub fn new_with_meta(used_method: ExtractorMethod, meta: ExtractorMethodMeta) -> Self {
        Self::new(used_method, Some(meta))
    }

    pub fn new_without_meta(used_method: ExtractorMethod) -> Self {
        Self::new(used_method, None)
    }
}

/// Some kind of metadata for the used extraction method.
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum ExtractorMethodMeta {
    Html(LinkOrigin)
}

pub trait ExtractorMethodMetaFactory {
    fn new_without_meta(&self) -> ExtractorMethodHint;
    fn new_with_meta(&self, meta: ExtractorMethodMeta) -> ExtractorMethodHint;
}