use crate::dts::tree::{Data, Node, Property};
use crate::visitors::Visitor;

pub struct FilterDisabled;

impl FilterDisabled {
    pub fn new() -> Self {
        Self
    }

    fn is_disabled(node: &Node) -> bool {
        if let Node::Existing { proplist, .. } = node {
            if let Some(Property::Existing {
                val: Some(data), ..
            }) = proplist.get("status")
            {
                for d in data {
                    if let Data::String(s) = d {
                        if s == "disabled" {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

impl Visitor for FilterDisabled {
    fn enter_node(&mut self, _name: &str, node: &mut Node) -> bool {
        if let Node::Existing { children, .. } = node {
            // Remove disabled children
            children.retain(|_name, child_node| !Self::is_disabled(child_node));
        }
        true
    }
}
