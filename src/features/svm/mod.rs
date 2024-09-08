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
use std::fmt::Debug;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, ErrorKind, Read};
use std::sync::Arc;
use camino::{Utf8Path};
use isolang::Language;
use liblinear::{Model};
use liblinear::parameter::serde::{GenericParameters, SupportsParametersCreation};
use liblinear::solver::{GenericSolver};
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use crate::core::io::root::RootSetter;
use crate::features::csv::CsvProvider;
use crate::features::svm::classifier::{DocumentClassifier};
use crate::features::svm::error::{LibLinearError, SvmCreationError};
use crate::features::svm::config::{SvmRecognizerConfig, DocumentClassifierConfig};
use crate::features::text_processing::tf_idf::{IdfAlgorithm, TfAlgorithm, TfIdf};
use crate::features::tokenizing::SupportsStopwords;
use crate::features::tokenizing::stopwords::StopWordList;
use crate::features::tokenizing::tokenizer::Tokenizer;

pub mod classifier;
pub mod error;
pub mod config;

/// An entry of a train csv
#[derive(Debug, Deserialize)]
pub struct TrainModelEntry {
    #[serde(alias = "is_gdbr")]
    pub is_class: bool,
    pub text: String
}

/// gdbr - L2R_L2LOSS_SVR
pub fn create_document_classifier<C, TF, IDF, SOLVER>(
    cfg: &SvmRecognizerConfig<TF, IDF>,
    context: &C,
    root_setter: Option<&impl RootSetter>
) -> Result<DocumentClassifier<TF, IDF, SOLVER>, SvmCreationError<IDF>> where
    TF: TfAlgorithm + Serialize + DeserializeOwned + Clone + Debug,
    IDF: IdfAlgorithm + Serialize + DeserializeOwned + Clone + Debug,
    SOLVER: SupportsParametersCreation,
    Model<SOLVER>: TryFrom<Model<GenericSolver>>,
    C: SupportsStopwords, {
    let model = match &cfg {
        SvmRecognizerConfig::Load {
            trained_svm,
            min_doc_length,
            min_vector_length,
            ..
        } => {
            let trained_svm = if let Some(root_setter) = root_setter {
                Cow::Owned(root_setter.set_root_if_not_exists(trained_svm))
            } else {
                Cow::Borrowed(trained_svm)
            };
            let mut outp = BufReader::new(File::options().read(true).open(trained_svm.as_path())?);
            let mut recognizer: DocumentClassifier<TF, IDF, SOLVER> = bincode::deserialize_from(&mut outp)?;
            if let Some(value) = min_doc_length {
                recognizer.set_min_doc_length(*value)
            }
            if let Some(value) = min_vector_length {
                recognizer.set_min_vector_length(*value)
            }
            recognizer
        }
        SvmRecognizerConfig::Train {
            language,
            classifier: training,
            ..
        } => {
            train(
                language,
                training,
                match context.stopword_registry() {
                    None => {None}
                    Some(registry) => {
                        if cfg.can_train() {
                            registry.get_or_load(cfg.language())
                        } else {
                            None
                        }
                    }
                }
            )?
        }
        SvmRecognizerConfig::All {
            language,
            classifier: training,
            retrain_if_possible,
            trained_svm,
            min_doc_length,
            min_vector_length,
            ..
        } => {
            let trained_svm = if let Some(root_setter) = root_setter {
                Cow::Owned(root_setter.set_root_if_not_exists(trained_svm))
            } else {
                Cow::Borrowed(trained_svm)
            };
            if !retrain_if_possible && trained_svm.exists() {
                let mut outp = BufReader::new(File::options().read(true).open(trained_svm.as_path())?);
                let mut recognizer: DocumentClassifier<TF, IDF, SOLVER> = bincode::deserialize_from(&mut outp)?;
                if let Some(value) = min_doc_length {
                    recognizer.set_min_doc_length(*value)
                }
                if let Some(value) = min_vector_length {
                    recognizer.set_min_vector_length(*value)
                }
                recognizer
            } else {
                let trained = train(
                    language,
                    training,
                    match context.stopword_registry() {
                        None => {None}
                        Some(registry) => {
                            if cfg.can_train() {
                                registry.get_or_load(cfg.language())
                            } else {
                                None
                            }
                        }
                    }
                )?;
                let mut outp = BufWriter::new(File::options().write(true).create(true).truncate(true).open(trained_svm.as_path())?);
                bincode::serialize_into(&mut outp, &trained_svm)?;
                trained
            }
        }
    };

    if let Some(test_data) = cfg.test_data() {
        if test_data.exists() {
            log::warn!("Testing the trained svm is currently not supported!")
        }
    }

    Ok(model)
}



/// Reads the train data from a csv.
fn read_train_data<Idf: IdfAlgorithm>(path: impl AsRef<Utf8Path>) -> Result<CsvProvider<TrainModelEntry, impl Read>, SvmCreationError<Idf>> {
    let mut csv_reader = csv::ReaderBuilder::new();
    csv_reader.has_headers(true);
    Ok(
        CsvProvider::new(
            csv_reader.from_reader(
                BufReader::new(
                    File::open(
                        path.as_ref()
                    )?
                )
            )
        )?
    )
}

pub(crate) fn train<TF, IDF, SOLVER>(
    language: &Language,
    training: &DocumentClassifierConfig<TF, IDF>,
    stopwords: Option<Arc<StopWordList>>
) -> Result<DocumentClassifier<TF, IDF, SOLVER>, SvmCreationError<IDF>> where
    TF: TfAlgorithm + Clone + Debug,
    IDF: IdfAlgorithm + Clone + Debug,
    SOLVER: SupportsParametersCreation
{
    if !training.train_data.exists() {
        return Err(SvmCreationError::IO(std::io::Error::new(ErrorKind::NotFound, format!("The file {} was not found!", training.train_data.to_string()))));
    }


    let tokenizer = Tokenizer::new(
        language.clone(),
        training.normalize_tokens,
        stopwords,
        training.stemmer.clone()
    );

    let vectorizer = match &training.tf_idf_data {
        None => {
            let reader = read_train_data(&training.train_data)?;
            super::text_processing::create_vectorizer(
                reader.map(|value| value.text),
                &tokenizer,
                TfIdf::new(training.tf.clone(), training.idf.clone())
            ).map_err(SvmCreationError::Idf)?
        }
        Some(path) => {
            let data = BufReader::new(File::options().read(true).open(path)?);
            super::text_processing::create_vectorizer(
                data.lines().filter_map(|value| value.ok()),
                &tokenizer,
                TfIdf::new(training.tf.clone(), training.idf.clone())
            ).map_err(SvmCreationError::Idf)?
        }
    };
    let reader = read_train_data(&training.train_data)?;

    let parameters = if let Some(ref params) = training.parameters {
        params.clone().try_into().map_err(LibLinearError::from)?
    } else {
        let mut generalized = GenericParameters::default();
        generalized.epsilon = Some(0.0003);
        generalized.p = Some(0.1);
        generalized.cost = Some(10.0);
        generalized.try_into().map_err(LibLinearError::from)?
    };

    Ok(
        DocumentClassifier::train(
            language,
            vectorizer,
            tokenizer,
            reader,
            &parameters,
            training.min_doc_length,
            training.min_vector_length
        )?
    )
}

#[cfg(test)]
pub(crate) mod test {
    use std::io::Read;
    use camino::Utf8PathBuf;
    use isolang::Language;
    use liblinear::parameter::serde::GenericParameters;
    use liblinear::solver::L2R_L2LOSS_SVR;
    use rust_stemmers::Algorithm;
    use crate::features::csv::CsvProvider;
    use crate::features::svm::config::DocumentClassifierConfig;
    use crate::features::svm::{read_train_data, train, TrainModelEntry};
    use crate::features::svm::classifier::DocumentClassifier;
    use crate::features::text_processing::tf_idf::{Idf, Tf};
    use crate::features::tokenizing::{SupportsStopwords, StopwordRegistryConfig};
    use crate::features::tokenizing::stopwords::{StopWordRegistry, StopWordRepository};

    struct RegistryContainer {
        stop_word_registry: StopWordRegistry
    }

    impl SupportsStopwords for RegistryContainer {
        fn stopword_registry(&self) -> Option<&StopWordRegistry> {
            Some(&self.stop_word_registry)
        }
    }

    impl Default for RegistryContainer {
        fn default() -> Self {
            Self {
                stop_word_registry: StopWordRegistry::initialize(
                    &StopwordRegistryConfig { registries: vec![StopWordRepository::IsoDefault] }
                ).unwrap()
            }
        }
    }

    pub fn create_german_gdbr_svm() -> DocumentClassifier<Tf, Idf, L2R_L2LOSS_SVR> {
        let reg = RegistryContainer::default();
        let cfg: DocumentClassifierConfig = DocumentClassifierConfig::new(
            super::super::text_processing::tf_idf::defaults::TERM_FREQUENCY_INVERSE.tf,
            super::super::text_processing::tf_idf::defaults::TERM_FREQUENCY_INVERSE.idf,
            "data/gdbr/de/svm.csv".into(),
            Some("data/gdbr/de/tf_idf.txt".into()),
            true,
            true,
            Some(Algorithm::German),
            Some(
                GenericParameters {
                    epsilon: Some(0.0003),
                    p: Some(0.1),
                    cost: Some(10.0),
                    ..GenericParameters::default()
                }
            ),
            5,
            5
        );
        train::<_, _, L2R_L2LOSS_SVR>(
            &Language::Deu,
            &cfg,
            reg.stop_word_registry.get_or_load(&Language::Deu)
        ).expect("The training failed!")
    }

    pub fn train_data() -> CsvProvider<TrainModelEntry, impl Read + Sized> {
        read_train_data::<Idf>(
            Utf8PathBuf::from("data/gdbr/de/svm.csv".to_string())
        ).unwrap()
    }

    #[test]
    fn can_train_svm(){

        let trained = create_german_gdbr_svm();

        let train_data = train_data();

        for value in train_data {
            println!("{:?} -> {:?}", trained.predict(&value.text).unwrap() < 0.5, value.is_class);
        }
        let x = serde_json::to_string(&trained).unwrap();
        let _loaded: DocumentClassifier<Tf, Idf, L2R_L2LOSS_SVR> = serde_json::from_str(&x).unwrap();
        drop(x);
    }
}