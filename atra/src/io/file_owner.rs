use camino::Utf8Path;
use crate::io::errors::ErrorWithPath;

/// The owner of some kind of file. This trait allows to perform various actions on the read/write process.
pub trait FileOwner {
    /// Returns true if [path] is in use.
    #[allow(dead_code)] fn is_in_use<Q: AsRef<Utf8Path>>(&self, path: Q) -> bool;

    /// Waits until the target [path] is free or frees it in some way.
    async fn wait_until_free_path<Q: AsRef<Utf8Path>>(&self, target: Q) -> Result<(), ErrorWithPath>;
}