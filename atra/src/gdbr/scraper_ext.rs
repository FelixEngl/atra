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
