use crate::dts::tree::Node;

pub mod dependency;
pub mod filter;
pub mod interrupts;
pub mod reg_extractor;
pub mod sorter;
pub mod writer;

/// Visitor Trait
///
/// Allows executing logic when entering and exiting nodes during tree traversal.
pub trait Visitor {
    /// Called before entering a node.
    ///
    /// # Arguments
    /// * `_name` - The name of the node being entered.
    /// * `_node` - The node itself.
    ///
    /// # Returns
    /// * `true` - Continue traversing the children of this node.
    /// * `false` - Skip the children of this node.
    fn enter_node(&mut self, _name: &str, _node: &Node) -> bool {
        true
    }

    /// Called after exiting a node (after all children have been visited).
    ///
    /// # Arguments
    /// * `_name` - The name of the node being exited.
    /// * `_node` - The node itself.
    fn exit_node(&mut self, _name: &str, _node: &Node) {}
}

/// Tree Walker
///
/// Helper struct to traverse the Device Tree structure.
pub struct Walker;

impl Walker {
    /// Walks the tree starting from `node`.
    ///
    /// # Arguments
    /// * `node` - The current node to visit.
    /// * `name` - The name of the current node.
    /// * `visitor` - The visitor to invoke callbacks on.
    pub fn walk(node: &Node, name: &str, visitor: &mut impl Visitor) {
        // Enter hook
        if visitor.enter_node(name, node) {
            // Recursively traverse children
            if let Node::Existing { children, .. } = node {
                // Iterate over children. Order is arbitrary (HashMap).
                // If deterministic order is required, use a Sorter visitor first.
                for (child_name, child_node) in children {
                    Walker::walk(child_node, child_name, visitor);
                }
            }
        }
        // Exit hook
        visitor.exit_node(name, node);
    }
}
