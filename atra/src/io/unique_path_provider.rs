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

use crate::io::serial::{SerialProvider};
use crate::io::templating::{FileNameTemplate, FileNameTemplateArgs, RecoverInstruction, TemplateError};
use camino::{Utf8Path, Utf8PathBuf};
use std::fmt::Debug;

/// Provides a unique path under the `root`
#[derive(Debug, Clone)]
pub struct UniquePathProvider {
    root: Utf8PathBuf,
    serial_provider: SerialProvider,
}

impl UniquePathProvider {
    #[cfg(test)]
    pub fn root(&self) -> &Utf8Path {
        &self.root
    }

    pub fn new(root: impl AsRef<Utf8Path>, serial_provider: SerialProvider) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            serial_provider,
        }
    }

    pub fn with_template(self, template: FileNameTemplate) -> UniquePathProviderWithTemplate {
        UniquePathProviderWithTemplate::new(self, template)
    }

    #[cfg(test)]
    pub fn without_provider(root: impl AsRef<Utf8Path>) -> Self {
        Self::new(root, SerialProvider::NoSerial)
    }
}



impl UniquePathProvider
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

    pub fn current_path(
        &self,
        template: &FileNameTemplate,
        args: Option<&FileNameTemplateArgs>,
    ) -> Result<Utf8PathBuf, TemplateError> {
        let mut name = String::new();
        template.write_current(&mut name, &self.serial_provider, args)?;
        Ok(self.root.join(name))
    }
}

/// Provides a path based on a given template
#[derive(Debug, Clone)]
pub struct UniquePathProviderWithTemplate {
    provider: UniquePathProvider,
    template: FileNameTemplate,
}

impl UniquePathProviderWithTemplate {
    pub fn new(provider: UniquePathProvider, template: FileNameTemplate) -> Self {
        Self { provider, template }
    }

    pub fn root(&self) -> &Utf8Path {
        &self.provider.root
    }

    #[cfg(test)]
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

    pub fn current_path_with_args(
        &self,
        args: &FileNameTemplateArgs
    ) -> Result<Utf8PathBuf, TemplateError> {
        self.provider.current_path(&self.template, Some(args))
    }

    pub fn current_path_no_args(&self) -> Result<Utf8PathBuf, TemplateError> {
        self.provider.current_path(&self.template, None)
    }


    pub fn get_recover_information(&self) -> RecoverInstruction {
        RecoverInstruction {
            serial_state: self.provider.serial_provider.current_serial(),
            instructions: self.template.get_recover_information()
        }
    }

    pub fn recover(&mut self, recover_instruction: RecoverInstruction) {
        if let Some(serial_state) = recover_instruction.serial_state {
            self.provider.serial_provider.set_current_serial(serial_state)
        }
        self.template.recover(recover_instruction.instructions.as_slice())
    }
}

#[cfg(test)]
mod test {
    use camino::Utf8PathBuf;
    use crate::io::serial::{SerialProviderKind, SerialValue};
    use crate::io::templating::{file_name_template, RecoverInstruction, RecoverInstructionElement};
    use crate::io::unique_path_provider::{UniquePathProvider, UniquePathProviderWithTemplate};

    fn provide_template() -> UniquePathProviderWithTemplate {
        let template_base = file_name_template!("hello_world" _ "whatever").unwrap();
        let worker_id = 12;
        let recrawl_iteration = 1;

        let template = file_name_template!(ref template_base _ worker_id _ "rc" _ recrawl_iteration _ serial ".warc")
            .unwrap();

        UniquePathProviderWithTemplate::new(
            UniquePathProvider::new(
                "test/tata",
                SerialProviderKind::Long.into()
            ),
            template
        )
    }

    #[test]
    fn can_properly_provide_current(){
        let next = provide_template();
        assert_eq!(
            Utf8PathBuf::from("test/tata\\hello_world_whatever_12_rc_1_0.warc"),
            next.current_path_no_args().unwrap(),
            "Failed with current!"
        );

        assert_eq!(
            Utf8PathBuf::from("test/tata\\hello_world_whatever_12_rc_1_0.warc"),
            next.provide_path_no_args().unwrap(),
            "Failed to provide!"
        );

        assert_eq!(
            Utf8PathBuf::from("test/tata\\hello_world_whatever_12_rc_1_1.warc"),
            next.current_path_no_args().unwrap(),
            "Failed with current!"
        );


        assert_eq!(
            Utf8PathBuf::from("test/tata\\hello_world_whatever_12_rc_1_1.warc"),
            next.provide_path_no_args().unwrap(),
            "Failed to provide!"
        );

        assert_eq!(
            Utf8PathBuf::from("test/tata\\hello_world_whatever_12_rc_1_2.warc"),
            next.current_path_no_args().unwrap(),
            "Failed with current!"
        );

        assert_eq!(
            Utf8PathBuf::from("test/tata\\hello_world_whatever_12_rc_1_2.warc"),
            next.current_path_no_args().unwrap(),
            "Failed with current!"
        );
    }

    #[test]
    fn backup_works() {
        let next = provide_template();
        next.provide_path_no_args().unwrap();
        next.provide_path_no_args().unwrap();
        next.provide_path_no_args().unwrap();
        let expected = RecoverInstruction {
            serial_state: Some(SerialValue::Long(3)),
            instructions: vec![
                (0, RecoverInstructionElement::SubElement(vec![])),
                (2, RecoverInstructionElement::Dynamic("12".to_string())),
                (6, RecoverInstructionElement::Dynamic("1".to_string()))
            ]
        };
        assert_eq!(expected, next.get_recover_information());

        let last = next.current_path_no_args().unwrap();
        assert_eq!(last, next.current_path_no_args().unwrap());

        let mut recovered = provide_template();
        recovered.recover(next.get_recover_information());
        assert_eq!(expected, recovered.get_recover_information());
        assert_eq!(last, recovered.current_path_no_args().unwrap());
    }
}