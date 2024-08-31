use camino::Utf8PathBuf;
use crate::features::text_processing::tf_idf::{Idf, Tf};

pub trait Config: super::super::text_processing::traits::Config {
    fn retrain_if_possible(&self) -> bool;

    fn path_to_train_data(&self) -> Option<Utf8PathBuf>;

    fn path_to_trained_classifier(&self) -> Option<Utf8PathBuf>;

    fn tf(&self) -> Tf;

    fn idf(&self) -> Idf;
}

pub trait Context : super::super::text_processing::traits::Context {
    fn gdpr_config(&self) -> impl Config;
}

