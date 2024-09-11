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

use liblinear::errors::{ModelError, TrainingInputError, PredictionInputError};
use thiserror::Error;
use text_processing::tf_idf::{IdfAlgorithm};

/// An error from liblinear
#[derive(Debug, Error)]
pub enum LibLinearError {
    #[error(transparent)]
    Training(#[from] TrainingInputError),
    #[error(transparent)]
    Build(#[from] ModelError),
    #[error(transparent)]
    Prediction(#[from] PredictionInputError)
}


/// An error from creating a svm classifier
#[derive(Debug, Error)]
pub enum SvmCreationError<Idf: IdfAlgorithm> {
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    Idf(Idf::Error),
    #[error(transparent)]
    LibLinear(#[from] LibLinearError),
    #[error(transparent)]
    CSV(#[from] csv::Error),
    #[error(transparent)]
    Serialisation(#[from] bincode::Error),
}