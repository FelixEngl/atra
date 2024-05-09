use std::borrow::Borrow;
use std::collections::{HashMap};
use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::Deref;
use itertools::Itertools;
use serde::{Deserialize, Serialize};


/// The statistics over the documents in a corpus
pub trait CorpusDocumentStatistics {
    /// A word in a corpus
    type Word;
    /// The number of documents in the corpus
    fn document_count(&self) -> u64;
    /// The number of distinct words in the corpus
    fn word_count(&self) -> u64;
    /// The number of unique words in the corpus
    fn unique_word_count(&self) -> usize;
    /// The frquency of a [word] in a corpus
    fn word_frequency(&self, word: &Self::Word) -> Option<u64>;

    /// Returns an iterator over the words and associated values
    fn iter(&self) -> impl Iterator<Item=(&Self::Word, &u64)>;
}


/// Collects the frequencies in a corpus
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct CorpusStatisticsCollector<W> where W: Hash + Eq + Clone {
    document_count: u64,
    word_count: u64,
    word_counts: HashMap<W, u64>
}

impl<W> CorpusStatisticsCollector<W> where W: Hash + Eq + Clone {
    pub fn add<D: IntoIterator<Item=W>>(&mut self, doc: D) {
        self.document_count = self.document_count.saturating_add(1);
        for value in doc {
            self.word_count = self.word_count.saturating_add(1);
            self.word_counts
                .entry(value)
                .and_modify(|value| *value=value.saturating_add(1))
                .or_insert(1);
        }
    }

    pub fn provide_vectorizer<TF: tf::Algorithm, IDF: idf::Algorithm>(&self) -> Result<DocumentVectorizer<W, TF>, IDF::Error> {
        self.word_counts
            .iter()
            .map(|(word, count)| {
                IDF::calculate_idf_with_word_frequency(self, word, *count)
                    .map(|value| (word, value))
            })
            .collect()
    }


    pub fn vectorize_document<TF: tf::Algorithm, IDF: idf::Algorithm, D: IntoIterator<Item=W>>(&self, doc: D) -> Result<VectorizedDocument<W>, IDF::Error> {
        let result = TF::calculate_tf(doc).into_iter().map(|(word, tf)|{
            match IDF::calculate_idf(self, &word) {
                Ok(Some(idf)) => {
                    Ok(TfIdfVectorEntry::TFIDF(word, tf*idf))
                }
                Ok(None) => {
                    Ok(TfIdfVectorEntry::TF(word, tf))
                }
                Err(err) => {
                    Err(err)
                }
            }
        })
            .collect::<Result<Vec<TfIdfVectorEntry<W>>, IDF::Error>>()?
            .into();

        Ok(result)

    }
}

impl<W> Display for CorpusStatisticsCollector<W> where W: Hash + Eq + Clone + ToString  {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Document Count: {}\n", self.document_count)?;
        write!(f, "Word Count: {}\n", self.word_count)?;
        write!(f, "Unique Word Count: {}\n", self.unique_word_count())?;
        write!(f, "Words:")?;
        for (word, count) in &self.word_counts {
            write!(f, "\n  {}: {count}", word.to_string())?;
        }
        Ok(())
    }
}

impl<W> CorpusDocumentStatistics for CorpusStatisticsCollector<W> where W: Hash + Eq + Clone {
    type Word = W;
    
    #[inline]
    fn document_count(&self) -> u64 {
        self.document_count
    }

    #[inline]
    fn word_count(&self) -> u64 {
        self.word_count
    }

    #[inline]
    fn unique_word_count(&self) -> usize {
        self.word_counts.len()
    }

    fn word_frequency(&self, word: &W) -> Option<u64> {
        self.word_counts.get(word).copied()
    }

    fn iter(&self) -> impl Iterator<Item=(&Self::Word, &u64)> {
        self.word_counts.iter()
    }
}




#[derive(Debug, Default)]
#[repr(transparent)]
pub struct DocumentVectorizer<'a, W, TF: tf::Algorithm> where W: Hash + Eq + Clone {
    inner: HashMap<&'a W, f64>,
    _tf: PhantomData<TF>
}

impl<'a, W, TF: tf::Algorithm> DocumentVectorizer<'a, W, TF> where W: Hash + Eq + Clone {
    fn new_with(inner: HashMap<&'a W, f64>) -> Self {
        Self{inner, _tf: PhantomData}
    }

    /// Vectorizes [doc] to a vector containing the words and associated
    pub fn vectorize_document<D: IntoIterator<Item=W>>(&self, doc: D) -> VectorizedDocument<W> {
        unsafe {
            self.vectorize_tf_document(TF::calculate_tf(doc))
        }
    }

    /// Vectorizes [doc] to a vector containing the words and associated
    pub fn vectorize_document_alt<TF2: tf::Algorithm, D: IntoIterator<Item=W>>(&self, doc: D) -> VectorizedDocument<W> {
        unsafe{
            self.vectorize_tf_document(TF2::calculate_tf(doc))
        }
    }

    /// Vectorizes [doc] to a vector containing the words.
    pub unsafe fn vectorize_tf_document(&self, doc: HashMap<W, f64>) -> VectorizedDocument<W> {
        doc
            .into_iter()
            .map(|(word, tf) | {
                match self.inner.get(&word) {
                    None => {
                        TfIdfVectorEntry::TF(word, tf)
                    }
                    Some(idf) => {
                        TfIdfVectorEntry::TFIDF(word, tf * idf)
                    }
                }
            })
            .collect_vec()
            .into()
    }
}

impl<'a, W, TF: tf::Algorithm> FromIterator<(&'a W, f64)> for DocumentVectorizer<'a, W, TF> where  W: Hash + Eq + Clone {
    fn from_iter<T: IntoIterator<Item=(&'a W, f64)>>(iter: T) -> Self {
        Self::new_with(iter.into_iter().collect())
    }
}

/// Represents the entry in a tf-idf-vector.
/// Either represents a complete TF-IDF value of only a TF value for fallback processing.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum TfIdfVectorEntry<W> {
    TF(W, f64),
    TFIDF(W, f64)
}

#[derive(Debug, Deserialize, Serialize)]
#[repr(transparent)]
pub struct VectorizedDocument<W>(Vec<TfIdfVectorEntry<W>>);

impl<W> From<Vec<TfIdfVectorEntry<W>>> for VectorizedDocument<W> {
    fn from(value: Vec<TfIdfVectorEntry<W>>) -> Self {
        Self(value)
    }
}

impl<W> Deref for VectorizedDocument<W> {
    type Target = [TfIdfVectorEntry<W>];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<W> Borrow<Vec<TfIdfVectorEntry<W>>> for VectorizedDocument<W> {
    fn borrow(&self) -> &Vec<TfIdfVectorEntry<W>> {
        &self.0
    }
}

// From https://en.wikipedia.org/wiki/Tf%E2%80%93idf
pub mod idf {
    use thiserror::Error;
    use crate::features::text_processing::tf_idf::{CorpusDocumentStatistics};

    /// Algorithm used for the Idf
    pub trait Algorithm {
        type Error;

        /// Calculates the IDF value for a single word based on the provided statistics.
        ///
        /// [number_of_documents] denotes the number of documents in the corpus
        /// [number_of_words] denote the number of distinct words in the whole corpus
        /// [word_frequency] denotes the frequency of a specific word in a corpus
        #[inline]
        fn calculate_idf<W, S: CorpusDocumentStatistics<Word=W>>(statistics: & S, word: &W) -> Result<Option<f64>, Self::Error> {
            statistics
                .word_frequency(word)
                .map(|value| Self::calculate_idf_with_word_frequency(statistics, word, value))
                .transpose()
        }

        /// Calculates the IDF value for a single word based on the provided statistics.
        /// [word_frequency] denotes the frequency of a specific word in a corpus.
        ///
        /// Returns nan if the calculation is not possible.
        fn calculate_idf_with_word_frequency<W, S: CorpusDocumentStatistics<Word=W>>(statistics: & S, word: &W, word_frequency: u64) -> Result<f64, Self::Error>;
    }

    // todo: Replace () with never type ! when stabilized
    // https://doc.rust-lang.org/std/primitive.never.html

    /// Indicates that the method never fails.
    type NeverFails = ();
    #[derive(Debug)]
    pub struct Unary;

    impl Algorithm for Unary {
        type Error = NeverFails;

        #[inline(always)]
        fn calculate_idf<W, S: CorpusDocumentStatistics<Word=W>>(_: &S, _: &W) -> Result<Option<f64>, Self::Error> {
            Ok(Some(1.0))
        }
        #[inline(always)]
        fn calculate_idf_with_word_frequency<W, S: CorpusDocumentStatistics<Word=W>>(_: &S, _: &W, _: u64) -> Result<f64, Self::Error> {
            Ok(1.0)
        }
    }
    #[derive(Debug)]
    pub struct InverseDocumentFrequency;

    impl Algorithm for InverseDocumentFrequency {
        type Error = NeverFails;
        fn calculate_idf_with_word_frequency<W, S: CorpusDocumentStatistics<Word=W>>(statistics: & S, _: &W, word_frequency: u64) -> Result<f64, Self::Error> {
            Ok((statistics.document_count() as f64 / word_frequency as f64).log10())
        }
    }
    #[derive(Debug)]
    pub struct InverseDocumentFrequencySmooth;

    impl Algorithm for InverseDocumentFrequencySmooth {
        type Error = NeverFails;
        fn calculate_idf_with_word_frequency<W, S: CorpusDocumentStatistics<Word=W>>(statistics: & S, _: &W, word_frequency: u64) -> Result<f64, Self::Error> {
            Ok((statistics.document_count() as f64 / (word_frequency as f64 + 1.0)).log10() + 1.0)
        }
    }
    #[derive(Debug)]
    pub struct InverseDocumentFrequencyMax;

    #[derive(Debug, Error, Copy, Clone)]
    #[error("The CorpusDocumentStatistics is seen as empty but this should not be possible.")]
    pub struct StatisticsEmptyError;

    impl Algorithm for InverseDocumentFrequencyMax {
        type Error = StatisticsEmptyError;
        fn calculate_idf_with_word_frequency<W, S: CorpusDocumentStatistics<Word=W>>(statistics: & S, _: &W, word_frequency: u64) -> Result<f64, Self::Error> {
            if let Some((_, max_value)) = statistics.iter().max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap()) {
                Ok(((*max_value as f64)/(word_frequency as f64 + 1.0)).log10())
            } else {
                Err(StatisticsEmptyError)
            }
        }
    }

    #[derive(Debug)]
    pub struct ProbabilisticInverseDocumentFrequency;

    impl Algorithm for ProbabilisticInverseDocumentFrequency {
        type Error = NeverFails;
        fn calculate_idf_with_word_frequency<W, S: CorpusDocumentStatistics<Word=W>>(statistics: & S, _: &W, word_frequency: u64) -> Result<f64, Self::Error> {
            let word_frequency = word_frequency as f64;
            Ok((statistics.document_count() as f64 - word_frequency) / (word_frequency))
        }
    }
}

// From https://en.wikipedia.org/wiki/Tf%E2%80%93idf
pub mod tf {
    use std::collections::hash_map::Entry;
    use std::collections::HashMap;
    use std::hash::Hash;

    /// Algorithm used for the TF
    pub trait Algorithm {
        /// Calculates the TF value for a [doc].
        /// If a specific value can not be calculated
        fn calculate_tf<W, D: IntoIterator<Item=W>>(doc: D) -> HashMap<W, f64> where W: Hash + Eq;
    }

    #[derive(Debug)]
    pub struct Binary;

    impl Algorithm for Binary {
        fn calculate_tf<W, D: IntoIterator<Item=W>>(doc: D) -> HashMap<W, f64> where W: Hash + Eq {
            let mut result = HashMap::new();
            for word in doc.into_iter() {
                result.insert(word, 1.0);
            }
            return result;
        }
    }

    #[derive(Debug)]
    pub struct RawCount;

    impl Algorithm for RawCount {
        fn calculate_tf<W, D: IntoIterator<Item=W>>(doc: D) -> HashMap<W, f64> where W: Hash + Eq {
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
    #[derive(Debug)]
    pub struct TermFrequency;

    impl Algorithm for TermFrequency {
        fn calculate_tf<W, D: IntoIterator<Item=W>>(doc: D) -> HashMap<W, f64> where W: Hash + Eq {
            let mut result = RawCount::calculate_tf(doc);
            let divider = result.values().sum::<f64>();
            for value in result.values_mut() {
                *value /= divider;
            }
            result
        }
    }

    #[derive(Debug)]
    pub struct LogNormalization;

    impl Algorithm for LogNormalization {
        fn calculate_tf<W, D: IntoIterator<Item=W>>(doc: D) -> HashMap<W, f64> where W: Hash + Eq {
            let mut result = RawCount::calculate_tf(doc);
            for value in result.values_mut() {
                *value = (*value + 1.0).log10();
            }
            result
        }
    }
    #[derive(Debug)]
    pub struct DoubleNormalization;

    impl Algorithm for DoubleNormalization {
        fn calculate_tf<W, D: IntoIterator<Item=W>>(doc: D) -> HashMap<W, f64> where W: Hash + Eq {
            let mut result = RawCount::calculate_tf(doc);
            let max_value = result.values().max_by(|a, b| a.partial_cmp(b).unwrap());
            if let Some(max_value) = max_value.copied() {
                for value in result.values_mut() {
                    *value = 0.5 + 0.5 * (*value/max_value);
                }
            }
            result
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
    use crate::features::text_processing::tf_idf::{CorpusStatisticsCollector, idf, tf, TfIdfVectorEntry, VectorizedDocument};
    use crate::features::text_processing::tf_idf::tf::Algorithm;

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
        let vectorizer = statistics.provide_vectorizer::<tf::RawCount, idf::InverseDocumentFrequency>().unwrap();
        println!("\n----\n{vectorizer:?}");
        let doc_known = lipsum_words_with_rng(rand::rngs::StdRng::seed_from_u64(123456), 30).unicode_words().map(|value| value.to_string()).collect_vec();
        let vectorized = vectorizer.vectorize_document(doc_known);
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
        let vectorizer = statistics.provide_vectorizer::<tf::TermFrequency, idf::InverseDocumentFrequency>().unwrap();
        let doc_test = "bro it is going to rain today".unicode_words().collect_vec();
        let tf1 = tf::TermFrequency::calculate_tf(doc_test.clone());

        fn test_vectorized(vectorized: VectorizedDocument<&str>) {
            println!("{:?}\n", vectorized);
            for value in vectorized.iter() {
                match value {
                    TfIdfVectorEntry::TF(word, value) => {
                        assert_eq!("bro", *word);
                        assert!((*value - 1.0/7.0).abs() < f64::EPSILON, "Failed for '{}', got {}", word, *value)
                    }
                    TfIdfVectorEntry::TFIDF(word, value) => {
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
            }
        }

        test_vectorized(unsafe{vectorizer.vectorize_tf_document(tf1.clone())});
        test_vectorized(vectorizer.vectorize_document(doc_test.clone()));
        test_vectorized(statistics.vectorize_document::<tf::TermFrequency, idf::InverseDocumentFrequency, _>(doc_test.clone()).unwrap());
    }
}