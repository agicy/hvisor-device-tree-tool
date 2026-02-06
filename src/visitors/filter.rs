use crate::dts::tree::Node;
use crate::visitors::Visitor;
use indexmap::IndexMap;

/// A visitor that filters nodes based on a predicate.
///
/// It constructs a new tree containing only the nodes (and their subtrees)
/// for which the predicate returns `false`. If the predicate returns `true`,
/// the node and its children are excluded.
pub struct NodeFilter<F>
where
    F: Fn(&Node) -> bool,
{
    predicate: F,
    // Stack to build the new tree.
    // Each element is a Node (the partially constructed node).
    stack: Vec<Node>,
    // The resulting root of the new tree.
    pub root: Option<Node>,
}

impl<F> NodeFilter<F>
where
    F: Fn(&Node) -> bool,
{
    /// Creates a new `NodeFilter` with the given predicate.
    ///
    /// # Arguments
    /// * `predicate` - A function that returns `true` if the node should be filtered out (excluded),
    ///                 and `false` if it should be kept.
    pub fn new(predicate: F) -> Self {
        Self {
            predicate,
            stack: Vec::new(),
            root: None,
        }
    }
}

impl<F> Visitor for NodeFilter<F>
where
    F: Fn(&Node) -> bool,
{
    fn enter_node(&mut self, _name: &str, node: &Node) -> bool {
        // If predicate returns true, it means "remove" (based on previous logic: if status==disabled return true).
        if (self.predicate)(node) {
            return false; // Skip children, don't add to stack
        }

        // Clone the node but with empty children
        let mut new_node = node.clone();
        if let Node::Existing { children, .. } = &mut new_node {
            *children = IndexMap::new();
        }

        self.stack.push(new_node);
        true
    }

    fn exit_node(&mut self, _name: &str, node: &Node) {
        // If we skipped this node (predicate true), do nothing
        if (self.predicate)(node) {
            return;
        }

        if let Some(new_node) = self.stack.pop() {
            if self.stack.is_empty() {
                // This was the root
                self.root = Some(new_node);
            } else {
                // Add to parent
                let parent = self.stack.last_mut().unwrap();
                if let Node::Existing { children, .. } = parent {
                    // Use the name passed from Walker (the key in the parent map)
                    children.insert(_name.to_string(), new_node);
                }
            }
        }
    }
}
