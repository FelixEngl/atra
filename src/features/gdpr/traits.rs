use std::sync::Arc;
use camino::Utf8PathBuf;
use liblinear::solver::L2R_L2LOSS_SVR;
use crate::features::gdpr::classifier::DocumentClassifier;
use crate::features::text_processing::tf_idf::{Idf, Tf};

pub trait GdbrConfig {
    fn retrain_if_possible(&self) -> bool;

    fn path_to_train_data(&self) -> Option<Utf8PathBuf>;

    fn path_to_trained_classifier(&self) -> Option<Utf8PathBuf>;

    fn tf(&self) -> Tf;

    fn idf(&self) -> Idf;
}

pub struct GdbrConfigElement {
    retrain_if_possible: bool,
    path_to_train_data: Option<Utf8PathBuf>,
    path_to_trained_classifier: Option<Utf8PathBuf>,
}

pub trait GdbrContext {
    fn get_gdbr_classifier(&self, lang: &isolang::Language) -> Arc<DocumentClassifier<Tf, Idf, L2R_L2LOSS_SVR>>;
}

