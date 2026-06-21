use crate::dts::tree::{Cell, Data, Node, Property};
use crate::visitors::Visitor;
use std::collections::HashMap;
use std::fmt::Write;

// Internal enum to track interrupt parent references.
#[derive(Debug, Clone, PartialEq)]
enum ParentRef {
    Phandle(u32),
    Name(String),
    None, // Root
}

/// Information about an extracted interrupt.
#[derive(Debug)]
pub struct InterruptInfo {
    /// The full path of the node defining the interrupt.
    pub alias: String,
    /// The name of the node.
    pub name: String,
    /// The number of cells per interrupt specifier.
    pub chunk_size: usize,
    /// The raw cell values in hex format.
    pub cells_hex: String,
    /// The full path of the interrupt parent.
    pub parent_alias: String,
}

/// A visitor that extracts interrupt information from the Device Tree.
///
/// It tracks `interrupt-parent` properties and resolves interrupt controllers
/// to produce a flattened list of interrupts.
pub struct InterruptsExtractor {
    // Tracks the current interrupt-parent scope.
    parent_stack: Vec<ParentRef>,
    // Stack to track the current path.
    path_stack: Vec<String>,

    // Stores controller information:
    // key: String (label name) OR String (phandle number as string)
    // value: (#interrupt-cells, full_path)
    controllers: HashMap<String, (u32, String)>,

    /// The collected list of interrupts.
    pub interrupts: Vec<InterruptInfo>,
}

impl InterruptsExtractor {
    /// Creates a new `InterruptsExtractor`.
    pub fn new() -> Self {
        Self {
            parent_stack: vec![ParentRef::None],
            path_stack: Vec::new(),
            controllers: HashMap::new(),
            interrupts: Vec::new(),
        }
    }

    /// Creates an extractor with all interrupt controllers indexed before the
    /// main walk. This is required for DTS files that use numeric phandles or
    /// reference controllers before their nodes are defined.
    pub fn from_root(root: &Node) -> Self {
        let mut extractor = Self::new();
        extractor.collect_controllers(root, "/");
        extractor
    }

    /// Generates a CSV-like output string of the extracted interrupts.
    ///
    /// Format: full_path,name,chunk_size,cells_hex,parent_full_path
    pub fn output(&self) -> String {
        let mut output = String::new();
        for irq in &self.interrupts {
            writeln!(
                output,
                "{},{},{},{},{}",
                irq.alias, irq.name, irq.chunk_size, irq.cells_hex, irq.parent_alias
            )
            .unwrap();
        }
        output
    }

    // Helper to extract a u32 property value.
    fn get_u32_prop(node: &Node, name: &str) -> Option<u32> {
        if let Node::Existing { proplist, .. } = node {
            if let Some(Property::Existing {
                val: Some(data), ..
            }) = proplist.get(name)
            {
                if let Some(Data::Cells(_, cells)) = data.first() {
                    if let Some(Cell::Num(val)) = cells.first() {
                        return Some(*val as u32);
                    }
                }
            }
        }
        None
    }

    fn collect_controllers(&mut self, node: &Node, path: &str) {
        if let Node::Existing {
            proplist,
            labels,
            children,
            ..
        } = node
        {
            if proplist.contains_key("interrupt-controller") {
                let cells = Self::get_u32_prop(node, "#interrupt-cells").unwrap_or(1);

                for label in labels {
                    self.controllers
                        .insert(label.clone(), (cells, path.to_string()));
                }

                for prop_name in ["phandle", "linux,phandle"] {
                    if let Some(phandle) = Self::get_u32_prop(node, prop_name) {
                        self.controllers
                            .insert(phandle.to_string(), (cells, path.to_string()));
                    }
                }
            }

            for (child_name, child) in children {
                let child_path = if path == "/" {
                    format!("/{}", child_name)
                } else {
                    format!("{}/{}", path, child_name)
                };
                self.collect_controllers(child, &child_path);
            }
        }
    }

    // Helper to determine the interrupt parent of a node.
    fn get_parent_ref(node: &Node) -> Option<ParentRef> {
        if let Node::Existing { proplist, .. } = node {
            if let Some(Property::Existing {
                val: Some(data), ..
            }) = proplist.get("interrupt-parent")
            {
                for d in data {
                    match d {
                        Data::Cells(_, cells) => {
                            if let Some(c) = cells.first() {
                                match c {
                                    Cell::Num(val) => return Some(ParentRef::Phandle(*val as u32)),
                                    Cell::Ref(name, _) => {
                                        return Some(ParentRef::Name(name.clone()));
                                    }
                                }
                            }
                        }
                        Data::Reference(name, _) => return Some(ParentRef::Name(name.clone())),
                        _ => {}
                    }
                }
            }
        }
        None
    }

    // Helper to extract the `interrupts` property values.
    fn get_cells_prop(node: &Node, name: &str) -> Option<Vec<u64>> {
        if let Node::Existing { proplist, .. } = node {
            if let Some(Property::Existing {
                val: Some(data), ..
            }) = proplist.get(name)
            {
                let mut result = Vec::new();
                for d in data {
                    if let Data::Cells(_, cells) = d {
                        for c in cells {
                            if let Cell::Num(val) = c {
                                result.push(*val);
                            }
                        }
                    }
                }
                if !result.is_empty() {
                    return Some(result);
                }
            }
        }
        None
    }

    fn get_cells_with_refs_prop(node: &Node, name: &str) -> Option<Vec<Cell>> {
        if let Node::Existing { proplist, .. } = node {
            if let Some(Property::Existing {
                val: Some(data), ..
            }) = proplist.get(name)
            {
                let mut result = Vec::new();
                for d in data {
                    if let Data::Cells(_, cells) = d {
                        result.extend(cells.iter().cloned());
                    }
                }
                if !result.is_empty() {
                    return Some(result);
                }
            }
        }
        None
    }

    fn resolve_parent(&self, parent: &ParentRef) -> Option<&(u32, String)> {
        match parent {
            ParentRef::Phandle(p) => self.controllers.get(&p.to_string()),
            ParentRef::Name(n) => self.controllers.get(n),
            ParentRef::None => None,
        }
    }

    fn push_interrupt(&mut self, full_path: &str, name: &str, chunk: &[u64], parent_alias: &str) {
        let cells_str = chunk
            .iter()
            .map(|c| format!("0x{:x}", c))
            .collect::<Vec<_>>()
            .join(",");

        self.interrupts.push(InterruptInfo {
            alias: full_path.to_string(),
            name: name.to_string(),
            chunk_size: chunk.len(),
            cells_hex: cells_str,
            parent_alias: parent_alias.to_string(),
        });
    }

    fn push_interrupts_extended(&mut self, full_path: &str, name: &str, cells: &[Cell]) {
        let mut index = 0;
        while index < cells.len() {
            let parent = match &cells[index] {
                Cell::Num(phandle) => ParentRef::Phandle(*phandle as u32),
                Cell::Ref(label, _) => ParentRef::Name(label.clone()),
            };
            index += 1;

            let Some((chunk_size, parent_alias)) = self.resolve_parent(&parent) else {
                break;
            };

            let chunk_size = *chunk_size as usize;
            if index + chunk_size > cells.len() {
                break;
            }

            let mut chunk = Vec::with_capacity(chunk_size);
            let mut valid = true;
            for cell in &cells[index..index + chunk_size] {
                match cell {
                    Cell::Num(value) => chunk.push(*value),
                    Cell::Ref(_, _) => {
                        valid = false;
                        break;
                    }
                }
            }
            if valid {
                let parent_alias = parent_alias.clone();
                self.push_interrupt(full_path, name, &chunk, &parent_alias);
            }
            index += chunk_size;
        }
    }
}

impl Visitor for InterruptsExtractor {
    fn enter_node(&mut self, name: &str, node: &Node) -> bool {
        self.path_stack.push(name.to_string());

        // Construct full path
        let full_path = if self.path_stack.len() == 1 && self.path_stack[0] == "/" {
            "/".to_string()
        } else {
            format!("/{}", self.path_stack[1..].join("/"))
        };

        // 0. Collect controller information. `from_root` already does this
        // before the walk, but keep this path so callers using `new` still get
        // useful results when controllers appear before their users.
        let is_controller = if let Node::Existing { proplist, .. } = node {
            proplist.contains_key("interrupt-controller")
        } else {
            false
        };

        if is_controller {
            let cells = Self::get_u32_prop(node, "#interrupt-cells").unwrap_or(1);
            let phandle = Self::get_u32_prop(node, "phandle");

            // Register by label(s)
            if let Node::Existing { labels, .. } = node {
                for label in labels {
                    self.controllers
                        .insert(label.clone(), (cells, full_path.clone()));
                }
            }
            // Register by phandle
            if let Some(p) = phandle {
                self.controllers
                    .insert(p.to_string(), (cells, full_path.clone()));
            }
            if let Some(p) = Self::get_u32_prop(node, "linux,phandle") {
                self.controllers
                    .insert(p.to_string(), (cells, full_path.clone()));
            }
        }

        // 1. Determine the current interrupt-parent
        let current_parent = if let Some(p) = Self::get_parent_ref(node) {
            p
        } else {
            self.parent_stack.last().cloned().unwrap_or(ParentRef::None)
        };

        // 2. Extract interrupts
        if let Some(irqs) = Self::get_cells_prop(node, "interrupts") {
            // Resolve parent
            let parent_info = self.resolve_parent(&current_parent);

            if let Some((cells, parent_alias)) = parent_info {
                let chunk_size = *cells as usize;
                let parent_alias = parent_alias.clone();
                if chunk_size > 0 {
                    for chunk in irqs.chunks(chunk_size) {
                        if chunk.len() == chunk_size {
                            self.push_interrupt(&full_path, name, chunk, &parent_alias);
                        }
                    }
                }
            } else {
                // Parent not found or not resolved yet
                // println!("Warning: parent not found for {}", name);
            }
        }

        if let Some(cells) = Self::get_cells_with_refs_prop(node, "interrupts-extended") {
            self.push_interrupts_extended(&full_path, name, &cells);
        }

        self.parent_stack.push(current_parent);
        true
    }

    fn exit_node(&mut self, _name: &str, _node: &Node) {
        self.parent_stack.pop();
        self.path_stack.pop();
    }
}
