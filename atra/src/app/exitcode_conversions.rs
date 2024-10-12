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

use std::process::ExitCode;
use crate::app::atra::AtraRunError;
use crate::app::consumer::GlobalError;
use crate::app::instruction::InstructionError;
use crate::contexts::local::LocalContextInitError;

impl Into<ExitCode> for InstructionError {
    fn into(self) -> ExitCode {
        match self {
            InstructionError::IOError(_) => {
                ExitCode::from(2)
            }
            InstructionError::ConfigError(_) => {
                ExitCode::from(3)
            }
            InstructionError::ConfigDeserializationError(_) => {
                ExitCode::from(4)
            }
            InstructionError::RootAlreadyExists(_) => {
                ExitCode::from(5)
            }
            InstructionError::DumbSerialisationError(_) => {
                ExitCode::from(70)
            }
        }
    }
}

impl Into<ExitCode> for AtraRunError {
    fn into(self) -> ExitCode {
        match self {
            AtraRunError::ContextInitialisation(value) => {
                match value {
                    LocalContextInitError::Io(_) | LocalContextInitError::IoWithPath(_) => {
                        11
                    }
                    LocalContextInitError::OpenDB(_) => {
                        12
                    }
                    LocalContextInitError::RocksDB(_) => {
                        13
                    }
                    LocalContextInitError::QueueFile(_) => {
                        14
                    }
                    LocalContextInitError::BlackList(_) => {
                        15
                    }
                    LocalContextInitError::Svm(_) => {
                        16
                    }
                    LocalContextInitError::WebGraph(_) => {
                        17
                    }
                    LocalContextInitError::Serde(_) => {
                        18
                    }
                }.into()
            }
            AtraRunError::WorkerContextInitialisation(_) => {
                ExitCode::from(40)
            }
            AtraRunError::Queue(_) => {
                ExitCode::from(50)
            }
            AtraRunError::Crawl(value) => {
                match value {
                    GlobalError::SlimCrawlError(_) => {
                        101
                    }
                    GlobalError::LinkHandling(_) => {
                        102
                    }
                    GlobalError::LinkState(_) => {
                        103
                    }
                    GlobalError::LinkStateDatabase(_) => {
                        104
                    }
                    GlobalError::CrawlWriteError(_) => {
                        105
                    }
                    GlobalError::QueueError(_) => {
                        106
                    }
                    GlobalError::ClientError(_) => {
                        107
                    }
                    GlobalError::RequestError(_) => {
                        108
                    }
                    GlobalError::IOError(_) => {
                        109
                    }

                }.into()
            }

        }
    }
}


