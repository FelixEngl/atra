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
use liblinear::parameter::traits::SetRegressionLossSensitivity;
use liblinear::{Model, Parameters};
use liblinear::model::traits::TrainableModel;
use liblinear::solver::{L2R_L2LOSS_SVR};
use liblinear::util::{TrainingInput};
use crate::features::gdpr::error::LibLinearError;

pub fn train(labels: Vec<f64>, features: Vec<Vec<(u32, f64)>>) -> Result<Model<L2R_L2LOSS_SVR>, LibLinearError> {
    let data = TrainingInput::from_sparse_features(
        labels,
        features
    )?;

    let mut params = Parameters::<L2R_L2LOSS_SVR>::default();
    params
        .regression_loss_sensitivity(0.1)
        .stopping_tolerance(0.0003)
        .constraints_violation_cost(10.0);


    Ok(Model::train(&data, &params)?)
}