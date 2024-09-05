use std::convert::TryFrom;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use camino::{Utf8Path, Utf8PathBuf};
use isolang::Language;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use crate::features::tokenizing::stopwords::iso_stopwords::iso_stopwords_for;

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
    #[serde(skip_serializing_if = "std::ops::Not::not", rename = "iso_default")]
    with_iso_default: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    dir: Option<Utf8PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<Utf8PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<Language>,
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

