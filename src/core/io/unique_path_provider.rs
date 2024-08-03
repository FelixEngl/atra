use std::fmt::{Debug, Display};
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;
use crate::core::io::templating::{DefaultSerialProvider, FileNameTemplate, NoSerial, SerialProvider, TemplateError};

/// Provides a unique path under the `root`
pub struct UniquePathProvider<S = DefaultSerialProvider> {
    root: Utf8PathBuf,
    serial_provider: S,
    template: FileNameTemplate
}

impl<S> UniquePathProvider<S> {
    pub fn with_provider(root: impl AsRef<Utf8Path>, provider: S) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            serial_provider: provider,
            template: todo!()
        }
    }
}

impl<S> UniquePathProvider<S> where S: Default {
    pub fn new_with(root: impl AsRef<Utf8Path>) -> Self {
        Self::with_provider(root, S::default())
    }
}

impl UniquePathProvider {
    pub fn new(root: impl AsRef<Utf8Path>) -> Self {
        Self::new_with(root)
    }
}


impl UniquePathProvider<NoSerial<u8>> {
    pub fn without_provider(root: impl AsRef<Utf8Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            serial_provider: NoSerial::default(),
            template: todo!()
        }
    }
}

impl<S> UniquePathProvider<S> where S: SerialProvider {
    pub fn provide_path(&self, template: &FileNameTemplate) -> Result<Utf8PathBuf, TemplateError> {
        let mut name = String::new();
        template.write(&mut name, &self.serial_provider)?;
        Ok(self.root.join(name))
    }
}

