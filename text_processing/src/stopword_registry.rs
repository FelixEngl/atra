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

use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;
use std::fmt::Debug;
use std::hash::Hash;
use std::io;
use std::ops::Deref;
use std::sync::{Arc, RwLock};
use compact_str::{CompactString, ToCompactString};
use isolang::Language;
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;
use std::convert::TryFrom;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use camino::{Utf8PathBuf};
use itertools::Itertools;
use thiserror::Error;
use iso_stopwords::iso_stopwords_for;

use crate::configs::StopwordRegistryConfig;


/// A registry for stopwords.
/// May have multiple repositories.
/// If multiple repositories are provided the used stopword list is
///     - The first loaded stopword list, if fast is set
///     - a combination of all provided stopword lists from all registered repositories if fast is not net.
#[derive(Debug, Default, Clone)]
pub struct StopWordRegistry {
    cached_stop_words: Arc<RwLock<HashMap<Language, Arc<StopWordList>>>>,
    repositories: Arc<RwLock<Vec<StopWordRepository>>>
}

impl StopWordRegistry {
    pub fn initialize(cfg: &StopwordRegistryConfig) -> Result<Self, io::Error>  {
        let new = Self::default();
        new.repositories.write().unwrap().extend(cfg.to_vec());
        Ok(new)
    }

    pub fn register(&mut self, repository: StopWordRepository) {
        self.repositories.write().unwrap().push(repository)
    }

    fn load_stop_words(&self, language: &Language) -> Option<Vec<String>> {
        let read = self.repositories.read().unwrap();
        let mut collection = Vec::new();
        for repo in read.deref() {
            if let Some(found) = repo.load_raw_stop_words(&language) {
                collection.extend(found)
            }
        }
        (!collection.is_empty()).then_some(collection)
    }

    pub fn get_or_load(&self, language: &Language) -> Option<Arc<StopWordList>> {
        let lock = self.cached_stop_words.read().unwrap();
        if let Some(found) = lock.get(&language).map(|value| value.clone()) {
            return Some(found);
        }
        drop(lock);
        let mut lock = self.cached_stop_words.write().unwrap();
        match lock.entry(language.clone()) {
            Entry::Occupied(value) => {
                Some(value.get().clone())
            }
            Entry::Vacant(value) => {
                let raw = self.load_stop_words(&language)?
                    .into_iter()
                    .map(CompactString::from)
                    .collect();
                Some(value.insert(Arc::new(StopWordList::from_raw(raw))).clone())
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

    pub fn from_raw(raw: HashSet<CompactString>) -> Self {
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





#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
#[serde(try_from = "StopWordRepositoryDev", into = "StopWordRepositoryDev")]
pub enum StopWordRepository {
    IsoDefault,
    DirRepo { with_iso_default: bool, dir: Utf8PathBuf },
    File { with_iso_default: bool, language: Language, file: Utf8PathBuf },
}

#[derive(Debug, Error)]
#[error("Was not able to propery convert the definition to a recognized StopWordRepository definition: {0:?}")]
#[repr(transparent)]
pub struct StopWordRepositoryConversionError(StopWordRepositoryDev);

impl TryFrom<StopWordRepositoryDev> for StopWordRepository {
    type Error = StopWordRepositoryConversionError;

    fn try_from(value: StopWordRepositoryDev) -> Result<Self, Self::Error> {
        match value {
            StopWordRepositoryDev { with_iso_default, dir: Some(dir), file: None, language: None } => {
                Ok(Self::DirRepo {with_iso_default, dir})
            }
            StopWordRepositoryDev { with_iso_default, dir: None, file: Some(file), language: Some(language) } => {
                Ok(Self::File {with_iso_default, file, language})
            }
            StopWordRepositoryDev { with_iso_default: true, dir: None, file: None, language: None } => {
                Ok(Self::IsoDefault)
            }
            err => Err(StopWordRepositoryConversionError(err))
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct StopWordRepositoryDev {
    #[serde(skip_serializing_if = "std::ops::Not::not", rename = "iso_default", default = "_default_with_iso_default")]
    with_iso_default: bool,
    #[serde(skip_serializing_if = "Option::is_none", alias = "directory")]
    dir: Option<Utf8PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<Utf8PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<Language>,
}

fn _default_with_iso_default() -> bool {
    false
}

impl From<StopWordRepository> for StopWordRepositoryDev {
    fn from(value: StopWordRepository) -> Self {
        match value {
            StopWordRepository::IsoDefault => {
                StopWordRepositoryDev {
                    with_iso_default: true,
                    dir: None,
                    ..Default::default()
                }
            }
            StopWordRepository::DirRepo { dir, with_iso_default} => {
                StopWordRepositoryDev {
                    dir: Some(dir),
                    with_iso_default: with_iso_default,
                    ..Default::default()
                }
            }
            StopWordRepository::File { file, language, with_iso_default } => {
                StopWordRepositoryDev {
                    file: Some(file),
                    language: Some(language),
                    with_iso_default: with_iso_default,
                    ..Default::default()
                }
            }
        }
    }
}

/// Provides stop word lists for a specific language
pub trait StopWordListRepository {
    fn load_raw_stop_words(&self, language: &Language) -> Option<Vec<String>>;
}

impl StopWordListRepository for StopWordRepository {
    fn load_raw_stop_words(&self, language: &Language) -> Option<Vec<String>> {
        fn load_file(file: impl AsRef<Path>, with_iso_default: bool, language: &Language) -> Option<Vec<String>> {
            let mut result = BufReader::new(File::open(file).ok()?)
                .lines()
                .collect::<Result<Vec<_>, _>>()
                .ok()?;
            if with_iso_default {
                if let Some(default) = iso_stopwords_for(language) {
                    result.extend(default.into_iter().map(|value| str::to_owned(*value)))
                }
            }
            Some(result)
        }

        fn load_stopwords(language: &Language) -> Option<Vec<String>> {
            Some(iso_stopwords_for(language)?.into_iter().map(|value| str::to_owned(*value)).collect_vec())
        }

        match self {
            StopWordRepository::IsoDefault => {
                load_stopwords(language)
            }
            StopWordRepository::DirRepo { dir, with_iso_default } => {
                if dir.exists() {
                    let file = dir.join(format!("{}.txt", language.to_639_3()));
                    if file.exists() {
                        load_file(file, *with_iso_default, language)
                    } else if let Some(file) = language.to_639_1().map(|value| dir.join(format!("{}.txt", value))).filter(|p| p.exists()) {
                        load_file(file, *with_iso_default, language)
                    } else {
                        log::warn!("The file {} does not exist! Falling back to iso only if selected for the repo!", file);
                        if *with_iso_default {
                            load_stopwords(language)
                        } else {
                            None
                        }
                    }
                } else {
                    log::warn!("The directory {} does not exist! Falling back to iso only if selected for the repo!", dir);
                    if *with_iso_default {
                        load_stopwords(language)
                    } else {
                        None
                    }
                }
            }
            StopWordRepository::File { file, language: file_lang, with_iso_default } => {
                if language != file_lang {
                    None
                } else if file.exists() {
                    load_file(file, *with_iso_default, language)
                } else {
                    log::warn!("The file {} does not exist! Falling back to iso only if selected for the repo!", file);
                    if *with_iso_default {
                        load_stopwords(language)
                    } else {
                        None
                    }
                }
            }
        }
    }
}

