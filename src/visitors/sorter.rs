use crate::dts::tree::{Cell, Data, Node, Property};
use crate::visitors::Visitor;
use indexmap::IndexMap;
use petgraph::algo::toposort;
use petgraph::graph::DiGraph;
use std::collections::HashMap;

/// A visitor that sorts nodes based on their dependencies (references).
///
/// It rebuilds the tree such that if Node A references Node B, Node B appears
/// before Node A in the child list of their parent.
pub struct SortByReference {
    stack: Vec<Node>,
    /// The root of the sorted tree.
    pub root: Option<Node>,
}

enum Dependency {
    Label(String),
}

impl SortByReference {
    /// Creates a new `SortByReference` visitor.
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            root: None,
        }
    }

    // Recursively collect all labels in the subtree rooted at `node`
    fn collect_labels_recursive(node: &Node, labels_out: &mut Vec<String>) {
        if let Node::Existing {
            labels, children, ..
        } = node
        {
            labels_out.extend(labels.clone());
            for child in children.values() {
                Self::collect_labels_recursive(child, labels_out);
            }
        }
    }

    // Topologically sorts the children map based on dependencies.
    fn topological_sort_map(children: &IndexMap<String, Node>) -> IndexMap<String, Node> {
        let mut graph = DiGraph::<&str, ()>::new();
        let mut indices = HashMap::new();

        for name in children.keys() {
            indices.insert(name.as_str(), graph.add_node(name.as_str()));
        }

        // Collect phandles and labels (including deep labels).
        // We map each label found in a child's subtree to that child's name.
        let mut label_map = HashMap::new(); // label -> child_name

        for (name, node) in children {
            let mut labels = Vec::new();
            Self::collect_labels_recursive(node, &mut labels);

            for label in labels {
                label_map.insert(label, name.as_str());
            }
        }

        // Add edges based on dependencies.
        for (name, node) in children {
            let source_idx = indices[name.as_str()];
            // Find dependencies
            let deps = Self::find_dependencies(node);
            for dep in deps {
                let target_name = match dep {
                    Dependency::Label(l) => label_map.get(&l).copied(),
                };

                if let Some(target_name) = target_name {
                    if let Some(target_idx) = indices.get(target_name) {
                        // Avoid self-cycles (if a node depends on itself or its own subtree)
                        if source_idx != *target_idx {
                            graph.update_edge(*target_idx, source_idx, ());
                        }
                    }
                }
            }
        }

        match toposort(&graph, None) {
            Ok(sorted_indices) => {
                let mut sorted_map = IndexMap::new();
                for idx in sorted_indices {
                    let name = graph[idx];
                    if let Some((k, v)) = children.get_key_value(name) {
                        sorted_map.insert(k.clone(), v.clone());
                    }
                }
                // Add any nodes that might have been missed (e.g. disconnected? toposort handles all)
                if sorted_map.len() != children.len() {
                    for (k, v) in children {
                        if !sorted_map.contains_key(k) {
                            sorted_map.insert(k.clone(), v.clone());
                        }
                    }
                }
                sorted_map
            }
            Err(_) => {
                // Cycle detected or error, return original order
                eprintln!(
                    "Warning: Cycle detected during topological sort, keeping original order."
                );
                children.clone()
            }
        }
    }

    // Finds dependencies (referenced labels) in a node's properties.
    fn find_dependencies(node: &Node) -> Vec<Dependency> {
        let mut deps = Vec::new();
        // Common dependency properties
        let dep_props = [
            "clocks",
            "interrupt-parent",
            "power-domains",
            "phys",
            "resets",
            "dmas",
            "iommus",
            "mboxes",
            "interconnects",
            // Add others if needed
        ];

        if let Node::Existing { proplist, .. } = node {
            for (prop_name, prop) in proplist {
                if let Property::Existing {
                    val: Some(data), ..
                } = prop
                {
                    if dep_props.contains(&prop_name.as_str()) {
                        for d in data {
                            match d {
                                Data::Reference(label, _) => {
                                    deps.push(Dependency::Label(label.clone()));
                                }
                                Data::Cells(_, cells) => {
                                    for c in cells {
                                        if let Cell::Ref(label, _) = c {
                                            deps.push(Dependency::Label(label.clone()));
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        deps
    }
}

impl Visitor for SortByReference {
    fn enter_node(&mut self, _name: &str, node: &Node) -> bool {
        // Clone node (empty children)
        let mut new_node = node.clone();
        if let Node::Existing { children, .. } = &mut new_node {
            *children = IndexMap::new();
        }
        self.stack.push(new_node);

        true
    }

    fn exit_node(&mut self, name: &str, _node: &Node) {
        if let Some(mut new_node) = self.stack.pop() {
            // Sort children before adding to parent
            if let Node::Existing { children, .. } = &mut new_node {
                if !children.is_empty() {
                    let sorted = Self::topological_sort_map(children);
                    *children = sorted;
                }
            }

            if self.stack.is_empty() {
                self.root = Some(new_node);
            } else {
                let parent = self.stack.last_mut().unwrap();
                if let Node::Existing { children, .. } = parent {
                    // Use the name from the argument, which is the key in the parent map
                    children.insert(name.to_string(), new_node);
                }
            }
        }
    }
}
