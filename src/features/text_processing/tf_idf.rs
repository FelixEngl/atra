use std::collections::{HashMap};
use std::collections::hash_map::Entry;
use std::error::Error;
use std::hash::Hash;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use crate::features::text_processing::corpus::CorpusDocumentStatistics;
use serde::de::DeserializeOwned;

pub mod defaults {
    use crate::features::text_processing::tf_idf::{Idf, Tf, TfIdf};
    pub const RAW_INVERSE: TfIdf<Tf, Idf> = TfIdf::new(Tf::RawCount, Idf::InverseDocumentFrequency);
    pub const TERM_FREQUENCY_INVERSE: TfIdf<Tf, Idf> = TfIdf::new(Tf::TermFrequency, Idf::InverseDocumentFrequency);
    pub const RAW_INVERSE_SMOOTH: TfIdf<Tf, Idf> = TfIdf::new(Tf::RawCount, Idf::InverseDocumentFrequencySmooth);
    pub const TERM_FREQUENCY_INVERSE_SMOOTH: TfIdf<Tf, Idf> = TfIdf::new(Tf::TermFrequency, Idf::InverseDocumentFrequencySmooth);
}

/// A combination of Tf and Idf
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound(
    serialize = "Tf: Serialize, Idf: Serialize",
    deserialize = "Tf: DeserializeOwned, Idf: DeserializeOwned"
))]
pub struct TfIdf<Tf, Idf> {
    pub tf: Tf,
    pub idf: Idf
}

impl<Tf, Idf>  TfIdf<Tf, Idf> where Tf: TfAlgorithm {
    delegate::delegate! {
        to self.tf {
            fn calculate_tf<W, D: IntoIterator<Item=W>>(&self, doc: D) -> HashMap<W, f64> where W: Hash + Eq;
        }
    }
}

impl<Tf, Idf>  TfIdf<Tf, Idf> where Idf: IdfAlgorithm {
    delegate::delegate! {
        to self.idf {
            fn calculate_idf<W, S: CorpusDocumentStatistics<Word=W>>(&self, statistics: & S, word: &W) -> Result<Option<f64>, Idf::Error>;
            fn calculate_idf_with_word_frequency<W, S: CorpusDocumentStatistics<Word=W>>(&self, statistics: & S, word: &W, word_frequency: u64) -> Result<f64, Idf::Error>;
        }
    }
}

impl<Idf> TfIdf<(), Idf> {
    pub const fn new_idf_only(idf: Idf) -> Self {
        Self::new((), idf)
    }
}

impl<Tf, Idf> TfIdf<Tf, Idf> {
    pub const fn new(tf: Tf, idf: Idf) -> Self {
        Self{tf, idf}
    }

    pub fn to_idf_only(self) -> TfIdf<(), Idf> {
        TfIdf::<(), Idf>::new_idf_only(self.idf)
    }
}

impl<T> From<T> for TfIdf<(), T> where T: IdfAlgorithm {
    fn from(value: T) -> Self {
        Self::new((), value)
    }
}

impl<Tf, Idf> Copy for TfIdf<Tf, Idf> where Tf: Copy, Idf: Copy{}


/// Trait for IDF Algorithms
pub trait IdfAlgorithm {
    type Error: Error;

    /// Calculates the IDF value for a single word based on the provided statistics.
    ///
    /// [number_of_documents] denotes the number of documents in the corpus
    /// [number_of_words] denote the number of distinct words in the whole corpus
    /// [word_frequency] denotes the frequency of a specific word in a corpus
    #[inline]
    fn calculate_idf<W, S: CorpusDocumentStatistics<Word=W>>(&self, statistics: & S, word: &W) -> Result<Option<f64>, Self::Error> {
        statistics
            .word_frequency(word)
            .map(|value| self.calculate_idf_with_word_frequency(statistics, word, value))
            .transpose()
    }

    /// Calculates the IDF value for a single word based on the provided statistics.
    /// [word_frequency] denotes the frequency of a specific word in a corpus.
    ///
    /// Returns nan if the calculation is not possible.
    fn calculate_idf_with_word_frequency<W, S: CorpusDocumentStatistics<Word=W>>(&self, statistics: & S, word: &W, word_frequency: u64) -> Result<f64, Self::Error>;
}


/// Default IDF Algorithms
/// From https://en.wikipedia.org/wiki/Tf%E2%80%93idf
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum Idf {
    Unary,
    InverseDocumentFrequency,
    InverseDocumentFrequencySmooth,
    InverseDocumentFrequencyMax,
    ProbabilisticInverseDocumentFrequency
}

#[derive(Debug, Error, Copy, Clone)]
pub enum IdfError {
    #[error("The CorpusDocumentStatistics is seen as empty but this should not be possible.")]
    StatisticsEmptyError
}

impl IdfAlgorithm for Idf {
    type Error = IdfError;

    /// Calculates the IDF value for a single word based on the provided statistics.
    ///
    /// [number_of_documents] denotes the number of documents in the corpus
    /// [number_of_words] denote the number of distinct words in the whole corpus
    /// [word_frequency] denotes the frequency of a specific word in a corpus
    #[inline]
    fn calculate_idf<W, S: CorpusDocumentStatistics<Word=W>>(&self, statistics: & S, word: &W) -> Result<Option<f64>, IdfError> {
        match self {
            Idf::Unary => {
                Ok(Some(1.0))
            }
            other => {
                statistics
                    .word_frequency(word)
                    .map(|value| other.calculate_idf_with_word_frequency(statistics, word, value))
                    .transpose()
            }
        }

    }

    /// Calculates the IDF value for a single word based on the provided statistics.
    /// [word_frequency] denotes the frequency of a specific word in a corpus.
    ///
    /// Returns nan if the calculation is not possible.
    fn calculate_idf_with_word_frequency<W, S: CorpusDocumentStatistics<Word=W>>(&self, statistics: & S, word: &W, word_frequency: u64) -> Result<f64, IdfError> {
        match self {
            Idf::Unary => {
                Ok(1.0)
            }
            Idf::InverseDocumentFrequency => {
                Ok((statistics.document_count() as f64 / word_frequency as f64).log10())
            }
            Idf::InverseDocumentFrequencySmooth => {
                Ok((statistics.document_count() as f64 / (word_frequency as f64 + 1.0)).log10() + 1.0)
            }
            Idf::InverseDocumentFrequencyMax => {
                if let Some((_, max_value)) = statistics.iter().max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap()) {
                    Ok(((*max_value as f64)/(word_frequency as f64 + 1.0)).log10())
                } else {
                    Err(IdfError::StatisticsEmptyError)
                }
            }
            Idf::ProbabilisticInverseDocumentFrequency => {
                let word_frequency = word_frequency as f64;
                Ok((statistics.document_count() as f64 - word_frequency) / (word_frequency))
            }
        }
    }
}


/// Trait for TF Algorithm
pub trait TfAlgorithm {
    /// Calculates the TF value for a [doc].
    /// If a specific value can not be calculated
    fn calculate_tf<W, D: IntoIterator<Item=W>>(&self, doc: D) -> HashMap<W, f64> where W: Hash + Eq;
}

/// Default TF Algorithms
/// From https://en.wikipedia.org/wiki/Tf%E2%80%93idf
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum Tf {
    Binary,
    RawCount,
    TermFrequency,
    LogNormalization,
    DoubleNormalization
}

impl Tf {
    /// The implementation for Tf::RawCount, used in multiple impls.
    fn raw_count<W, D: IntoIterator<Item=W>>(doc: D) -> HashMap<W, f64> where W: Hash + Eq {
        let mut result = HashMap::new();
        for word in doc {
            match result.entry(word) {
                Entry::Occupied(mut value) => {
                    value.insert(*value.get() + 1.0);
                }
                Entry::Vacant(value) => {
                    value.insert(1.0);
                }
            }
        }
        result
    }
}

impl TfAlgorithm for Tf {

    /// Calculates the TF value for a [doc].
    /// If a specific value can not be calculated
    fn calculate_tf<W, D: IntoIterator<Item=W>>(&self, doc: D) -> HashMap<W, f64> where W: Hash + Eq {
        match self {
            Tf::Binary => {
                let mut result = HashMap::new();
                for word in doc.into_iter() {
                    result.insert(word, 1.0);
                }
                result
            }
            Tf::RawCount => {
                Self::raw_count(doc)
            }
            Tf::TermFrequency => {
                let mut result = Self::raw_count(doc);
                let divider = result.values().sum::<f64>();
                for value in result.values_mut() {
                    *value /= divider;
                }
                result
            }
            Tf::LogNormalization => {
                let mut result = Self::raw_count(doc);
                for value in result.values_mut() {
                    *value = (*value + 1.0).log10();
                }
                result
            }
            Tf::DoubleNormalization => {
                let mut result = Self::raw_count(doc);
                let max_value = result.values().max_by(|a, b| a.partial_cmp(b).unwrap()).copied();
                if let Some(max_value) = max_value {
                    for value in result.values_mut() {
                        *value = 0.5 + 0.5 * (*value/max_value);
                    }
                }
                result
            }
        }
    }
}


#[cfg(test)]
mod test {
    use itertools::Itertools;
    use lipsum::{lipsum_words_with_rng};
    use rand::{Rng, SeedableRng};
    use rand::distributions::uniform::SampleRange;
    use unicode_segmentation::UnicodeSegmentation;
    use crate::features::text_processing::corpus::CorpusStatisticsCollector;
    use crate::features::text_processing::tf_idf::{Tf, TfAlgorithm};
    use crate::features::text_processing::vectorizer::{TfIdfVectorEntry, VectorizedDocument};

    fn create_pseudo_random_statistics<R: SampleRange<usize>>(seed: u64, doc_range: R) -> CorpusStatisticsCollector<String> {
        let mut statistics = CorpusStatisticsCollector::default();

        statistics.add(lipsum_words_with_rng(rand::rngs::StdRng::seed_from_u64(seed), 30).unicode_words().map(|value| value.to_string()));
        let mut random = rand::rngs::StdRng::seed_from_u64(seed);
        for _ in 0..random.gen_range(doc_range).saturating_sub(1) {
            random = rand::rngs::StdRng::from_rng(random.clone()).unwrap();
            let doc = lipsum_words_with_rng(random.clone(), 30).unicode_words().map(|value| value.to_string()).collect_vec();
            statistics.add(doc);
        }
        return statistics
    }

    #[test]
    fn tf_idf_works(){
        let statistics = create_pseudo_random_statistics(123456, 30..50);
        println!("{statistics}");
        let vectorizer = statistics.provide_vectorizer(super::defaults::RAW_INVERSE_SMOOTH).unwrap();
        println!("\n----\n{vectorizer:?}");
        let doc_known = lipsum_words_with_rng(rand::rngs::StdRng::seed_from_u64(123456), 30).unicode_words().map(|value| value.to_string()).collect_vec();
        let vectorized = vectorizer.vectorize_document(doc_known, true);
        println!("\n----\n{vectorized:?}");
    }

    #[test]
    fn simple_tf_idf(){
        let mut statistics = CorpusStatisticsCollector::default();
        let doc1 = "it is going to rain today".unicode_words().collect_vec();
        let doc2 = "today i am not going outside".unicode_words().collect_vec();
        let doc3 = "i am going to watch the season premiere".unicode_words().collect_vec();
        statistics.add(doc1.clone());
        statistics.add(doc2.clone());
        statistics.add(doc3.clone());
        println!("{statistics}");
        let vectorizer = statistics.provide_vectorizer(super::defaults::TERM_FREQUENCY_INVERSE).unwrap();
        let doc_test = "bro it is going to rain today".unicode_words().collect_vec();
        let tf1 = Tf::TermFrequency.calculate_tf(doc_test.clone());

        fn test_vectorized(vectorized: VectorizedDocument<&str>) {
            println!("{:?}\n", vectorized);
            for TfIdfVectorEntry(word, value) in vectorized.iter() {
                match *word {
                    "to" | "today" => {
                        assert!((*value - 0.025155894150811604).abs() < f64::EPSILON, "Failed for '{}', got {}", word, *value)
                    }
                    "it" | "is" | "rain" => {
                        assert!((*value - 0.06816017924566606).abs() < f64::EPSILON, "Failed for '{}', got {}", word, *value)
                    }
                    "going" => {
                        assert!((*value - 0.0).abs() < f64::EPSILON, "Failed for '{}', got {}", word, *value)
                    }
                    unknown => {
                        panic!("Unknown TFIDF: {}", unknown)
                    }
                }
            }
        }

        // test_vectorized(unsafe{vectorizer.vectorize_tf_document(tf1.clone())});
        // test_vectorized(vectorizer.vectorize_document(doc_test.clone()));
        // test_vectorized(statistics.vectorize_document(doc_test.clone(), &super::defaults::TERM_FREQUENCY_INVERSE).unwrap());
    }
}