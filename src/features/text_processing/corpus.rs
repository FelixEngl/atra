use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::hash::Hash;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use crate::features::text_processing::tf_idf::{IdfAlgorithm, TfIdf};
use crate::features::text_processing::vectorizer::{DocumentVectorizer, DocumentVectorizerNoTf};

/// The statistics over the documents in a corpus
pub trait CorpusDocumentStatistics {
    /// A word in a corpus
    type Word;
    /// The number of documents in the corpus
    fn document_count(&self) -> u64;
    /// The number of distinct words in the corpus
    #[allow(dead_code)] fn word_count(&self) -> u64;
    /// The number of unique words in the corpus
    fn unique_word_count(&self) -> usize;
    /// The frquency of a [word] in a corpus
    fn word_frequency(&self, word: &Self::Word) -> Option<u64>;

    /// Returns an iterator over the words and associated values
    fn iter(&self) -> impl Iterator<Item=(&Self::Word, &u64)>;
}

///
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct CorpusStatisticsCollectorVersion {
    document_count: u64,
    word_count: u64,
}

/// Collects the frequencies in a corpus
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(bound(serialize = "W: Serialize + Hash + Eq", deserialize = "W: DeserializeOwned + Hash + Eq"))]
pub struct CorpusStatisticsCollector<W> {
    document_count: u64,
    word_count: u64,
    word_counts: HashMap<W, u64>
}

impl<W> CorpusStatisticsCollector<W> {
    pub fn version(&self) -> CorpusStatisticsCollectorVersion {
        CorpusStatisticsCollectorVersion {
            word_count: self.word_count,
            document_count: self.document_count,
        }
    }
}

impl<W> CorpusStatisticsCollector<W> where W: Hash + Eq + Clone {
    pub fn provide_vectorizer_without_tf<Idf: IdfAlgorithm>(&self, idf: Idf) -> Result<DocumentVectorizerNoTf<W, Idf>, Idf::Error> {
        self.provide_vectorizer(TfIdf::new((), idf))
    }

    pub fn provide_vectorizer<Tf, Idf: IdfAlgorithm>(&self, tf_idf: TfIdf<Tf, Idf>) -> Result<DocumentVectorizer<W, Tf, Idf>, Idf::Error> {
        let result = self.word_counts.iter().map(|(word, count)| {
            tf_idf.idf.calculate_idf_with_word_frequency(self, word, *count).map(|value| (word.clone(), value))
        }).collect::<Result<_, _>>()?;
        Ok(
            DocumentVectorizer::from_idf_mapping(
                result,
                tf_idf
            )
        )
    }
}

impl<W> CorpusStatisticsCollector<W> where W: Hash + Eq {
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



    // pub fn vectorize_document<Tf: TfAlgorithm, Idf: IdfAlgorithm, D: IntoIterator<Item=W>>(&self, doc: D, TfIdf{tf, idf}: &TfIdf<Tf, Idf>) -> Result<VectorizedDocument<W>, Idf::Error> {
    //     let result = tf.calculate_tf(doc).into_iter().map(|(word, tf)|{
    //         match idf.calculate_idf(self, &word) {
    //             Ok(Some(idf)) => {
    //                 Ok(TfIdfVectorEntry::tf_idf(word, tf*idf))
    //             }
    //             Ok(None) => {
    //                 Ok(TfIdfVectorEntry::tf(word, tf))
    //             }
    //             Err(err) => {
    //                 Err(err)
    //             }
    //         }
    //     })
    //         .collect::<Result<Vec<TfIdfVectorEntry<W>>, Idf::Error>>()?
    //         .into();
    //
    //     Ok(result)
    // }
}

impl<W> Display for CorpusStatisticsCollector<W> where W: Hash + Eq + ToString  {
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

impl<W> CorpusDocumentStatistics for CorpusStatisticsCollector<W> where W: Hash + Eq {
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