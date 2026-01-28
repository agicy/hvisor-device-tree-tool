use std::io::{self, Write};
use crate::dts::tree::{Node, PropertyValue};
use crate::visitors::Visitor;

/// DTS 输出访问者
/// 负责将内存中的树结构写回为符合规范的 .dts 文本格式
pub struct DtsWriter<W: Write> {
    writer: W,
    indent_level: usize,
    indent_str: String,
    is_root: bool,
}

impl<W: Write> DtsWriter<W> {
    pub fn new(writer: W, indent_spaces: usize) -> Self {
        Self {
            writer,
            indent_level: 0,
            indent_str: " ".repeat(indent_spaces),
            is_root: true,
        }
    }

    /// 辅助函数：生成当前层级的缩进字符串
    fn get_indent(&self) -> String {
        self.indent_str.repeat(self.indent_level)
    }

    /// 辅助函数：格式化属性值
    /// 根据 PropertyValue 的枚举类型还原 DTS 语法
    fn fmt_value(val: &PropertyValue) -> String {
        match val {
            PropertyValue::String(s) => format!("\"{}\"", s), // 字符串加引号
            PropertyValue::StringList(list) => {
                // 字符串列表: "a", "b"
                list.iter()
                    .map(|s| format!("\"{}\"", s))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
            PropertyValue::CellArray(cells) => {
                // Cell 数组: <0x1 0x2>
                // 这里为了通用性，默认使用十六进制输出
                let content = cells.iter()
                    .map(|c| format!("0x{:x}", c))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("<{}>", content)
            }
            PropertyValue::Bytestring(bytes) => {
                // 字节数组: [00 FF AA]
                let content = bytes.iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("[{}]", content)
            }
            PropertyValue::Reference(r) => {
                // 引用: &label
                format!("&{}", r)
            }
            // 处理 phandle 引用，如果有单独的类型
            PropertyValue::Phandle(handle) => {
                format!("<0x{:x}>", handle) 
            }
            // 如果库中有其他复杂类型（如 Cell 中混合引用），需要更复杂的匹配
            _ => "/* unknown value */".to_string(),
        }
    }
}

impl<W: Write> Visitor for DtsWriter<W> {
    fn enter_node(&mut self, name: &str, node: &Node) -> bool {
        let indent = self.get_indent();

        // 1. 如果是根节点，先打印版本号
        if self.is_root {
            writeln!(self.writer, "/dts-v1/;").unwrap();
            writeln!(self.writer).unwrap(); // 空行
            self.is_root = false;
        }

        // 2. 构建节点头： label1: label2: name {
        let mut prefix = String::new();
        
        // 打印 Labels (例如 uart0: ...)
        // 注意：node.labels 是 device_tree_source 解析出来的标签列表
        if !node.labels.is_empty() {
            for label in &node.labels {
                prefix.push_str(label);
                prefix.push_str(": ");
            }
        }

        // 根节点名字特殊处理
        let node_name = if name.is_empty() { "/" } else { name };
        
        writeln!(self.writer, "{}{}{} {{", indent, prefix, node_name).unwrap();

        // 3. 打印该节点的所有属性
        // 这里的 properties 是 IndexMap，所以顺序是稳定的
        for (prop_name, prop_val) in &node.properties {
            let val_str = Self::fmt_value(prop_val);
            // 某些属性可能没有值（如 boolean 属性 present）
            // 需要检查 PropertyValue 是否表示空
            if val_str.is_empty() {
                 writeln!(self.writer, "{}{}{};", indent, self.indent_str, prop_name).unwrap();
            } else {
                 writeln!(self.writer, "{}{}{} = {};", indent, self.indent_str, prop_name, val_str).unwrap();
            }
        }

        // 4. 增加缩进，准备处理子节点
        self.indent_level += 1;
        
        true // 继续遍历子节点
    }

    fn exit_node(&mut self, _name: &str, _node: &Node) {
        // 5. 减少缩进
        if self.indent_level > 0 {
            self.indent_level -= 1;
        }

        let indent = self.get_indent();
        // 6. 闭合括号
        writeln!(self.writer, "{}}};", indent).unwrap();
    }
}