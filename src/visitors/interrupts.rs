use crate::dts::tree::{Cell, Data, Node, Property};
use crate::visitors::Visitor;

pub struct InterruptsExtractor {
    // 追踪 interrupt-parent phandle
    parent_stack: Vec<u32>,
}

impl InterruptsExtractor {
    pub fn new() -> Self {
        Self {
            parent_stack: vec![0],
        } // 0 表示无 parent 或 root
    }

    fn get_phandle_prop(node: &Node, name: &str) -> Option<u32> {
        if let Node::Existing { proplist, .. } = node {
            if let Some(Property::Existing {
                val: Some(data), ..
            }) = proplist.get(name)
            {
                // <&phandle> might be Cell::Ref or Data::Reference depending on parser
                // or <1> if manually set
                for d in data {
                    match d {
                        Data::Cells(_, cells) => {
                            if let Some(Cell::Num(val)) = cells.first() {
                                return Some(*val as u32);
                            }
                            // If it's a ref, we can't resolve it here easily without a lookup table
                        }
                        Data::Reference(_, _) => {
                            // Needs resolution
                        }
                        _ => {}
                    }
                }
            }
        }
        None
    }

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
}

impl Visitor for InterruptsExtractor {
    fn enter_node(&mut self, name: &str, node: &mut Node) -> bool {
        // 1. 检查是否有 interrupt-parent 覆盖
        let current_parent = if let Some(p) = Self::get_phandle_prop(node, "interrupt-parent") {
            p
        } else {
            *self.parent_stack.last().unwrap()
        };

        // 2. 提取 interrupts
        if let Some(irqs) = Self::get_cells_prop(node, "interrupts") {
            // 这里同样需要知道 interrupt-controller 的 #interrupt-cells 才能正确分组
            // 为简化，直接打印原始数组
            let irq_str = irqs
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            let parent_str = if current_parent != 0 {
                format!("phandle:{}", current_parent)
            } else {
                "none".into()
            };

            println!("{} <{}> parent:{}", name, irq_str, parent_str);
        }

        self.parent_stack.push(current_parent);
        true
    }

    fn exit_node(&mut self, _name: &str, _node: &mut Node) {
        self.parent_stack.pop();
    }
}
