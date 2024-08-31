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

use std::borrow::{Borrow, Cow};
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;
use std::fmt::Debug;
use std::fs::File;
use std::hash::{Hash};
use std::io;
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use camino::{Utf8Path, Utf8PathBuf};
use compact_str::{CompactString, ToCompactString};
use isolang::Language;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use unicode_segmentation::{UnicodeSegmentation};
use unicode_normalization::{UnicodeNormalization};



/// Provides stop word lists for a specific language
pub trait StopWordListRepository: Debug {
    fn load_raw_stop_words(&self, language: Language) -> Option<Vec<String>>;
}

/// A stopword list repository in a directory.
/// The dir contains multiple files containing <name>.txt files
/// where <name> is the name of the language containing the stopwords as ISO 639 1
#[derive(Debug)]
pub struct DirStopWordListRepository {
    dir: Utf8PathBuf
}

impl DirStopWordListRepository {
    pub fn new<P: AsRef<Utf8Path>>(path: P) -> Result<Self, io::Error> {
        let dir = path.as_ref().to_path_buf();
        if dir.exists() && !dir.is_dir() {
            return Err(io::Error::from(io::ErrorKind::Unsupported))
        }
        Ok(Self { dir })
    }
}

impl StopWordListRepository for DirStopWordListRepository {
    fn load_raw_stop_words(&self, language: Language) -> Option<Vec<String>> {
        if !self.dir.exists() {
            return None
        }
        BufReader::new(File::open(self.dir.join(format!("{}.txt", language.to_639_1()?))).ok()?)
            .lines()
            .collect::<Result<Vec<_>, _>>()
            .ok()
    }
}

/// A registry for stopwords.
/// May have multiple repositories.
/// If multiple repositories are provided the used stopword list is
///     - The first loaded stopword list, if fast is set
///     - a combination of all provided stopword lists from all registered repositories if fast is not net.
#[derive(Debug, Default)]
pub struct StopWordListRegistry {
    fast: bool,
    cached_stop_words: tokio::sync::RwLock<HashMap<Language, Arc<StopWordList>>>,
    repositories: Vec<Box<dyn StopWordListRepository>>
}

impl StopWordListRegistry {
    pub fn new(fast: bool) -> Self {
        Self {
            fast,
            ..StopWordListRegistry::default()
        }
    }

    pub fn register<R: StopWordListRepository + 'static>(&mut self, repository: R) {
        self.register_boxed(Box::new(repository))
    }

    pub fn register_boxed(&mut self, repository: Box<dyn StopWordListRepository>) {
        self.repositories.push(repository);
    }

    fn load_stop_words(&self, language: Language) -> Option<Vec<String>> {
        if self.fast || self.repositories.len() == 1 {
            for repo in &self.repositories {
                if let Some(found) = repo.load_raw_stop_words(language) {
                    return Some(found);
                }
            }
        } else {
            let mut collection = Vec::new();
            for repo in &self.repositories {
                if let Some(found) = repo.load_raw_stop_words(language) {
                    collection.extend(found)
                }
            }
            if !collection.is_empty() {
                return Some(collection)
            }
        }

        return None;
    }

    #[cfg(test)]
    pub fn get_or_load_sync(&self, language: Language) -> Option<Arc<StopWordList>> {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(
                async move {
                    self.get_or_load(language).await
                }
            )
    }

    pub async fn get_or_load(&self, language: Language) -> Option<Arc<StopWordList>> {
        let lock = self.cached_stop_words.read().await;
        if let Some(found) = lock.get(&language).map(|value| value.clone()) {
            return Some(found);
        }
        drop(lock);
        let mut lock = self.cached_stop_words.write().await;
        match lock.entry(language) {
            Entry::Occupied(value) => {
                Some(value.get().clone())
            }
            Entry::Vacant(value) => {
                Some(value.insert(Arc::new(StopWordList::from(
                    self.load_stop_words(language)?
                        .into_iter()
                        .map(CompactString::from_string_buffer)
                        .collect()
                ))).clone())
            }
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopWordList {
    raw: HashSet<CompactString>,
    normalized: HashSet<CompactString>
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum ContainsKind {
    Raw,
    Normalized,
    Both
}

impl StopWordList {

    pub fn new(mut raw: HashSet<CompactString>, mut normalized: HashSet<CompactString>) -> Self {
        raw.shrink_to_fit();
        normalized.shrink_to_fit();
        Self { raw, normalized }
    }

    pub fn from(raw: HashSet<CompactString>) -> Self {
        let normalized = raw
            .iter()
            .map(|value| value.nfc().collect::<CompactString>())
            .collect::<HashSet<_>>();
        Self::new(raw, normalized)
    }

    pub fn extend_with(&mut self, other: Self) {
        self.raw.extend(other.raw);
        self.normalized.extend(other.normalized);
        self.raw.shrink_to_fit();
        self.normalized.shrink_to_fit();
    }

    #[inline]
    pub fn contains<Q: ?Sized>(&self, kind: ContainsKind, value: &Q) -> bool
        where
            CompactString: Borrow<Q>,
            Q: Hash + Eq, {
        match kind {
            ContainsKind::Raw => {self.contains_raw(value)}
            ContainsKind::Normalized => {self.contains_normalized(value)}
            ContainsKind::Both => {self.contains_both(value)}
        }
    }

    #[inline]
    pub fn contains_both<Q: ?Sized>(&self, value: &Q) -> bool
        where
            CompactString: Borrow<Q>,
            Q: Hash + Eq, {
        self.contains_raw(value) || self.contains_normalized(value)
    }

    #[inline]
    pub fn contains_raw<Q: ?Sized>(&self, value: &Q) -> bool
        where
            CompactString: Borrow<Q>,
            Q: Hash + Eq, {
        self.raw.contains(value)
    }

    #[inline]
    pub fn contains_normalized<Q: ?Sized>(&self, value: &Q) -> bool
        where
            CompactString: Borrow<Q>,
            Q: Hash + Eq, {
        self.normalized.contains(value)
    }
}

impl<Q> Extend<Q> for StopWordList where Q: ToCompactString {
    fn extend<T: IntoIterator<Item=Q>>(&mut self, iter: T) {
        for value in iter.into_iter() {
            let word = value.to_compact_string();
            let normalized = word.nfc().to_compact_string();
            self.raw.insert(word);
            self.normalized.insert(normalized);
        }
        self.raw.shrink_to_fit();
        self.normalized.shrink_to_fit();
    }
}


#[derive(Debug, Serialize, Deserialize)]
pub struct Tokenizer {
    normalize: bool,
    stop_words: Option<Arc<StopWordList>>,
    stemmer: Option<rust_stemmers::Algorithm>,
}

impl Tokenizer {

    pub fn new(
        normalize: bool,
        stop_words: Option<Arc<StopWordList>>,
        stemmer: Option<rust_stemmers::Algorithm>
    ) -> Self {
        Self {
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
    use crate::features::text_processing::text_preprocessor::{DirStopWordListRepository, StopWordListRegistry, Tokenizer};

    #[test]
    fn can_exec(){
        let mut registry = StopWordListRegistry::new(false);
        registry.register(DirStopWordListRepository::new("./data/stopwords/iso").expect("Should exist!"));
        let tokenizer = Tokenizer::new(
            true,
            registry.get_or_load_sync(Language::from_639_1("de").unwrap()),
            Some(rust_stemmers::Algorithm::German)
        );

        const TEST_TEXT: &str = "Hallo welt was ist Höher, ÅΩ oder `katze\u{30b}hier";

        println!("{TEST_TEXT}\n{:?}", tokenizer.tokenize(TEST_TEXT))
    }
}