use liblinear::model::traits::ModelBase;
use liblinear::PredictionInput;
use liblinear::solver::{L2R_L2LOSS_SVR};
use liblinear::solver::traits::IsLogisticRegressionSolver;
use serde::{Deserialize, Serialize};
use crate::features::gdpr::error::LibLinearError;
use crate::features::gdpr::TrainModelEntry;
use crate::features::text_processing::text_preprocessor::Tokenizer;
use crate::features::text_processing::tf_idf::{IdfAlgorithm, TfAlgorithm};
use crate::features::text_processing::vectorizer::{DocumentVectorizer};
use liblinear::solver::traits::IsTrainableSolver;
use serde::de::DeserializeOwned;
use liblinear::Model;
use liblinear::solver::GenericSolver;

#[derive(Serialize, Deserialize)]
#[serde(bound(
    serialize = "Tf: Serialize, Idf: Serialize, SOLVER: IsTrainableSolver",
    deserialize = "Tf: DeserializeOwned, Idf: DeserializeOwned, SOLVER: IsTrainableSolver, Model<SOLVER>: TryFrom<Model<GenericSolver>>"
))]
pub struct DocumentClassifier<Tf, Idf, SOLVER> {
    #[serde(with = "model_serializer")]
    svm: liblinear::Model<SOLVER>,
    vectorizer: DocumentVectorizer<String, Tf, Idf>,
    tokenizer: Tokenizer
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

    pub fn serialize<S, SOLVER: IsTrainableSolver>(model: &liblinear::Model<SOLVER>, ser: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let file = tempdir().map_err(S::Error::custom)?;
        let model_path = file.path().join("model.tmp");
        let model_path = model_path.canonicalize_utf8().map_err(S::Error::custom)?;
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
        let model_path = file.path().join("model.tmp");
        let model_path = model_path.canonicalize_utf8().map_err(D::Error::custom)?;
        let mut buf = BufWriter::new(File::options().write(true).open(&model_path).map_err(D::Error::custom)?);
        buf.write(&bytes).map_err(D::Error::custom)?;
        buf.flush().map_err(D::Error::custom)?;
        drop(buf);
        let model = liblinear::model::serde::load_model_from_disk(model_path.as_str()).map_err(D::Error::custom)?;
        Ok(model.try_into().map_err(|err| D::Error::custom("Failed to convert model!"))?)
    }
}

impl<Tf, Idf, SOLVER> DocumentClassifier<Tf, Idf, SOLVER> {
    pub fn new(svm: liblinear::Model<SOLVER>, vectorizer: DocumentVectorizer<String, Tf, Idf>, tokenizer: Tokenizer) -> Self {
        Self { svm, vectorizer, tokenizer }
    }
}

pub fn train<Tf: TfAlgorithm, Idf: IdfAlgorithm, I: IntoIterator<Item=TrainModelEntry>>(
    vectorizer: DocumentVectorizer<String, Tf, Idf>, tokenizer: Tokenizer, data: I
) -> Result<DocumentClassifier<Tf, Idf, L2R_L2LOSS_SVR>, LibLinearError> {

    let mut labels = Vec::new();
    let mut features = Vec::new();

    for value in data {
        labels.push(if value.is_gdbr { 1.0 } else { 0.0 });
        let vector =
            vectorizer
                .vectorize_document(tokenizer.tokenize(&value.text), true)
                .sparse_features();
        features.push(vector);
    }

    log::info!("Train SVM with {} elements.", labels.len());

    Ok(
        DocumentClassifier::new(
            super::svm::train(labels, features)?,
            vectorizer,
            tokenizer
        )
    )
}




impl<Tf, Idf, SOLVER> DocumentClassifier<Tf, Idf, SOLVER> where Tf: TfAlgorithm, Idf: IdfAlgorithm {
    pub fn calculate_similarity(&self, doc_a: impl AsRef<str>, doc_b: impl AsRef<str>) -> f64 {
        let a = self.vectorizer.vectorize_document(self.tokenizer.tokenize(doc_a.as_ref()), true);
        let b = self.vectorizer.vectorize_document(self.tokenizer.tokenize(doc_b.as_ref()), true);
        match a.cosine_sim(&b) {
            Ok(value) => { value }
            Err(_) => {f64::NAN}
        }
    }
}

impl<Tf, Idf, SOLVER> DocumentClassifier<Tf, Idf, SOLVER> where Tf: TfAlgorithm, Idf: IdfAlgorithm, SOLVER: IsLogisticRegressionSolver {
    pub fn predict_dsgvo(&self, doc: impl AsRef<str>) -> Result<f64, LibLinearError> {
        let doc = self.vectorizer
            .vectorize_document(self.tokenizer.tokenize(doc.as_ref()), true)
            .sparse_features();
        Ok(self.svm.predict(&PredictionInput::from_sparse_features(doc)?)?)
    }
}