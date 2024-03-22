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
use crate::core::extraction::extractor::SubExtractor;
use crate::core::extraction::html::LinkOrigin;

/// Holds information about the used extraction information
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExtractorMeta {
    pub extractor: SubExtractor,
    #[serde(default)]
    pub meta: SubExtractorMeta
}


/// Some kind of metadata for the used extraction method.
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize, Default)]
pub enum SubExtractorMeta {
    Html(LinkOrigin),
    #[default]
    None,
}



/// A trait marking a factory for [ExtractorMeta]
pub trait ExtractorMetaFactory {
    fn create_meta(&self, meta: SubExtractorMeta) -> ExtractorMeta;

    #[inline]
    fn create_empty_meta(&self) -> ExtractorMeta {
        self.create_meta(SubExtractorMeta::None)
    }
}
