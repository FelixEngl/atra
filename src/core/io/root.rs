use camino::{Utf8Path, Utf8PathBuf};

/// Used to set the root for a path if possible
pub trait RootSetter {
    fn set_root_if_possible(&self, path: impl AsRef<Utf8Path>) -> Option<Utf8PathBuf>;

    /// Sets the root if the [path] does not exists and setting the root is possible
    fn set_root_if_not_exists(&self, path: impl AsRef<Utf8Path>) -> Utf8PathBuf {
        let path = path.as_ref();
        if !path.exists() {
            if let Some(new_path) = self.set_root_if_possible(path) {
                new_path
            } else {
                path.to_path_buf()
            }
        } else {
            path.to_path_buf()
        }
    }
}