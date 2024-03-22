//Copyright 2024 Felix Engl
//
//Licensed under the Apache License, Version 2.0 (the "License");
//you may not use this file except in compliance with the License.
//You may obtain a copy of the License at
//
//    http://www.apache.org/licenses/LICENSE-2.0
//
//Unless required by applicable law or agreed to in writing, software
//distributed under the License is distributed on an "AS IS" BASIS,
//WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//See the License for the specific language governing permissions and
//limitations under the License.

use liblinear;
use liblinear::SolverType;
use liblinear::util::{TrainingInput, TrainingInputError};
use thiserror::Error;
use failure::Error;

#[derive(Debug, Error)]
pub enum SVMError {
    Training(#[from] TrainingInputError),
    Build(#[from] Error)
}

pub fn train(labels: Vec<f64>, features: Vec<Vec<(u32, f64)>>) -> Result<impl liblinear::LibLinearModel, anyhow::Error> {
    let mut builder = liblinear::Builder::new();
    builder.problem()
        .input_data(
            TrainingInput::from_sparse_features(
                labels,
                features
            )?
        );

    builder.parameters()
        .solver_type(SolverType::L2R_L2LOSS_SVC)
        .regression_loss_sensitivity(0.1)
        .stopping_criterion(0.0003)
        .constraints_violation_cost(10.0);

    Ok(builder.build_model()?)
}