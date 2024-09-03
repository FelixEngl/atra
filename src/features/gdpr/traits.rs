use std::sync::Arc;
use camino::{Utf8Path, Utf8PathBuf};
use isolang::Language;
use liblinear::solver::L2R_L2LOSS_SVR;
use rust_stemmers::Algorithm;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use crate::features::gdpr::classifier::DocumentClassifier;
use crate::features::text_processing::tf_idf::{Idf, Tf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct GdbrRecognizerConfigDev {
    language: Language,
    #[serde(skip_serializing_if = "std::ops::Not::not", rename = "retrain")]
    retrain_if_possible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tf: Option<Tf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    idf: Option<Idf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tf_idf_data: Option<Utf8PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    train_data: Option<Utf8PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    test_data: Option<Utf8PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trained_svm: Option<Utf8PathBuf>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    normalize_tokens: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    filter_stopwords: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    stemmer: Option<Algorithm>,
}


impl From<GdbrRecognizerConfig> for GdbrRecognizerConfigDev {
    fn from(value: GdbrRecognizerConfig) -> Self {
        match value {
            GdbrRecognizerConfig::Load { trained_svm, language, test_data } => {
                Self {
                    language,
                    trained_svm: Some(trained_svm),
                    test_data,
                    ..Default::default()
                }
            }
            GdbrRecognizerConfig::Train {
                language,
                test_data,
                training
            } => {
                Self {
                    language,
                    train_data: Some(training.train_data),
                    test_data,
                    idf: Some(training.idf),
                    tf: Some(training.tf),
                    tf_idf_data: training.tf_idf_data,
                    filter_stopwords: training.filter_stopwords,
                    normalize_tokens: training.normalize_tokens,
                    stemmer: training.stemmer,
                    ..Default::default()
                }
            }
            GdbrRecognizerConfig::All {
                language,
                retrain_if_possible,
                trained_svm,
                test_data,
                training
            } => {
                Self {
                    trained_svm: Some(trained_svm),
                    language,
                    train_data: Some(training.train_data),
                    test_data,
                    idf: Some(training.idf),
                    tf: Some(training.tf),
                    tf_idf_data: training.tf_idf_data,
                    filter_stopwords: training.filter_stopwords,
                    normalize_tokens: training.normalize_tokens,
                    stemmer: training.stemmer,
                    retrain_if_possible
                }
            }
        }
    }
}


#[derive(Debug, Clone)]
pub struct GdbrRecognizerTrainConfig {
    pub tf: Tf,
    pub idf: Idf,
    pub train_data: Utf8PathBuf,
    pub tf_idf_data: Option<Utf8PathBuf>,
    pub normalize_tokens: bool,
    pub filter_stopwords: bool,
    pub stemmer: Option<Algorithm>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "GdbrRecognizerConfigDev", into = "GdbrRecognizerConfigDev")]
pub enum GdbrRecognizerConfig {
    Load {
        language: Language,
        trained_svm: Utf8PathBuf,
        test_data: Option<Utf8PathBuf>,
    },
    Train {
        language: Language,
        test_data: Option<Utf8PathBuf>,
        training: GdbrRecognizerTrainConfig,
    },
    All {
        language: Language,
        retrain_if_possible: bool,
        trained_svm: Utf8PathBuf,
        test_data: Option<Utf8PathBuf>,
        training: GdbrRecognizerTrainConfig
    }
}


impl GdbrRecognizerConfig {

    pub fn language(&self) -> &Language {
        match self {
            GdbrRecognizerConfig::Load { language, .. } => {language}
            GdbrRecognizerConfig::Train { language, .. } => {language}
            GdbrRecognizerConfig::All { language, .. } => {language}
        }
    }


    pub fn training(&self) -> Option<&GdbrRecognizerTrainConfig> {
        match self {
            GdbrRecognizerConfig::Train { training, .. } => {Some(&training)}
            GdbrRecognizerConfig::All { training, .. } => {Some(&training)}
            _ => None
        }
    }

    pub fn test_data(&self) -> Option<&Utf8Path> {
        match self {
            GdbrRecognizerConfig::Train { test_data: Some(test_data), .. } => Some(test_data.as_path()),
            GdbrRecognizerConfig::All { test_data: Some(test_data), .. } => Some(test_data.as_path()),
            GdbrRecognizerConfig::Load {test_data: Some(test_data), ..} => Some(test_data.as_path()),
            _ => None
        }
    }

    pub fn configure_root(self, path: impl AsRef<Utf8Path>) -> Self {
        todo!()
    }
}

#[derive(Debug, Error)]
#[error("Failed to initialize any meningful config with {0:?}")]
struct GdbrRecognizerConfigDevError(GdbrRecognizerConfigDev);

impl TryFrom<GdbrRecognizerConfigDev> for GdbrRecognizerConfig {
    type Error = GdbrRecognizerConfigDevError;

    fn try_from(value: GdbrRecognizerConfigDev) -> Result<Self, Self::Error> {
        match value {
            GdbrRecognizerConfigDev {
                language,
                retrain_if_possible: false,
                trained_svm: Some(trained_svm),
                train_data: None,
                test_data,
                tf: None,
                idf: None,
                tf_idf_data: None,
                filter_stopwords: false,
                normalize_tokens: false,
                stemmer: None
            } => {
                Ok(
                    Self::Load {
                        language,
                        trained_svm,
                        test_data
                    }
                )
            },
            GdbrRecognizerConfigDev {
                language,
                retrain_if_possible: false,
                trained_svm: None,
                train_data: Some(train_data),
                test_data,
                tf: Some(tf),
                idf: Some(idf),
                tf_idf_data,
                filter_stopwords,
                normalize_tokens,
                stemmer
            } => {
                Ok(
                    Self::Train {
                        language,
                        test_data,
                        training: GdbrRecognizerTrainConfig {
                            stemmer,
                            filter_stopwords,
                            normalize_tokens,
                            tf_idf_data,
                            train_data,
                            tf,
                            idf
                        }
                    }
                )
            },
            GdbrRecognizerConfigDev {
                language,
                retrain_if_possible,
                trained_svm: Some(trained_svm),
                train_data: Some(train_data),
                test_data,
                tf: Some(tf),
                idf: Some(idf),
                tf_idf_data,
                filter_stopwords,
                normalize_tokens,
                stemmer
            } => {
                Ok(
                    Self::All {
                        language,
                        test_data,
                        trained_svm,
                        retrain_if_possible,
                        training: GdbrRecognizerTrainConfig {
                            stemmer,
                            filter_stopwords,
                            normalize_tokens,
                            tf_idf_data,
                            train_data,
                            tf,
                            idf
                        }
                    }
                )
            }
            err => Err(GdbrRecognizerConfigDevError(err))
        }
    }
}



pub trait GdbrContext {
    fn get_gdbr_classifier(&self, lang: &isolang::Language) -> Arc<DocumentClassifier<Tf, Idf, L2R_L2LOSS_SVR>>;
}

