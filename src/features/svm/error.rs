use liblinear::errors::{ModelError, TrainingInputError, PredictionInputError};
use thiserror::Error;
use crate::features::text_processing::tf_idf::{IdfAlgorithm};

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