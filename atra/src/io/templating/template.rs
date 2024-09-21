// Copyright 2024. Felix Engl
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

use std::fmt::{Display, Formatter, Write};
use crate::io::serial::SerialProvider;
use crate::io::templating::{FileNameTemplateArgs, FileNameTemplateElement, RecoverInstructionElement, TemplateError};

/// A template for a filename
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct FileNameTemplate {
    parts: Vec<FileNameTemplateElement>,
}


impl FileNameTemplate {
    pub fn new(parts: Vec<FileNameTemplateElement>) -> Self {
        Self { parts }
    }

    /// Writes the template element to `f`. Returns true if some kind of content was written.
    pub fn write(
        &self,
        f: &mut impl Write,
        serial_provider: &SerialProvider,
        args: Option<&FileNameTemplateArgs>,
    ) -> Result<bool, TemplateError> {
        let mut wrote_something = false;
        for value in self.parts.iter() {
            wrote_something |= value.write(f, serial_provider, args)?;
        }
        Ok(wrote_something)
    }

    /// Writes the template element to `f`. Returns true if some kind of content was written.
    pub fn write_current(
        &self,
        f: &mut impl Write,
        serial_provider: &SerialProvider,
        args: Option<&FileNameTemplateArgs>,
    ) -> Result<bool, TemplateError> {
        let mut wrote_something = false;
        for value in self.parts.iter() {
            wrote_something |= value.write_current(f, serial_provider, args)?;
        }
        Ok(wrote_something)
    }

    pub fn recover(&mut self, information: &[(usize, RecoverInstructionElement)]) {
        for (i, instruction) in information {
            if let Some(target) = self.parts.get_mut(*i) {
                instruction.set(target);
            } else {
                log::warn!("Missing the {i} field in the template. Continue recovery.")
            }
        }
    }

    pub fn get_recover_information(&self) -> Vec<(usize, RecoverInstructionElement)> {
        self.parts.iter().enumerate().filter_map(|(i, value)| {
            match value {
                FileNameTemplateElement::Dynamic(value) => {
                    Some((i, RecoverInstructionElement::Dynamic(value.clone())))
                }
                FileNameTemplateElement::CustomSerial(value) => {
                    Some((i, RecoverInstructionElement::CustomSerial(value.current_serial())))
                }
                FileNameTemplateElement::FileNameTemplate(value) => {
                    Some((i, RecoverInstructionElement::SubElement(value.get_recover_information())))
                }
                _ => {None}
            }
        }).collect()
    }
}

impl Display for FileNameTemplate {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Ok(_) = self.write(f, &SerialProvider::NoSerial, None) {
            Ok(())
        } else {
            Err(std::fmt::Error)
        }
    }
}

impl From<Vec<FileNameTemplateElement>> for FileNameTemplate {
    fn from(value: Vec<FileNameTemplateElement>) -> Self {
        Self::new(value)
    }
}