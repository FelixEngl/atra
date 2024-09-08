pub mod stopwords;
pub mod tokenizer;

use std::ops::Deref;
use isolang::Language;
use rust_stemmers::Algorithm;
use serde::{Deserialize, Serialize};
use crate::features::tokenizing::stopwords::StopWordRepository;
use crate::features::tokenizing::stopwords::StopWordRegistry;

/// The config for a stopword registry
#[derive(Debug, Clone, Serialize, Deserialize, Eq, Default)]
#[serde(transparent)]
pub struct StopwordRegistryConfig {
    pub(super) registries: Vec<StopWordRepository>
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

/// The context needed for tokenizing to work
pub trait SupportsStopwords {
    fn stopword_registry(&self) -> Option<&StopWordRegistry>;
}

impl SupportsStopwords for Option<StopWordRegistry> {
    fn stopword_registry(&self) -> Option<&StopWordRegistry> {
        self.as_ref()
    }
}

impl SupportsStopwords for Option<&StopWordRegistry> {
    fn stopword_registry(&self) -> Option<&StopWordRegistry> {
        self.clone()
    }
}