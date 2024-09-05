use std::fmt::Debug;
use std::num::NonZeroUsize;
use std::sync::Arc;
use camino::{Utf8Path, Utf8PathBuf};
use isolang::Language;
use liblinear::parameter::serde::GenericParameters;
use liblinear::solver::L2R_L2LOSS_SVR;
use moka::ops::compute::Op;
use rust_stemmers::Algorithm;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use thiserror::Error;
use crate::features::svm::classifier::DocumentClassifier;
use crate::features::text_processing::tf_idf::{Idf, IdfAlgorithm, Tf, TfAlgorithm};



#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
struct SvmRecognizerConfigSer<TF: TfAlgorithm, IDF: IdfAlgorithm> {
    language: Language,
    #[serde(skip_serializing_if = "std::ops::Not::not", rename = "retrain")]
    retrain_if_possible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tf: Option<TF>,
    #[serde(skip_serializing_if = "Option::is_none")]
    idf: Option<IDF>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<GenericParameters>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_doc_length: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_vector_length: Option<usize>
}


impl<TF, IDF> Default for SvmRecognizerConfigSer<TF, IDF> where TF: TfAlgorithm, IDF: IdfAlgorithm {
    fn default() -> Self {
        Self {
            language: Default::default(),
            retrain_if_possible: Default::default(),
            tf: None,
            idf: None,
            tf_idf_data: Default::default(),
            train_data: Default::default(),
            test_data: Default::default(),
            trained_svm: Default::default(),
            normalize_tokens: Default::default(),
            filter_stopwords: Default::default(),
            stemmer: Default::default(),
            parameters: Default::default(),
            min_doc_length: Default::default(),
            min_vector_length: Default::default(),
        }
    }
}

impl<TF, IDF> Clone for SvmRecognizerConfigSer<TF, IDF> where TF: TfAlgorithm + Clone, IDF: IdfAlgorithm + Clone {
    fn clone(&self) -> Self {
        Self {
            language: self.language.clone(),
            retrain_if_possible: self.retrain_if_possible.clone(),
            tf: self.tf.clone(),
            idf: self.idf.clone(),
            tf_idf_data: self.tf_idf_data.clone(),
            train_data: self.train_data.clone(),
            test_data: self.test_data.clone(),
            trained_svm: self.trained_svm.clone(),
            normalize_tokens: self.normalize_tokens.clone(),
            filter_stopwords: self.filter_stopwords.clone(),
            stemmer: self.stemmer.clone(),
            parameters: self.parameters.clone(),
            min_doc_length: self.min_doc_length.clone(),
            min_vector_length: self.min_vector_length.clone()
        }
    }
}

impl<TF, IDF> From<SvmRecognizerConfig<TF, IDF>> for SvmRecognizerConfigSer<TF, IDF>
where
    TF: TfAlgorithm + Debug,
    IDF: IdfAlgorithm + Debug
{
    fn from(value: SvmRecognizerConfig<TF, IDF>) -> Self {
        match value {
            SvmRecognizerConfig::Load {
                trained_svm,
                language,
                test_data,
                min_doc_length,
                min_vector_length
            } => {
                Self {
                    language,
                    test_data,
                    trained_svm: Some(trained_svm),
                    min_doc_length,
                    min_vector_length,
                    ..Default::default()
                }
            }
            SvmRecognizerConfig::Train {
                language,
                test_data,
                classifier: training
            } => {
                Self {
                    language,
                    test_data,
                    train_data: Some(training.train_data),
                    idf: Some(training.idf),
                    tf: Some(training.tf),
                    tf_idf_data: training.tf_idf_data,
                    filter_stopwords: training.filter_stopwords,
                    normalize_tokens: training.normalize_tokens,
                    stemmer: training.stemmer,
                    parameters: training.parameters,
                    min_doc_length: (training.min_doc_length != 0).then_some(training.min_doc_length),
                    min_vector_length: (training.min_vector_length != 0).then_some(training.min_vector_length),
                    ..Default::default()
                }
            }
            SvmRecognizerConfig::All {
                language,
                retrain_if_possible,
                trained_svm,
                test_data,
                classifier: training,
                min_doc_length,
                min_vector_length
            } => {
                Self {
                    language,
                    test_data,
                    trained_svm: Some(trained_svm),
                    retrain_if_possible,
                    train_data: Some(training.train_data),
                    idf: Some(training.idf),
                    tf: Some(training.tf),
                    tf_idf_data: training.tf_idf_data,
                    filter_stopwords: training.filter_stopwords,
                    normalize_tokens: training.normalize_tokens,
                    stemmer: training.stemmer,
                    parameters: training.parameters,
                    min_doc_length,
                    min_vector_length,
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvmParameterConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) epsilon: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cost: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) nu: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cost_penalty: Option<Vec<(i32, f64)>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) initial_solutions: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bias: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) regularize_bias: Option<bool>,
}



#[derive(Debug, Clone)]
pub struct DocumentClassifierConfig<TF = Tf, IDF = Idf> where TF: TfAlgorithm, IDF: IdfAlgorithm {
    pub tf: TF,
    pub idf: IDF,
    pub train_data: Utf8PathBuf,
    pub tf_idf_data: Option<Utf8PathBuf>,
    pub normalize_tokens: bool,
    pub filter_stopwords: bool,
    pub stemmer: Option<Algorithm>,
    pub parameters: Option<GenericParameters>,
    pub min_doc_length: usize,
    pub min_vector_length: usize
}

impl<TF, IDF> DocumentClassifierConfig<TF, IDF> where TF: TfAlgorithm, IDF: IdfAlgorithm {
    pub fn new(
        tf: TF,
        idf: IDF,
        train_data: Utf8PathBuf,
        tf_idf_data: Option<Utf8PathBuf>,
        normalize_tokens: bool,
        filter_stopwords: bool,
        stemmer: Option<Algorithm>,
        parameters: Option<GenericParameters>,
        min_doc_length: usize,
        min_vector_length: usize
    ) -> Self {
        Self {
            tf,
            idf,
            train_data,
            tf_idf_data,
            normalize_tokens,
            filter_stopwords,
            stemmer,
            parameters,
            min_doc_length,
            min_vector_length
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound(
    serialize = "TF: Clone + Serialize, IDF: Clone + Serialize",
    deserialize = "TF: Clone + DeserializeOwned + Debug, IDF: Clone + DeserializeOwned + Debug"
))]
#[serde(try_from = "SvmRecognizerConfigSer<TF, IDF>", into = "SvmRecognizerConfigSer<TF, IDF>")]
pub enum SvmRecognizerConfig<TF = Tf, IDF = Idf> where TF: TfAlgorithm + Debug, IDF: IdfAlgorithm + Debug {
    Load {
        language: Language,
        trained_svm: Utf8PathBuf,
        test_data: Option<Utf8PathBuf>,
        min_doc_length: Option<usize>,
        min_vector_length: Option<usize>
    },
    Train {
        language: Language,
        test_data: Option<Utf8PathBuf>,
        classifier: DocumentClassifierConfig<TF, IDF>,
    },
    All {
        language: Language,
        retrain_if_possible: bool,
        trained_svm: Utf8PathBuf,
        test_data: Option<Utf8PathBuf>,
        classifier: DocumentClassifierConfig<TF, IDF>,
        min_doc_length: Option<usize>,
        min_vector_length: Option<usize>
    }
}


impl<TF, IDF> SvmRecognizerConfig<TF, IDF>
where
    TF: TfAlgorithm + Debug,
    IDF: IdfAlgorithm + Debug
{

    pub fn language(&self) -> &Language {
        match self {
            SvmRecognizerConfig::Load { language, .. } => {language}
            SvmRecognizerConfig::Train { language, .. } => {language}
            SvmRecognizerConfig::All { language, .. } => {language}
        }
    }


    pub fn training(&self) -> Option<&DocumentClassifierConfig<TF, IDF>> {
        match self {
            SvmRecognizerConfig::Train { classifier: training, .. } => {Some(&training)}
            SvmRecognizerConfig::All { classifier: training, .. } => {Some(&training)}
            _ => None
        }
    }

    pub fn test_data(&self) -> Option<&Utf8Path> {
        match self {
            SvmRecognizerConfig::Train { test_data: Some(test_data), .. } => Some(test_data.as_path()),
            SvmRecognizerConfig::All { test_data: Some(test_data), .. } => Some(test_data.as_path()),
            SvmRecognizerConfig::Load {test_data: Some(test_data), ..} => Some(test_data.as_path()),
            _ => None
        }
    }
}

#[derive(Debug, Error)]
#[error("Failed to initialize any meningful config with {0:?}")]
struct SvmRecognizerConfigSerError<TF: TfAlgorithm + Debug, IDF: IdfAlgorithm + Debug>(SvmRecognizerConfigSer<TF, IDF>);

impl<TF, IDF> TryFrom<SvmRecognizerConfigSer<TF, IDF>> for SvmRecognizerConfig<TF, IDF>  where TF: TfAlgorithm + Debug, IDF: IdfAlgorithm + Debug {
    type Error = SvmRecognizerConfigSerError<TF, IDF>;

    fn try_from(value: SvmRecognizerConfigSer<TF, IDF>) -> Result<Self, Self::Error> {
        match value {
            SvmRecognizerConfigSer {
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
                stemmer: None,
                parameters: None,
                min_vector_length,
                min_doc_length
            } => {
                Ok(
                    Self::Load {
                        language,
                        trained_svm,
                        test_data,
                        min_vector_length,
                        min_doc_length
                    }
                )
            },
            SvmRecognizerConfigSer {
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
                stemmer,
                parameters,
                min_vector_length,
                min_doc_length
            } => {
                Ok(
                    Self::Train {
                        language,
                        test_data,
                        classifier: DocumentClassifierConfig {
                            stemmer,
                            filter_stopwords,
                            normalize_tokens,
                            tf_idf_data,
                            train_data,
                            tf,
                            idf,
                            parameters,
                            min_vector_length: min_vector_length.unwrap_or_default(),
                            min_doc_length: min_doc_length.unwrap_or_default()
                        }
                    }
                )
            },
            SvmRecognizerConfigSer {
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
                stemmer,
                parameters,
                min_vector_length,
                min_doc_length
            } => {
                Ok(
                    Self::All {
                        language,
                        test_data,
                        trained_svm,
                        retrain_if_possible,
                        classifier: DocumentClassifierConfig {
                            stemmer,
                            filter_stopwords,
                            normalize_tokens,
                            tf_idf_data,
                            train_data,
                            tf,
                            idf,
                            parameters,
                            min_vector_length: min_vector_length.clone().unwrap_or_default(),
                            min_doc_length: min_doc_length.clone().unwrap_or_default(),
                        },
                        min_vector_length,
                        min_doc_length
                    }
                )
            }
            err => Err(SvmRecognizerConfigSerError(err))
        }
    }
}



pub trait GdbrContext {
    fn get_gdbr_classifier(&self, lang: &isolang::Language) -> Arc<DocumentClassifier<Tf, Idf, L2R_L2LOSS_SVR>>;
}

