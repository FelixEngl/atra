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

use crate::app::ApplicationMode;
use crate::config::Config;
use crate::seed::SeedDefinition;

/// The kind of instruction provided by the args.
#[derive(Debug)]
pub enum Instruction {
    RunInstruction(RunInstruction),
    Nothing,
}

/// The instruction to run atra.
#[derive(Debug)]
pub struct RunInstruction {
    pub mode: ApplicationMode,
    pub config: Config,
    pub seeds: Option<SeedDefinition>,
    pub recover_mode: bool,
}
