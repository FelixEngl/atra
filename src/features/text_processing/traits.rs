use isolang::Language;
use rust_stemmers::Algorithm;
use crate::features::text_processing::text_preprocessor::StopWordListRegistry;

pub trait Config {
    fn normalize_text(&self) -> bool;
    fn target_language(&self) -> Option<Language>;
    fn stemmer(&self) -> Option<Algorithm>;
}

pub trait Context {
    fn stop_registry(&self) -> &StopWordListRegistry;
}