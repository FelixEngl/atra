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

use serde::{Deserialize, Serialize};
use crate::io::serial::SerialValue;
use crate::io::templating::FileNameTemplateElement;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename = "R")]
pub struct RecoverInstruction {
    #[serde(rename = "SS")]
    pub serial_state: Option<SerialValue>,
    #[serde(rename = "I")]
    pub instructions: Vec<(usize, RecoverInstructionElement)>
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename = "E")]
pub enum RecoverInstructionElement {
    #[serde(rename = "D")]
    Dynamic(String),
    #[serde(rename = "C")]
    CustomSerial(Option<SerialValue>),
    #[serde(rename = "S")]
    SubElement(Vec<(usize, RecoverInstructionElement)>)
}

impl RecoverInstructionElement {
    pub fn set(&self, target: &mut FileNameTemplateElement) {
        match (self, target) {
            (Self::Dynamic(value), FileNameTemplateElement::Dynamic(targ)) => {
                let _ = std::mem::replace(targ, value.clone());
            }
            (Self::CustomSerial(value), FileNameTemplateElement::CustomSerial(target))=> {
                if let Some(value) = value {
                    target.set_current_serial(value.clone())
                }
            }
            (Self::SubElement(value), FileNameTemplateElement::FileNameTemplate(targ)) => {
                targ.recover(value.as_slice())
            }
            (from, to) => {
                log::warn!("Can not set a {from:?} as a {to:?}!")
            }
        }
    }
}