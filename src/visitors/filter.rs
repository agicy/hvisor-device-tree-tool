use crate::dts::tree::Node;
use crate::visitors::Visitor;

pub struct NodeFilter<F>
where
    F: Fn(&Node) -> bool,
{
    predicate: F,
}

impl<F> NodeFilter<F>
where
    F: Fn(&Node) -> bool,
{
    pub fn new(predicate: F) -> Self {
        Self { predicate }
    }
}

impl<F> Visitor for NodeFilter<F>
where
    F: Fn(&Node) -> bool,
{
    fn enter_node(&mut self, _name: &str, node: &mut Node) -> bool {
        if let Node::Existing { children, .. } = node {
            // Remove children that satisfy the predicate (should be filtered out)
            children.retain(|_name, child_node| !(self.predicate)(child_node));
        }
        true
    }
}
