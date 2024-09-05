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

use std::borrow::{Cow};
use std::fmt::Debug;
use std::sync::{Arc};
use isolang::Language;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use unicode_segmentation::{UnicodeSegmentation};
use unicode_normalization::{UnicodeNormalization};
use crate::features::tokenizing::stopwords::{ContainsKind, StopWordList};

/// A primitive tokenizer.
#[derive(Debug, Serialize, Deserialize)]
pub struct Tokenizer {
    language: Language,
    normalize: bool,
    stop_words: Option<Arc<StopWordList>>,
    stemmer: Option<rust_stemmers::Algorithm>,
}

impl Tokenizer {

    pub fn new(
        language: Language,
        normalize: bool,
        stop_words: Option<Arc<StopWordList>>,
        stemmer: Option<rust_stemmers::Algorithm>
    ) -> Self {
        Self {
            language,
            normalize,
            stop_words,
            stemmer
        }
    }

    /// Preprocesses a text
    pub fn tokenize(&self, text: &str) -> Vec<String> {
        let text = if self.normalize {
            Cow::Owned(text.nfc().to_string())
        } else {
            Cow::Borrowed(text)
        };

        let text = text.unicode_words();

        let text = if let Some(stop_words) = &self.stop_words {
            let target = if self.normalize {
                ContainsKind::Normalized
            } else {
               ContainsKind::Raw
            };
            text.filter(|value| !stop_words.contains(target, *value)).collect_vec()
        } else {
            text.collect_vec()
        };

        if let Some(stemmer) = self.stemmer {
            let stemmer = rust_stemmers::Stemmer::create(stemmer);
            text.into_iter().map(|value| stemmer.stem(value).to_lowercase()).collect_vec()
        } else {
            text.into_iter().map(|value| value.to_lowercase()).collect_vec()
        }
    }
}

#[cfg(test)]
mod test {
    use isolang::Language;
    use crate::features::tokenizing::stopwords::{StopWordRegistry, StopWordRepository};
    use crate::features::tokenizing::tokenizer::Tokenizer;

    #[test]
    fn can_exec(){
        let mut registry = StopWordRegistry::default();
        registry.register(StopWordRepository::IsoDefault);
        let tokenizer = Tokenizer::new(
            Language::Deu,
            true,
            registry.get_or_load_sync(&Language::from_639_1("de").unwrap()),
            Some(rust_stemmers::Algorithm::German)
        );

        const TEST_TEXT: &str = "Hallo welt was ist Höher, ÅΩ oder `katze\u{30b}hier";

        println!("{TEST_TEXT}\n{:?}", tokenizer.tokenize(TEST_TEXT))
    }
}