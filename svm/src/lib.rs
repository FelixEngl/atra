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

pub mod classifier;
pub mod config;
pub mod error;
mod toolkit;

mod csv2;

use crate::classifier::{DocumentClassifier, TrainDataEntry};
use crate::config::{DocumentClassifierConfig, SvmRecognizerConfig};
use crate::error::{LibLinearError, SvmCreationError};
use camino::Utf8Path;
pub use csv2::CsvProvider;
use isolang::Language;
use liblinear::parameter::serde::{GenericParameters, SupportsParametersCreation};
use liblinear::solver::GenericSolver;
use liblinear::Model;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::Debug;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, ErrorKind, Read};
use std::path::Path;
use std::sync::Arc;
use text_processing::stopword_registry::{StopWordList, StopWordRegistry};
use text_processing::tf_idf::{IdfAlgorithm, TfAlgorithm, TfIdf};
use text_processing::tokenizer::Tokenizer;
use crate::error::LibLinearError::Prediction;

pub fn create_document_classifier<TF, IDF, SOLVER>(
    cfg: &SvmRecognizerConfig<TF, IDF>,
    stopword_registry: Option<&StopWordRegistry>,
) -> Result<DocumentClassifier<TF, IDF, SOLVER>, SvmCreationError<IDF>>
where
    TF: TfAlgorithm + Serialize + DeserializeOwned + Clone + Debug,
    IDF: IdfAlgorithm + Serialize + DeserializeOwned + Clone + Debug,
    SOLVER: SupportsParametersCreation,
    Model<SOLVER>: TryFrom<Model<GenericSolver>>,
{
    let model = match &cfg {
        SvmRecognizerConfig::Load {
            trained_svm,
            min_doc_length,
            min_vector_length,
            ..
        } => {
            let mut outp = BufReader::new(File::options().read(true).open(trained_svm.as_path())?);
            let mut recognizer: DocumentClassifier<TF, IDF, SOLVER> =
                bincode::deserialize_from(&mut outp)?;
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
        } => train(
            language,
            training,
            stopword_registry
                .and_then(|value| cfg.can_train().then(|| value.get_or_load(cfg.language())))
                .flatten(),
        )?,
        SvmRecognizerConfig::All {
            language,
            classifier: training,
            retrain_if_possible,
            trained_svm,
            min_doc_length,
            min_vector_length,
            ..
        } => {
            if !retrain_if_possible && trained_svm.exists() {
                let mut outp =
                    BufReader::new(File::options().read(true).open(trained_svm.as_path())?);
                let mut recognizer: DocumentClassifier<TF, IDF, SOLVER> =
                    bincode::deserialize_from(&mut outp)?;
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
                    stopword_registry
                        .and_then(|value| {
                            cfg.can_train().then(|| value.get_or_load(cfg.language()))
                        })
                        .flatten(),
                )?;
                let mut outp = BufWriter::new(
                    File::options()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(trained_svm.as_path())?,
                );
                bincode::serialize_into(&mut outp, &trained)?;
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

/// An entry of a train csv
#[derive(Debug, Deserialize)]
pub struct CsvTrainModelEntry {
    #[serde(alias = "is_gdbr")]
    pub is_class: bool,
    pub text: String,
}

impl TrainDataEntry for crate::CsvTrainModelEntry {
    fn get_label(&self) -> f64 {
        if self.is_class {
            1.0
        } else {
            -1.0
        }
    }

    fn get_text(&self) -> &str {
        &self.text
    }
}

/// Reads the train data from a csv.
pub fn read_train_data<Idf: IdfAlgorithm>(
    path: impl AsRef<Path>,
) -> Result<CsvProvider<CsvTrainModelEntry, impl Read>, SvmCreationError<Idf>> {
    let mut csv_reader = csv::ReaderBuilder::new();
    csv_reader.has_headers(true);
    Ok(CsvProvider::new(
        csv_reader.from_reader(BufReader::new(File::open(path.as_ref())?)),
    )?)
}

pub fn train<TF, IDF, SOLVER>(
    language: &Language,
    training: &DocumentClassifierConfig<TF, IDF>,
    stopwords: Option<Arc<StopWordList>>,
) -> Result<DocumentClassifier<TF, IDF, SOLVER>, SvmCreationError<IDF>>
where
    TF: TfAlgorithm + Clone + Debug,
    IDF: IdfAlgorithm + Clone + Debug,
    SOLVER: SupportsParametersCreation,
{
    log::info!("Train SVM for {}", language.to_name());
    if !training.train_data.exists() {
        return Err(SvmCreationError::IO(std::io::Error::new(
            ErrorKind::NotFound,
            format!(
                "The file {} was not found!",
                training.train_data.to_string()
            ),
        )));
    }

    let tokenizer = Tokenizer::new(
        language.clone(),
        training.normalize_tokens,
        stopwords,
        training.stemmer.clone(),
    );


    let vectorizer = match &training.tf_idf_data {
        None => {
            let reader = read_train_data(&training.train_data)?;
            text_processing::vectorizer::create_vectorizer(
                reader.map(|value| value.text),
                &tokenizer,
                TfIdf::new(training.tf.clone(), training.idf.clone()),
            )
            .map_err(SvmCreationError::Idf)?
        }
        Some(path) => {
            let data = BufReader::new(File::options().read(true).open(path)?);
            text_processing::vectorizer::create_vectorizer(
                data.lines().filter_map(|value| value.ok()),
                &tokenizer,
                TfIdf::new(training.tf.clone(), training.idf.clone()),
            )
            .map_err(SvmCreationError::Idf)?
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

    Ok(DocumentClassifier::train(
        language,
        vectorizer,
        tokenizer,
        reader,
        &parameters,
        training.min_doc_length,
        training.min_vector_length,
    )?)
}

#[cfg(test)]
mod test {
    use crate::classifier::DocumentClassifier;
    use crate::config::DocumentClassifierConfig;
    use crate::csv2::CsvProvider;
    use crate::{read_train_data, train, CsvTrainModelEntry};
    use camino::Utf8PathBuf;
    use isolang::Language;
    use liblinear::parameter::serde::GenericParameters;
    use liblinear::solver::L2R_L2LOSS_SVR;
    use rust_stemmers::Algorithm;
    use std::io::Read;
    use text_processing::configs::StopwordRegistryConfig;
    use text_processing::stopword_registry::{StopWordRegistry, StopWordRepository};
    use text_processing::tf_idf::{Idf, Tf};

    fn create_german_gdbr_svm() -> DocumentClassifier<Tf, Idf, L2R_L2LOSS_SVR> {
        let reg = StopwordRegistryConfig {
            registries: vec![StopWordRepository::IsoDefault],
        };
        let reg = StopWordRegistry::initialize(&reg);

        let cfg: DocumentClassifierConfig = DocumentClassifierConfig::new(
            text_processing::tf_idf::defaults::TERM_FREQUENCY_INVERSE.tf,
            text_processing::tf_idf::defaults::TERM_FREQUENCY_INVERSE.idf,
            "data/gdbr/de/svm.csv".into(),
            Some("data/gdbr/de/tf_idf.txt".into()),
            true,
            true,
            Some(Algorithm::German),
            Some(GenericParameters {
                epsilon: Some(0.0003),
                p: Some(0.1),
                cost: Some(10.0),
                ..GenericParameters::default()
            }),
            5,
            5,
        );

        train::<_, _, L2R_L2LOSS_SVR>(&Language::Deu, &cfg, reg.get_or_load(&Language::Deu))
            .expect("The training failed!")
    }

    fn train_data() -> CsvProvider<CsvTrainModelEntry, impl Read + Sized> {
        read_train_data::<Idf>(Utf8PathBuf::from("data/gdbr/de/svm.csv".to_string())).unwrap()
    }

    #[test]
    fn can_train_svm() {
        let trained = create_german_gdbr_svm();

        let train_data = train_data();

        for value in train_data {
            println!(
                "{:?} -> {:?}",
                trained.predict(&value.text).unwrap() < 0.5,
                value.is_class
            );
        }
        let x = serde_json::to_string(&trained).unwrap();
        let _loaded: DocumentClassifier<Tf, Idf, L2R_L2LOSS_SVR> =
            serde_json::from_str(&x).unwrap();
        drop(x);
    }
}
