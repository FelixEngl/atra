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

/// The builder used by this crawler
pub type ClientBuilder = reqwest_middleware::ClientBuilder;

/// The client used by this crawler
pub type Client = reqwest_middleware::ClientWithMiddleware;

/// The client error
pub type ClientError = reqwest_middleware::Error;

/// The client result
pub type Result<T> = std::result::Result<T, ClientError>;
