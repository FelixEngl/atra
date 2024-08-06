pub mod crawl_results;
pub mod warc;

use std::error::Error;

pub trait ContentStore<T, Q> {
    type StoredInfo;
    type StoreError: Error;
    type RetrieveError: Error;

    /// Allows to store some kind of content.
    async fn store(&self, value: &T) -> Result<Self::StoredInfo, Self::StoreError>;

    /// Retrieves a value for some kind of query
    async fn retrieve(&self, query: &Q) -> Result<Option<T>, Self::RetrieveError>;
}