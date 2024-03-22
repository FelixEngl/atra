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

use std::borrow::Cow;
use std::collections::HashMap;
use compact_str::CompactString;
use unicode_segmentation::{UnicodeSegmentation};
use unicode_normalization::{UnicodeNormalization};


pub struct GlobalTokenizer {
    stop_word_filter: tokio::sync::RwLock<HashMap<String, Vec<CompactString>>>,
}

/// Preprocesses a text
pub fn tokenize(text: &str, normalize: bool, stop_words: Option<Vec<String>>, stemming: Option<rust_stemmers::Algorithm>) {
    let text = if normalize {
        Cow::Owned(text.nfc().to_string())
    } else {
        Cow::Borrowed(text)
    };

    let words = text.unicode_words();

    // if let Some(language) = stop_word_removal {
    //     stop_words::get()
    // }



    if let Some(stemmer) = stemming {
        let stemmer = rust_stemmers::Stemmer::create(stemmer);
        let stemmed = words.map(|value| stemmer.stem(value));
    } else {

    }
}