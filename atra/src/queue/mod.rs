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

mod raw;
mod url;

pub use raw::errors::QueueError;
pub use raw::implementation::RawAgingQueueFile;
pub use raw::AgingQueueElement;
pub use raw::EnqueueCalled;
pub use url::element::UrlQueueElement;
pub use url::queue::UrlQueueWrapper;
pub use url::result::*;
pub use url::UrlQueue;
pub use url::PollWaiterRef;
pub use url::PollWaiter;