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

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, ErrorKind, Write};
use camino::{Utf8Path, Utf8PathBuf};
use csv::{StringRecord, StringRecordsIntoIter};
use isolang::Language;
use itertools::Itertools;
use liblinear::Model;
use liblinear::solver::{GenericSolver, L2R_L2LOSS_SVR};
use liblinear::solver::traits::{IsSupportVectorRegressionSolver, IsTrainableSolver};
use serde::{Deserialize};
use thiserror::Error;
use crate::features::gdpr::classifier::{DocumentClassifier};
use crate::features::gdpr::classifier::train as train2;
use crate::features::gdpr::error::LibLinearError;
use crate::features::gdpr::traits::{GdbrContext, GdbrRecognizerConfig, GdbrRecognizerTrainConfig};
use crate::features::text_processing::tf_idf::{Idf, IdfAlgorithm, Tf, TfIdf};
use crate::features::tokenizing::StopwordContext;
use crate::features::tokenizing::tokenizer::Tokenizer;

pub mod classifier;
pub mod error;
pub mod traits;

#[derive(Debug, Error)]
pub enum CreateModelError {
    #[error(transparent)]
    LibLinear(#[from] LibLinearError),
    #[error(transparent)]
    CSV(#[from] csv::Error),
}

#[derive(Debug, Deserialize)]
pub struct TrainModelEntry {
    is_gdbr: bool,
    text: String
}


#[derive(Debug, Error)]
pub enum GdprError {
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    Idf(#[from] <Idf as IdfAlgorithm>::Error),
    #[error(transparent)]
    LibLinear(#[from] LibLinearError),
    #[error(transparent)]
    CSV(#[from] csv::Error),
    #[error(transparent)]
    Serialisation(#[from] bincode::Error)
}


pub async fn gdpr<C, SOLVER: IsTrainableSolver + IsSupportVectorRegressionSolver>(
    cfg: &GdbrRecognizerConfig,
    context: &C
) -> Result<DocumentClassifier<Tf, Idf, SOLVER>, GdprError> where
    C: StopwordContext,
    Model<SOLVER>: TryFrom<Model<GenericSolver>>
{

    async fn train<C, SOLVER: IsTrainableSolver + IsSupportVectorRegressionSolver>(
        context: &C,
        language: &Language,
        training: &GdbrRecognizerTrainConfig
    ) -> Result<DocumentClassifier<Tf, Idf, SOLVER>, GdprError> where C: StopwordContext {
        struct CsvProvider {
            header: StringRecord,
            string_records_iter: StringRecordsIntoIter<BufReader<File>>,
        }

        impl Iterator for CsvProvider {
            type Item = TrainModelEntry;

            fn next(&mut self) -> Option<Self::Item> {
                let next = self.string_records_iter.next()?.ok()?;
                next.deserialize(Some(&self.header)).ok()
            }
        }

        fn read_train_data(path: impl AsRef<Utf8Path>) -> Result<CsvProvider, GdprError> {
            let mut csv_reader = csv::ReaderBuilder::new();
            csv_reader.has_headers(true);
            let mut csv_reader = csv_reader.from_reader(BufReader::new(File::open(path.as_ref())?));
            let header = csv_reader.headers()?;
            Ok(
                CsvProvider {
                    header: header.clone(),
                    string_records_iter: csv_reader.into_records()
                }
            )
        }

        if !training.train_data.exists() {
            return Err(GdprError::IO(std::io::Error::new(ErrorKind::NotFound, format!("The file {} was not found!", training.train_data.to_string()))));
        }

        let stopwords = context.stopword_registry().get_or_load(&language).await;
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
                )?
            }
            Some(path) => {
                let data = BufReader::new(File::options().read(true).open(path)?);
                super::text_processing::create_vectorizer(
                    data.lines().filter_map(|value| value.ok()),
                    &tokenizer,
                    TfIdf::new(training.tf.clone(), training.idf.clone())
                )?
            }
        };
        let reader = read_train_data(&training.train_data)?;
        Ok(
            train2(
                language,
                vectorizer,
                tokenizer,
                reader
            )?
        )
    }

    let model = match cfg {
        GdbrRecognizerConfig::Load {
            trained_svm,
            ..
        } => {
            let mut outp = BufReader::new(File::options().read(true).open(trained_svm)?);
            bincode::deserialize_from(&mut outp)?
        }
        GdbrRecognizerConfig::Train {
            language,
            training,
            ..
        } => {
            train(
                context,
                language,
                training
            ).await?
        }
        GdbrRecognizerConfig::All {
            language,
            training,
            retrain_if_possible,
            trained_svm,
            ..
        } => {
            if !retrain_if_possible && trained_svm.exists() {
                let mut outp = BufReader::new(File::options().read(true).open(trained_svm)?);
                bincode::deserialize_from(&mut outp)?
            } else {
                let trained = train(
                    context,
                    language,
                    training
                ).await?;
                let mut outp = BufWriter::new(File::options().write(true).create(true).truncate(true).open(trained_svm)?);
                bincode::serialize_into(&mut outp, &trained_svm)?;
                trained
            }
        }
    };

    if let Some(test_data) = cfg.test_data() {
        if test_data.exists() {
            todo!()
        }
    }

    Ok(model)
}
