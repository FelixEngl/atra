use camino::{Utf8Path, Utf8PathBuf};
use sealed::sealed;
use thiserror::Error;

type Result<T> = std::result::Result<T, ErrorWithPath>;

/// An error with a path
#[derive(Debug, Error)]
#[error("Failed for file '{path}' with:\n{source}")]
pub struct ErrorWithPath {
    path: Utf8PathBuf,
    #[source]
    source: std::io::Error
}

impl ErrorWithPath {
    #[inline]
    pub fn new(path: Utf8PathBuf, source: std::io::Error) -> Self {
        Self { path, source }
    }
}


/// Helper trait to convert Result-enums to Result-enums with FSAError
#[sealed]
pub trait ToErrorWithPath<T> where Self: Sized {
    /// Converts a normal IO error to an error with a path
    fn to_error_with_path<P: AsRef<Utf8Path>>(self, path: P) -> Result<T>;

    // /// Maps a normal IO error to an error with a path
    // fn map_to_error_with_path<P: AsRef<Utf8Path>, F: FnOnce() -> P>(self, path_provider: F) -> Result<T> {
    //     self.to_error_with_path(path_provider())
    // }
}

#[sealed]
impl<T> ToErrorWithPath<T> for std::result::Result<T, std::io::Error> {
    fn to_error_with_path<P: AsRef<Utf8Path>>(self, path: P) -> Result<T> {
        self.map_err(|e| ErrorWithPath::new(path.as_ref().to_path_buf(), e))
    }

    // fn map_to_error_with_path<P: AsRef<Utf8Path>, F: FnOnce() -> P>(self, path_provider: F) -> Result<T> {
    //     self.map_err(|value| ErrorWithPath::new(path_provider().as_ref().to_path_buf(), value))
    // }
}