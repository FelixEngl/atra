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

use std::fmt::{Debug, Formatter};
use isolang::Language;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use liblinear::{Parameters, PredictionInput, TrainingInput};
use liblinear::solver::traits::{Solver, IsTrainableSolver};
use liblinear::Model;
use liblinear::model::traits::{ModelBase, TrainableModel};
use liblinear::solver::GenericSolver;
use text_processing::tokenizer::Tokenizer;
use crate::error::LibLinearError;
use text_processing::tf_idf::{IdfAlgorithm, TfAlgorithm};
use text_processing::vectorizer::{DocumentVectorizer};

#[derive(Serialize, Deserialize)]
#[serde(bound(
    serialize = "TF: Serialize, IDF: Serialize, SOLVER: IsTrainableSolver",
    deserialize = "TF: DeserializeOwned, IDF: DeserializeOwned, SOLVER: IsTrainableSolver, Model<SOLVER>: TryFrom<Model<GenericSolver>>"
))]
pub struct DocumentClassifier<TF, IDF, SOLVER> {
    language: Language,
    #[serde(with = "model_serializer")]
    model: Model<SOLVER>,
    vectorizer: DocumentVectorizer<String, TF, IDF>,
    tokenizer: Tokenizer,
    min_doc_length: usize,
    min_vector_length: usize
}

impl<TF, IDF, SOLVER> Debug for DocumentClassifier<TF, IDF, SOLVER> where TF: Debug, IDF: Debug, SOLVER: Solver  {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentClassifier")
            .field("language", &self.language)
            .field("vectorizer", &self.vectorizer)
            .field("model_solver", &SOLVER::ordinal())
            .field("tokenizer", &self.tokenizer)
            .field("min_doc_length", &self.min_doc_length)
            .field("min_vector_length", &self.min_vector_length)
            .finish()
    }
}

mod model_serializer {
    use std::fs::File;
    use std::io::{BufReader, BufWriter, Read, Write};
    use camino_tempfile::{tempdir};
    use liblinear::Model;
    use liblinear::solver::GenericSolver;
    use liblinear::solver::traits::IsTrainableSolver;
    use serde::{Deserialize, Deserializer, Serializer};
    use serde::de::Error as SError;
    use serde::ser::Error as DError;

    pub fn serialize<S, SOLVER: IsTrainableSolver>(model: &Model<SOLVER>, ser: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let file = tempdir().map_err(S::Error::custom)?;
        std::fs::create_dir_all(file.path()).unwrap();
        let model_path = file.path().join("model.tmp");
        liblinear::model::serde::save_model_to_disk(
            model,
            model_path.as_str()
        ).map_err(S::Error::custom)?;
        let mut dat = Vec::new();
        BufReader::new(File::options().read(true).open(model_path).map_err(S::Error::custom)?).read_to_end(&mut dat).map_err(S::Error::custom)?;
        ser.serialize_bytes(&dat)
    }

    pub fn deserialize<'de, D, SOLVER>(de: D) -> Result<Model<SOLVER>, D::Error> where D: Deserializer<'de>, Model<SOLVER>: TryFrom<Model<GenericSolver>> {
        let bytes: Vec<u8> = Vec::deserialize(de)?;
        let file = tempdir().map_err(D::Error::custom)?;
        std::fs::create_dir_all(file.path()).unwrap();
        let model_path = file.path().join("model.tmp");
        let mut buf = BufWriter::new(File::options().write(true).create_new(true).open(&model_path).map_err(D::Error::custom)?);
        buf.write(&bytes).map_err(D::Error::custom)?;
        buf.flush().map_err(D::Error::custom)?;
        drop(buf);
        let model = liblinear::model::serde::load_model_from_disk(model_path.as_str()).map_err(D::Error::custom)?;
        Ok(model.try_into().map_err(|_| D::Error::custom("Failed to convert model! {err:?}"))?)
    }
}

impl<TF, IDF, SOLVER> DocumentClassifier<TF, IDF, SOLVER> {
    pub fn new(
        language: Language,
        model: Model<SOLVER>,
        vectorizer: DocumentVectorizer<String, TF, IDF>,
        tokenizer: Tokenizer,
        min_doc_length: usize,
        min_vector_length: usize
    ) -> Self {
        Self { language, model, vectorizer, tokenizer, min_doc_length, min_vector_length }
    }

    pub fn model(&self) -> &Model<SOLVER> {
        &self.model
    }

    pub fn tokenize(&self, doc: &str) -> Vec<String> {
        self.tokenizer.tokenize(doc)
    }

    pub fn set_min_doc_length(&mut self, min_doc_length: usize) {
        self.min_doc_length = min_doc_length;
    }

    pub fn set_min_vector_length(&mut self, min_vector_length: usize) {
        self.min_vector_length = min_vector_length;
    }
}

/// A struct implementing this is used as train data.
pub trait TrainDataEntry {
    /// The label of the entry
    fn get_label(&self) -> f64;

    /// The text of the entry
    fn get_text(&self) -> &str;
}

impl<Text> TrainDataEntry for (f64, Text) where Text: AsRef<str> {
    fn get_label(&self) -> f64 {
        self.0
    }

    fn get_text(&self) -> &str {
        self.1.as_ref()
    }
}

impl<TF, IDF, SOLVER> DocumentClassifier<TF, IDF, SOLVER>
where
    TF: TfAlgorithm,
    IDF: IdfAlgorithm,
    SOLVER: IsTrainableSolver
{
    pub fn train<I: IntoIterator<Item=T>, T: TrainDataEntry>(
        language: &Language,
        vectorizer: DocumentVectorizer<String, TF, IDF>,
        tokenizer: Tokenizer,
        data: I,
        parameters: &Parameters<SOLVER>,
        min_doc_length: usize,
        min_vector_length: usize
    ) -> Result<DocumentClassifier<TF, IDF, SOLVER>, LibLinearError> {
        let mut labels = Vec::new();
        let mut features = Vec::new();

        for value in data {
            labels.push(value.get_label());
            let vector =
                vectorizer
                    .vectorize_document(tokenizer.tokenize(value.get_text()), true)
                    .sparse_features();
            features.push(vector);
        }

        log::info!("Train SVM with {} elements.", labels.len());

        let data = TrainingInput::from_sparse_features(
            labels,
            features
        )?;

        let model = Model::train(&data, parameters)?;
        Ok(
            DocumentClassifier::new(
                language.clone(),
                model,
                vectorizer,
                tokenizer,
                min_doc_length,
                min_vector_length
            )
        )
    }
}




impl<TF, IDF, SOLVER> DocumentClassifier<TF, IDF, SOLVER> where TF: TfAlgorithm, IDF: IdfAlgorithm {
    pub fn calculate_similarity(&self, doc_a: impl AsRef<str>, doc_b: impl AsRef<str>) -> f64 {
        let a = self.vectorizer.vectorize_document(self.tokenizer.tokenize(doc_a.as_ref()), true);
        let b = self.vectorizer.vectorize_document(self.tokenizer.tokenize(doc_b.as_ref()), true);
        match a.cosine_sim(&b) {
            Ok(value) => { value }
            Err(_) => {f64::NAN}
        }
    }
}

impl<TF, IDF, SOLVER> DocumentClassifier<TF, IDF, SOLVER> where TF: TfAlgorithm, IDF: IdfAlgorithm, SOLVER: Solver {
    pub fn predict(&self, doc: &str) -> Result<f64, LibLinearError> {
        let doc = self.tokenizer.tokenize(doc);
        if doc.len() <= self.min_doc_length {
            return Ok(-f64::NAN)
        }
        let doc = self.vectorizer
            .vectorize_document(doc, true);
        if doc.0 <= self.min_vector_length {
            return Ok(-f64::NAN)
        }
        Ok(self.model.predict(&PredictionInput::from_sparse_features(doc.sparse_features())?)?)
    }
}


