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

use ego_tree::iter::{Edge, Traverse};
use ego_tree::NodeRef;
use scraper::Node;

/// Iterator over descendent text nodes.
#[derive(Debug, Clone)]
pub struct Text<'a> {
    inner: Traverse<'a, Node>,
}

impl<'a> Text<'a> {
    pub fn new(inner: Traverse<'a, Node>) -> Self {
        Self { inner }
    }

    pub fn traverse(node: &NodeRef<'a, Node>) -> Self {
        Self::new(node.traverse())
    }
}

impl<'a> Iterator for Text<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        for edge in &mut self.inner {
            if let Edge::Open(node) = edge {
                if let Node::Text(ref text) = node.value() {
                    return Some(&**text);
                }
            }
        }
        None
    }
}
