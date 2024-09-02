pub mod stopwords;
pub mod tokenizer;

use std::collections::HashMap;
use camino::Utf8PathBuf;
use isolang::Language;
use rust_stemmers::Algorithm;
use serde::{Deserialize, Serialize};
use crate::features::tokenizing::stopwords::StopWordListRegistry;

/// The config for the text processing used by other modules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenizersConfig {
    /// The default tokenizer config
    pub default: Option<TokenizerConfig>,
    /// A language specific tokenizer
    pub specific: Option<HashMap<Language, TokenizerConfig>>
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
pub trait StopwordContext {
    fn stopword_registry(&self) -> &StopWordListRegistry;
}