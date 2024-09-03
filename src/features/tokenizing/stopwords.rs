pub mod iso_stopwords;
mod repository;

pub use repository::{StopWordRepository, StopWordRepositoryConversionError, StopWordListRepository};

use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;
use std::fmt::Debug;
use std::hash::Hash;
use std::io;
use std::io::BufRead;
use std::sync::Arc;

use compact_str::{CompactString, ToCompactString};
use isolang::Language;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::core::config::Configs;



/// A registry for stopwords.
/// May have multiple repositories.
/// If multiple repositories are provided the used stopword list is
///     - The first loaded stopword list, if fast is set
///     - a combination of all provided stopword lists from all registered repositories if fast is not net.
#[derive(Debug, Default)]
pub struct StopWordRegistry {
    cached_stop_words: tokio::sync::RwLock<HashMap<Language, Arc<StopWordList>>>,
    repositories: Vec<StopWordRepository>
}

unsafe impl Send for StopWordRegistry {}
unsafe impl Sync for StopWordRegistry {}

impl StopWordRegistry {
    pub fn initialize(cfg: &Configs) -> Result<Self, io::Error>  {
        let mut new = Self::default();
        new.repositories = cfg.paths.stopword_registry_config().registries;
        Ok(new)
    }

    pub fn register(&mut self, repository: StopWordRepository) {
        self.repositories.push(repository)
    }

    fn load_stop_words(&self, language: &Language) -> Option<Vec<String>> {
        let mut collection = Vec::new();
        for repo in &self.repositories {
            if let Some(found) = repo.load_raw_stop_words(&language) {
                collection.extend(found)
            }
        }
        return (!collection.is_empty()).then_some(collection);
    }

    #[cfg(test)]
    pub fn get_or_load_sync(&self, language: &Language) -> Option<Arc<StopWordList>> {
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

    pub async fn get_or_load(&self, language: &Language) -> Option<Arc<StopWordList>> {
        let lock = self.cached_stop_words.read().await;
        if let Some(found) = lock.get(&language).map(|value| value.clone()) {
            return Some(found);
        }
        drop(lock);
        let mut lock = self.cached_stop_words.write().await;
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



/// Retrieves the default stopwords for a provided [lang] in iso3 format.
pub fn get_default_stopwords_for(lang: &str) -> Option<&'static [&'static str]>{
    todo!()
}


/// Retrieves the default stopwords for a provided [lang].
pub fn get_default_stopwords_for_lang(lang: &isolang::Language) -> Option<&'static [&'static str]>{
    get_default_stopwords_for(lang.to_639_3())
}
