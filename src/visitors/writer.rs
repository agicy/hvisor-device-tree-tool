use crate::dts::tree::{Cell, Data, Node, Property};
use crate::visitors::Visitor;
use std::io::Write;

/// A visitor that writes the Device Tree back to a DTS format.
///
/// It traverses the tree and writes the nodes and properties to the provided writer
/// in a standard DTS text format.
pub struct DtsWriter<W> {
    writer: W,
    is_root: bool,
    indent_level: usize,
    indent_str: String,
    // Stack to track if the current node is the first child in its scope.
    // true: next child is first (no newline needed before it).
    // false: next child is not first (newline needed before it).
    first_child_stack: Vec<bool>,
}

impl<W: Write> DtsWriter<W> {
    /// Creates a new `DtsWriter`.
    ///
    /// # Arguments
    /// * `writer` - The output destination implementing `Write`.
    /// * `is_root` - Whether to treat the starting node as the root of the file
    ///               (printing version header etc.).
    pub fn new(writer: W, is_root: bool) -> Self {
        Self {
            writer,
            is_root,
            indent_level: 0,
            indent_str: "\t".to_string(),
            first_child_stack: Vec::new(),
        }
    }

    fn get_indent(&self) -> String {
        self.indent_str.repeat(self.indent_level)
    }

    fn fmt_data(data: &Data) -> String {
        match data {
            Data::String(s) => format!("\"{}\"", s),
            Data::Cells(bits, cells) => {
                let mut content = String::new();
                if *bits != 32 {
                    content.push_str(&format!("/bits/ {} ", bits));
                }
                content.push('<');
                let cell_strs: Vec<String> = cells
                    .iter()
                    .map(|c| match c {
                        Cell::Num(n) => format!("0x{:x}", n),
                        Cell::Ref(r, _) => format!("&{}", r),
                    })
                    .collect();
                content.push_str(&cell_strs.join(" "));
                content.push('>');
                content
            }
            Data::ByteArray(bytes) => {
                let content = bytes
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("[{}]", content)
            }
            Data::Reference(r, _) => {
                format!("&{}", r)
            }
        }
    }
}

impl<W: Write> Visitor for DtsWriter<W> {
    fn enter_node(&mut self, name: &str, node: &Node) -> bool {
        // 0. Handle sibling spacing
        if !self.is_root {
            if let Some(&is_first) = self.first_child_stack.last() {
                if !is_first {
                    writeln!(self.writer).unwrap();
                }
            }
            // Mark that we have processed a node in this scope
            if let Some(last) = self.first_child_stack.last_mut() {
                *last = false;
            }
        }

        let indent = self.get_indent();

        // 1. If root, print version header.
        if self.is_root {
            writeln!(self.writer, "/dts-v1/;").unwrap();
            writeln!(self.writer).unwrap(); // Empty line
            self.is_root = false;
        }

        match node {
            Node::Deleted { name: _, offset: _ } => {
                // TODO: handle deleted nodes if necessary
                return false;
            }
            Node::Existing {
                name: _,
                proplist,
                children: _,
                labels,
                offset: _,
            } => {
                // 2. Build node header: label1: label2: name {
                let mut prefix = String::new();

                if !labels.is_empty() {
                    for label in labels {
                        prefix.push_str(label);
                        prefix.push_str(": ");
                    }
                }

                // Root node special case: name is usually empty string or "/" in our parser
                // but in output we want "/ {"
                let display_name = if name.is_empty() { "/" } else { name };

                writeln!(self.writer, "{}{}{} {{", indent, prefix, display_name).unwrap();

                // 3. Push stack for this node's children
                self.first_child_stack.push(true);

                // 4. Print properties
                self.indent_level += 1;
                let indent_prop = self.get_indent();

                for (key, prop) in proplist {
                    match prop {
                        Property::Deleted { .. } => {}
                        Property::Existing {
                            name: _,
                            val,
                            offset: _,
                            labels,
                        } => {
                            let mut prop_prefix = String::new();
                            for label in labels {
                                prop_prefix.push_str(label);
                                prop_prefix.push_str(": ");
                            }

                            if let Some(v) = val {
                                if v.is_empty() {
                                    writeln!(self.writer, "{}{}{};", indent_prop, prop_prefix, key)
                                        .unwrap();
                                } else {
                                    let val_strs: Vec<String> =
                                        v.iter().map(Self::fmt_data).collect();
                                    writeln!(
                                        self.writer,
                                        "{}{}{} = {};",
                                        indent_prop,
                                        prop_prefix,
                                        key,
                                        val_strs.join(", ")
                                    )
                                    .unwrap();
                                }
                            } else {
                                writeln!(self.writer, "{}{}{};", indent_prop, prop_prefix, key)
                                    .unwrap();
                            }
                        }
                    }
                }

                if !proplist.is_empty() {
                    // If we printed properties, the next child is not "first" in terms of spacing
                    if let Some(last) = self.first_child_stack.last_mut() {
                        *last = false;
                    }
                }
            }
        }
        true
    }

    fn exit_node(&mut self, _name: &str, node: &Node) {
        if let Node::Existing { .. } = node {
            self.first_child_stack.pop();
            self.indent_level -= 1;
            let indent = self.get_indent();
            writeln!(self.writer, "{}}};", indent).unwrap();
        }
    }
}
