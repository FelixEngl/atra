use liblinear::errors::{ModelError, TrainingInputError, PredictionInputError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LibLinearError {
    #[error(transparent)]
    Training(#[from] TrainingInputError),
    #[error(transparent)]
    Build(#[from] ModelError),
    #[error(transparent)]
    Prediction(#[from] PredictionInputError)
}