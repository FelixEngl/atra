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
use camino::Utf8Path;
use csv::{StringRecord, StringRecordsIntoIter};
use itertools::Itertools;
use liblinear::solver::L2R_L2LOSS_SVR;
use serde::{Deserialize};
use thiserror::Error;
use crate::features::gdpr::classifier::{DocumentClassifier, train};
use crate::features::gdpr::error::LibLinearError;
use crate::features::gdpr::traits::GdbrContext;
use crate::features::text_processing::text_preprocessor::Tokenizer;
use crate::features::text_processing::tf_idf::{Idf, IdfAlgorithm, Tf, TfIdf};
use crate::features::gdpr::traits::GdbrConfig;
use crate::features::text_processing::traits::Config as Cfg2;

pub mod svm;
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
    #[error("Was not able to load the gdpr!")]
    NoGdprDataFound,
    #[error(transparent)]
    Serialisation(#[from] bincode::Error)
}

pub async fn gdpr(
    context: &impl GdbrContext
) -> Result<DocumentClassifier<Tf, Idf, L2R_L2LOSS_SVR>, GdprError> {
    let cfg = context.gdpr_config();
    async fn train_gdpr(
        context: &impl GdbrContext,
        path: impl AsRef<Utf8Path>
    ) -> Result<DocumentClassifier<Tf, Idf, L2R_L2LOSS_SVR>, GdprError>{
        let path = path.as_ref();
        let svm = path.join("svm.csv");
        if !svm.exists() {
            return Err(GdprError::IO(std::io::Error::new(ErrorKind::NotFound, format!("The file {} was not found!", svm.to_string()))));
        }
        let tf_idf = path.join("tf_idf.txt");
        if !tf_idf.exists() {
            return Err(GdprError::IO(std::io::Error::new(ErrorKind::NotFound, format!("The file {} was not found!", tf_idf.to_string()))));
        }
        let data = BufReader::new(File::options().read(true).open(tf_idf)?);

        let cfg = context.gdpr_config();

        let tokenizer = if let Some(lang) = cfg.target_language() {
            context.stopword_registry().get_or_load(lang).await
        } else {
            None
        };


        let tokenizer = Tokenizer::new(
            cfg.normalize_text(),
            tokenizer,
            cfg.stemmer()
        );



        let vectorizer = super::text_processing::create_vectorizer(
            data.lines().filter_map(|value| value.ok()),
            &tokenizer,
            TfIdf::new(cfg.tf(), cfg.idf())
        )?;

        let mut csv_reader = csv::ReaderBuilder::new();
        csv_reader.has_headers(true);
        let mut csv_reader = csv_reader.from_path(svm)?;
        let header = csv_reader.headers()?;

        struct CsvProvider {
            header: StringRecord,
            string_records_iter: StringRecordsIntoIter<File>,
        }

        impl Iterator for CsvProvider {
            type Item = TrainModelEntry;

            fn next(&mut self) -> Option<Self::Item> {
                let next = self.string_records_iter.next()?.ok()?;
                next.deserialize(Some(&self.header)).ok()
            }
        }

        let provider = CsvProvider {
            header: header.clone(),
            string_records_iter: csv_reader.into_records()
        };

        let training = train(
            vectorizer,
            tokenizer,
            provider
        )?;

        Ok(training)
    }

    let can_load_something = cfg.path_to_trained_classifier().is_some_and(|value| value.exists());

    if !can_load_something || cfg.retrain_if_possible() {
        if let Some(path) = cfg.path_to_train_data() {
            match train_gdpr(context, path).await {
                Ok(value) => {
                    if let Some(path) = cfg.path_to_trained_classifier() {
                        if path.exists() {
                            std::fs::remove_file(&path)?;
                        }
                        let outp = File::options().create(true).write(true).open(path)?;
                        let mut outp = BufWriter::new(outp);
                        bincode::serialize_into(&mut outp, &value)?;
                        outp.flush()?;
                        drop(outp);
                        return Ok(value);
                    } else {
                        log::warn!("No path to save trained svm provided!");
                        return Ok(value);
                    }
                }
                Err(err) => {
                    log::error!("Failed to train the svm due to {}", err);
                }
            }
        } else {
            log::warn!("Was not able to find train data for gdpr filter!");
        }
        if !can_load_something {
            return Err(GdprError::NoGdprDataFound);
        }
    }

    if let Some(path) = cfg.path_to_trained_classifier() {
        let mut file = BufReader::new(File::options().read(true).open(path)?);
        Ok(bincode::deserialize_from(&mut file)?)
    } else {
        log::warn!("Was not able to find train data for gdpr filter!");
        Err(GdprError::NoGdprDataFound)
    }
}
