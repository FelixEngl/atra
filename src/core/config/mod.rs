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

pub mod crawl;
pub mod system;
pub mod configs;
pub mod session;
pub mod paths;


pub use system::SystemConfig;
pub use crawl::CrawlConfig;
#[allow(unused_imports)]
pub use session::SessionConfig;
#[allow(unused_imports)]
pub use paths::PathsConfig;
pub use configs::Configs;
pub use crawl::BudgetSetting;