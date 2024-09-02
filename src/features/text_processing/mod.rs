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

use crate::features::text_processing::corpus::CorpusStatisticsCollector;
use crate::features::text_processing::tf_idf::{IdfAlgorithm, TfAlgorithm, TfIdf};
use crate::features::tokenizing::tokenizer::Tokenizer;

pub mod tf_idf;
pub mod corpus;
pub mod vectorizer;

pub fn create_vectorizer<I: Iterator<Item=T>, T: AsRef<str>, Tf: TfAlgorithm, Idf: IdfAlgorithm>(mut train_data: I, tokenizer: &Tokenizer, tf_idf: TfIdf<Tf, Idf>) -> Result<vectorizer::DocumentVectorizer<String, Tf,Idf>, Idf::Error> {
    let mut corpus_statistics = CorpusStatisticsCollector::default();
    for document in train_data {
        let tokens = tokenizer.tokenize(document.as_ref());
        corpus_statistics.add(tokens);
    }
    Ok(corpus_statistics.provide_vectorizer(tf_idf)?)
}