use crate::dts::tree::{Cell, Data, Node, Property};
use crate::visitors::Visitor;

/// Information about an extracted register region.
#[derive(Debug)]
pub struct RegInfo {
    /// The full path of the node.
    pub alias: String,
    /// The start address in hex format.
    pub start_hex: String,
    /// The size in hex format.
    pub size_hex: String,
}

/// A visitor that extracts register information (`reg` property) from nodes.
///
/// It tracks `#address-cells` and `#size-cells` to correctly parse the `reg` property.
pub struct RegExtractor {
    // Stack to track address-cells and size-cells from parent nodes.
    address_cells_stack: Vec<u32>,
    size_cells_stack: Vec<u32>,
    // Stack to track the current path.
    path_stack: Vec<String>,
    /// The collected register information.
    pub regs: Vec<RegInfo>,
}

impl RegExtractor {
    /// Creates a new `RegExtractor`.
    pub fn new() -> Self {
        Self {
            // Default root values: usually 1, 1, or 2, 1 (64bit).
            address_cells_stack: vec![1],
            size_cells_stack: vec![1],
            path_stack: Vec::new(),
            regs: Vec::new(),
        }
    }

    /// Generates a CSV-like output string of the extracted registers.
    ///
    /// Format: full_path,start_hex,size_hex
    pub fn output(&self) -> String {
        let mut output = String::new();
        for reg in &self.regs {
            use std::fmt::Write;
            if !reg.size_hex.is_empty() {
                writeln!(
                    output,
                    "{},{},{}",
                    reg.alias, reg.start_hex, reg.size_hex
                )
                .unwrap();
            } else {
                writeln!(output, "{},{},", reg.alias, reg.start_hex).unwrap();
            }
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
}

impl Visitor for RegExtractor {
    fn enter_node(&mut self, name: &str, node: &Node) -> bool {
        self.path_stack.push(name.to_string());

        // 1. Get parent's cells settings (as default).
        let parent_addr_cells = *self.address_cells_stack.last().unwrap();
        let parent_size_cells = *self.size_cells_stack.last().unwrap();

        // 2. Determine cells used by current node FOR ITS CHILDREN.
        // The current node's reg property is parsed using PARENT's cells.
        // The current node's #address-cells/#size-cells define the context for ITS CHILDREN.
        
        let child_addr_cells =
            Self::get_u32_prop(node, "#address-cells").unwrap_or(parent_addr_cells);
        let child_size_cells =
            Self::get_u32_prop(node, "#size-cells").unwrap_or(parent_size_cells);

        // 3. Extract and print Reg using PARENT's cells.
        if let Node::Existing {
            proplist, ..
        } = node
        {
            if let Some(Property::Existing {
                val: Some(data), ..
            }) = proplist.get("reg")
            {
                // Flatten all cells
                let mut all_cells = Vec::new();
                for d in data {
                    if let Data::Cells(_, cells) = d {
                        for c in cells {
                            if let Cell::Num(val) = c {
                                all_cells.push(*val);
                            }
                        }
                    }
                }

                // Use parent's cells to parse current node's reg
                let chunk_size = (parent_addr_cells + parent_size_cells) as usize;
                if chunk_size > 0 && !all_cells.is_empty() {
                    for chunk in all_cells.chunks(chunk_size) {
                        if chunk.len() < chunk_size {
                            break;
                        }
                        let start_parts = &chunk[0..parent_addr_cells as usize];
                        let size_parts = &chunk[parent_addr_cells as usize..];

                        // Format: alias,name,start,size
                        // Construct full path
                        let full_path = if self.path_stack.len() == 1 && self.path_stack[0] == "/" {
                            "/".to_string()
                        } else {
                            format!("/{}", self.path_stack[1..].join("/"))
                        };

                        // Format helper
                        let format_hex = |parts: &[u64]| -> String {
                            if parts.is_empty() {
                                return "".to_string();
                            }
                            let mut val: u128 = 0;
                            for &p in parts {
                                val = (val << 32) | (p as u128);
                            }
                            format!("0x{:x}", val)
                        };

                        let start_hex = format_hex(start_parts);
                        let size_hex = format_hex(size_parts);

                        self.regs.push(RegInfo {
                            alias: full_path,
                            start_hex,
                            size_hex,
                        });
                    }
                }
            }
        }

        // 4. Push current cells to stack for children.
        self.address_cells_stack.push(child_addr_cells);
        self.size_cells_stack.push(child_size_cells);

        true
    }

    fn exit_node(&mut self, _name: &str, _node: &Node) {
        self.address_cells_stack.pop();
        self.size_cells_stack.pop();
        self.path_stack.pop();
    }
}
