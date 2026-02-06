use crate::dts::tree::{Cell, Data, Node, Property};
use crate::visitors::Visitor;
use std::collections::HashMap;
use std::fmt::Write;

/// Represents a dependency relationship between two nodes.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DependencyInfo {
    /// The path of the child node (the dependent).
    pub child_path: String,
    /// The label of the parent node (the dependency).
    pub parent_label: String,
}

/// Extracts dependency information from the Device Tree.
///
/// This visitor identifies dependencies based on common properties like `clocks`,
/// `interrupt-parent`, etc., which reference other nodes.
pub struct DependencyExtractor {
    // Stack of current path components during traversal.
    path_stack: Vec<String>,
    // Collected dependency relationships.
    dependencies: Vec<DependencyInfo>,
    // Map of node labels to their full paths.
    label_map: HashMap<String, String>,
}

impl DependencyExtractor {
    /// Creates a new `DependencyExtractor`.
    pub fn new() -> Self {
        Self {
            path_stack: Vec::new(),
            dependencies: Vec::new(),
            label_map: HashMap::new(),
        }
    }

    /// Generates the output string describing the dependencies.
    ///
    /// The output format is "child_path -> parent_path".
    /// If the parent path cannot be resolved from the label, it uses "label:<label_name>".
    pub fn output(&self) -> String {
        // Sort dependencies to ensure deterministic output.
        // Note: We cannot modify self.dependencies directly here because output takes &self.
        let mut deps = self.dependencies.iter().collect::<Vec<_>>();
        deps.sort();

        let mut output = String::new();
        for dep in deps {
            let target_path = self
                .label_map
                .get(&dep.parent_label)
                .cloned()
                .unwrap_or_else(|| format!("label:{}", dep.parent_label));
            writeln!(output, "{} -> {}", dep.child_path, target_path).unwrap();
        }
        output
    }

    // Constructs the current full path from the stack.
    fn get_current_path(&self) -> String {
        if self.path_stack.is_empty() {
            return "/".to_string();
        }
        let mut path = String::new();
        for p in &self.path_stack {
            if p == "/" {
                path.push('/');
            } else {
                if !path.ends_with('/') {
                    path.push('/');
                }
                path.push_str(p);
            }
        }
        if path.is_empty() {
            "/".to_string()
        } else {
            path
        }
    }

    // Checks for common dependency properties in the current node.
    fn check_dependencies(&mut self, node: &Node, current_path: &str) {
        if let Node::Existing { proplist, .. } = node {
            // List of common properties that imply a dependency.
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
            ];

            for prop_name in dep_props {
                if let Some(Property::Existing {
                    val: Some(data), ..
                }) = proplist.get(prop_name)
                {
                    for d in data {
                        match d {
                            Data::Reference(name, _) => {
                                self.dependencies.push(DependencyInfo {
                                    child_path: current_path.to_string(),
                                    parent_label: name.clone(),
                                });
                            }
                            Data::Cells(_, cells) => {
                                for c in cells {
                                    if let Cell::Ref(name, _) = c {
                                        self.dependencies.push(DependencyInfo {
                                            child_path: current_path.to_string(),
                                            parent_label: name.clone(),
                                        });
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
}

impl Visitor for DependencyExtractor {
    fn enter_node(&mut self, name: &str, node: &Node) -> bool {
        // Update path stack.
        // Root node name is usually "" or "/".
        let node_name = if name.is_empty() { "/" } else { name };
        self.path_stack.push(node_name.to_string());

        let current_path = self.get_current_path();

        // Record label -> path mapping.
        if let Node::Existing { labels, .. } = node {
            for label in labels {
                self.label_map.insert(label.clone(), current_path.clone());
            }
        }

        // Collect dependencies.
        self.check_dependencies(node, &current_path);

        true
    }

    fn exit_node(&mut self, _name: &str, _node: &Node) {
        self.path_stack.pop();
    }
}
