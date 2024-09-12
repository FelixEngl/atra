// Copyright 2024 Felix Engl
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::io::serial::{DefaultSerialProvider, NoSerial, SerialProvider};
use crate::io::templating::{FileNameTemplate, FileNameTemplateArgs, TemplateError};
use camino::{Utf8Path, Utf8PathBuf};
use std::fmt::Debug;

/// Provides a unique path under the `root`
#[derive(Debug, Clone)]
pub struct UniquePathProvider<S = DefaultSerialProvider> {
    root: Utf8PathBuf,
    serial_provider: S,
}

impl<S> UniquePathProvider<S> {
    pub fn root(&self) -> &Utf8Path {
        &self.root
    }

    pub fn with_provider(root: impl AsRef<Utf8Path>, provider: S) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            serial_provider: provider,
        }
    }

    pub fn with_template(self, template: FileNameTemplate) -> UniquePathProviderWithTemplate<S> {
        UniquePathProviderWithTemplate::new(self, template)
    }
}

impl<S> UniquePathProvider<S>
where
    S: Default,
{
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
        Self::with_provider(root, NoSerial::<u8>::default())
    }
}

impl<S> UniquePathProvider<S>
where
    S: SerialProvider,
{
    pub fn provide_path(
        &self,
        template: &FileNameTemplate,
        args: Option<&FileNameTemplateArgs>,
    ) -> Result<Utf8PathBuf, TemplateError> {
        let mut name = String::new();
        template.write(&mut name, &self.serial_provider, args)?;
        Ok(self.root.join(name))
    }
}

/// Provides a path based on a given template
#[derive(Debug, Clone)]
pub struct UniquePathProviderWithTemplate<S = DefaultSerialProvider> {
    provider: UniquePathProvider<S>,
    template: FileNameTemplate,
}

impl<S> UniquePathProviderWithTemplate<S> {
    pub fn new(provider: UniquePathProvider<S>, template: FileNameTemplate) -> Self {
        Self { provider, template }
    }

    pub fn root(&self) -> &Utf8Path {
        &self.provider.root
    }
}

impl<S> UniquePathProviderWithTemplate<S>
where
    S: SerialProvider,
{
    pub fn provide_path(
        &self,
        args: Option<&FileNameTemplateArgs>,
    ) -> Result<Utf8PathBuf, TemplateError> {
        self.provider.provide_path(&self.template, args)
    }

    pub fn provide_path_with_args(
        &self,
        args: &FileNameTemplateArgs,
    ) -> Result<Utf8PathBuf, TemplateError> {
        self.provider.provide_path(&self.template, Some(args))
    }

    pub fn provide_path_no_args(&self) -> Result<Utf8PathBuf, TemplateError> {
        self.provider.provide_path(&self.template, None)
    }
}
