use crate::dts::tree::{Cell, Data, Node, Property};
use crate::visitors::Visitor;
use std::io::Write;

/// DTS 输出访问者
/// 负责将内存中的树结构写回为符合规范的 .dts 文本格式
pub struct DtsWriter<W: Write> {
    writer: W,
    indent_level: usize,
    indent_str: String,
    is_root: bool,
}

impl<W: Write> DtsWriter<W> {
    pub fn new(writer: W, use_tabs: bool, indent_width: usize) -> Self {
        let indent_str = if use_tabs {
            "\t".to_string()
        } else {
            " ".repeat(indent_width)
        };
        Self {
            writer,
            indent_level: 0,
            indent_str,
            is_root: true,
        }
    }

    /// 辅助函数：生成当前层级的缩进字符串
    fn get_indent(&self) -> String {
        self.indent_str.repeat(self.indent_level)
    }

    /// 辅助函数：格式化属性值
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
    fn enter_node(&mut self, name: &str, node: &mut Node) -> bool {
        let indent = self.get_indent();

        // 1. 如果是根节点，先打印版本号
        if self.is_root {
            writeln!(self.writer, "/dts-v1/;").unwrap();
            writeln!(self.writer).unwrap(); // 空行
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
                children,
                labels,
                offset: _,
            } => {
                // 2. 构建节点头： label1: label2: name {
                let mut prefix = String::new();

                if !labels.is_empty() {
                    for label in labels {
                        prefix.push_str(label);
                        prefix.push_str(": ");
                    }
                }

                // 根节点名字特殊处理
                let node_name = if name.is_empty() { "/" } else { name };
                writeln!(self.writer, "{}{}{} {{", indent, prefix, node_name).unwrap();

                // 3. 打印该节点的所有属性
                let has_props = !proplist.is_empty();
                for (prop_name, prop) in proplist {
                    match prop {
                        Property::Deleted { .. } => {
                            writeln!(
                                self.writer,
                                "{}{}// Property {} deleted;",
                                indent, self.indent_str, prop_name
                            )
                            .unwrap();
                        }
                        Property::Existing { val, .. } => {
                            if let Some(values) = val {
                                if values.is_empty() {
                                    writeln!(
                                        self.writer,
                                        "{}{}{};",
                                        indent, self.indent_str, prop_name
                                    )
                                    .unwrap();
                                } else {
                                    let val_strs: Vec<String> =
                                        values.iter().map(Self::fmt_data).collect();
                                    writeln!(
                                        self.writer,
                                        "{}{}{} = {};",
                                        indent,
                                        self.indent_str,
                                        prop_name,
                                        val_strs.join(", ")
                                    )
                                    .unwrap();
                                }
                            } else {
                                writeln!(
                                    self.writer,
                                    "{}{}{};",
                                    indent, self.indent_str, prop_name
                                )
                                .unwrap();
                            }
                        }
                    }
                }
                // 如果既有属性又有子节点，添加空行
                if has_props && !children.is_empty() {
                    writeln!(self.writer).unwrap();
                }

                // 4. 手动遍历子节点，以便控制空行
                self.indent_level += 1;

                let count = children.len();
                for (i, (child_name, child_node)) in children.iter_mut().enumerate() {
                    self.enter_node(child_name, child_node);
                    self.exit_node(child_name, child_node);

                    // 如果不是最后一个子节点，打印空行
                    if i < count - 1 {
                        writeln!(self.writer).unwrap();
                    }
                }

                self.indent_level -= 1;

                false // 已经手动遍历了子节点，告诉 Walker 不要再遍历
            }
        }
    }

    fn exit_node(&mut self, _name: &str, node: &mut Node) {
        match node {
            Node::Existing { .. } => {
                let indent = self.get_indent();
                // 6. 闭合括号
                writeln!(self.writer, "{}}};", indent).unwrap();
            }
            _ => {}
        }
    }
}
