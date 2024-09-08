pub mod iso_stopwords;
mod repository;

pub use repository::{StopWordRepository, StopWordListRepository};

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

use crate::features::tokenizing::StopwordRegistryConfig;

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
