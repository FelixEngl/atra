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

use crate::corpus::CorpusStatisticsCollector;
use crate::tf_idf::{IdfAlgorithm, TfAlgorithm, TfIdf};
use crate::tokenizer::Tokenizer;
use itertools::Itertools;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::Hash;
use std::ops::Deref;

/// Creates a vectorizer for a corpus.
pub fn create_vectorizer<
    I: Iterator<Item = T>,
    T: AsRef<str>,
    Tf: TfAlgorithm,
    Idf: IdfAlgorithm,
>(
    train_data: I,
    tokenizer: &Tokenizer,
    tf_idf: TfIdf<Tf, Idf>,
) -> Result<DocumentVectorizer<String, Tf, Idf>, Idf::Error> {
    let mut corpus_statistics = CorpusStatisticsCollector::default();
    for document in train_data {
        let tokens = tokenizer.tokenize(document.as_ref());
        corpus_statistics.add(tokens);
    }
    Ok(corpus_statistics.provide_vectorizer(tf_idf)?)
}

/// Represents the entry in a tf-idf-vector.
/// Either represents a complete Tf-Idf value of only a Tf value for fallback processing.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TfIdfVectorEntry<W>(pub W, pub f64);
impl<W> Eq for TfIdfVectorEntry<W> where W: Eq {}

impl<W> PartialEq for TfIdfVectorEntry<W>
where
    W: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0) && float_cmp::approx_eq!(f64, self.1, other.1)
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct VectorizedDocument<W>(pub usize, pub Vec<TfIdfVectorEntry<W>>);

impl<W> VectorizedDocument<W> {
    pub fn plain_vector(&self) -> Vec<f64> {
        self.iter().map(|value| value.1).collect()
    }

    pub fn sparse_features(&self) -> Vec<(u32, f64)> {
        self.iter()
            .map(|value| value.1)
            .enumerate()
            .map(|(idx, value)| (idx as u32 + 1, value))
            .collect()
    }

    pub fn cosine_sim<W2>(&self, other: &VectorizedDocument<W2>) -> Result<f64, ()> {
        if self.1.len() != other.1.len() {
            return Err(());
        }
        let mut div = 0f64;
        let mut a_sum = 0f64;
        let mut b_sum = 0f64;
        for (a, b) in self.1.iter().zip_eq(other.1.iter()) {
            div += a.1 * b.1;
            a_sum += f64::powi(a.1, 2);
            b_sum += f64::powi(b.1, 2);
        }
        Ok(div / (a_sum.sqrt() * b_sum.sqrt()))
    }
}

impl<W> From<(usize, Vec<TfIdfVectorEntry<W>>)> for VectorizedDocument<W> {
    fn from((ct, value): (usize, Vec<TfIdfVectorEntry<W>>)) -> Self {
        Self(ct, value)
    }
}

impl<W> Deref for VectorizedDocument<W> {
    type Target = [TfIdfVectorEntry<W>];

    fn deref(&self) -> &Self::Target {
        &self.1
    }
}

/// Vectorizes a document with some kind of Idf
#[derive(Debug, Serialize, Deserialize)]
#[serde(bound(
    serialize = "W: Serialize + Hash + Eq, Tf: Serialize, Idf: Serialize",
    deserialize = "W: DeserializeOwned + Hash + Eq, Tf: DeserializeOwned, Idf: DeserializeOwned"
))]
pub struct DocumentVectorizer<W, Tf, Idf> {
    inner: MappedDocumentVectorizer<W>,
    tf_idf: TfIdf<Tf, Idf>,
}

pub type DocumentVectorizerNoTf<W, Idf> = DocumentVectorizer<W, (), Idf>;

impl<W, Tf, Idf> DocumentVectorizer<W, Tf, Idf> {
    pub fn tf_idf(&self) -> &TfIdf<Tf, Idf> {
        &self.tf_idf
    }
}

impl<W, Idf> DocumentVectorizer<W, (), Idf>
where
    W: Hash + Eq,
{
    fn without_tf(inner: MappedDocumentVectorizer<W>, idf: Idf) -> Self {
        Self::new(inner, TfIdf::new((), idf))
    }

    pub fn from_idf_mapping_without_tf(map: HashMap<W, f64>, idf: Idf) -> Self {
        Self::without_tf(MappedDocumentVectorizer::from_iter(map), idf)
    }

    pub fn from_iter_without_tf<T: IntoIterator<Item = (W, f64)>>(iter: T, idf: Idf) -> Self {
        Self::without_tf(MappedDocumentVectorizer::from_iter(iter), idf)
    }
}

impl<W, Tf, Idf> DocumentVectorizer<W, Tf, Idf>
where
    W: Hash + Eq,
{
    fn new(inner: MappedDocumentVectorizer<W>, tf_idf: TfIdf<Tf, Idf>) -> Self {
        Self { inner, tf_idf }
    }

    pub fn from_idf_mapping(map: HashMap<W, f64>, tf_idf: TfIdf<Tf, Idf>) -> Self {
        Self::new(MappedDocumentVectorizer::from_iter(map), tf_idf)
    }

    pub fn from_iter<T: IntoIterator<Item = (W, f64)>>(iter: T, tf_idf: TfIdf<Tf, Idf>) -> Self {
        Self::new(MappedDocumentVectorizer::from_iter(iter), tf_idf)
    }

    #[inline]
    pub fn vectorize_document_with<TfAlt: TfAlgorithm, D: IntoIterator<Item = W>>(
        &self,
        tf: &TfAlt,
        doc: D,
        normalize: bool,
    ) -> VectorizedDocument<&W> {
        VectorizedDocument::from(self.inner.vectorize_document(tf, doc, normalize))
    }

    pub unsafe fn vectorize_tf_document(
        &self,
        doc: HashMap<W, f64>,
        normalize: bool,
    ) -> VectorizedDocument<&W> {
        VectorizedDocument::from(self.inner.vectorize_tf_document(doc, normalize))
    }
}

impl<W, Tf, Idf> DocumentVectorizer<W, Tf, Idf>
where
    W: Hash + Eq,
    Tf: TfAlgorithm,
{
    #[inline]
    pub fn vectorize_document<D: IntoIterator<Item = W>>(
        &self,
        doc: D,
        normalize: bool,
    ) -> VectorizedDocument<&W> {
        self.vectorize_document_with(&self.tf_idf.tf, doc, normalize)
    }
}

impl<W, Tf, Idf> Clone for DocumentVectorizer<W, Tf, Idf>
where
    W: Clone,
    Tf: Clone,
    Idf: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            tf_idf: self.tf_idf.clone(),
        }
    }
}

/// Contains the raw idf data of a corpus
#[derive(Debug, Serialize, Deserialize)]
#[repr(transparent)]
struct MappedDocumentVectorizer<W> {
    inner: Vec<(W, f64)>,
}

impl<W> MappedDocumentVectorizer<W> {
    pub fn new(mut inner: Vec<(W, f64)>) -> Self {
        inner.shrink_to_fit();
        Self { inner }
    }

    #[allow(dead_code)]
    pub fn into_inner(self) -> Vec<(W, f64)> {
        self.inner
    }
}

impl<W> MappedDocumentVectorizer<W>
where
    W: Hash + Eq,
{
    /// Vectorizes [doc] to a vector containing the words and associated
    #[inline]
    fn vectorize_document<Tf: TfAlgorithm, D: IntoIterator<Item = W>>(
        &self,
        tf: &Tf,
        doc: D,
        normalized: bool,
    ) -> (usize, Vec<TfIdfVectorEntry<&W>>) {
        unsafe { self.vectorize_tf_document(tf.calculate_tf(doc), normalized) }
    }

    unsafe fn vectorize_tf_document(
        &self,
        doc: HashMap<W, f64>,
        normalized: bool,
    ) -> (usize, Vec<TfIdfVectorEntry<&W>>) {
        let mut result = Vec::with_capacity(self.inner.len());
        let mut ct = 0usize;
        for (word, idf) in &self.inner {
            let idf = *idf;
            if let Some(tf) = doc.get(word) {
                ct += 1;
                result.push(TfIdfVectorEntry(word, (*tf) * idf))
            } else {
                result.push(TfIdfVectorEntry(word, 0.0))
            }
        }
        if normalized {
            let sum: f64 = result.iter().map(|value| value.1).sum();
            for value in &mut result {
                value.1 /= sum;
            }
        }
        (ct, result)
    }
}

impl<W> MappedDocumentVectorizer<W>
where
    W: Clone,
{
    fn clone(&self) -> Self {
        MappedDocumentVectorizer::new(self.inner.clone())
    }
}

impl<W> FromIterator<(W, f64)> for MappedDocumentVectorizer<W>
where
    W: Hash + Eq,
{
    fn from_iter<T: IntoIterator<Item = (W, f64)>>(iter: T) -> Self {
        Self::new(iter.into_iter().collect())
    }
}
