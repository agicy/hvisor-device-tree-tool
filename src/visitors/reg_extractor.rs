use crate::dts::tree::{Cell, Data, Node, Property};
use crate::visitors::Visitor;

pub struct RegExtractor {
    // 存储 (address-cells, size-cells)
    cell_stack: Vec<(u32, u32)>,
}

impl RegExtractor {
    pub fn new() -> Self {
        Self {
            // 根节点默认值，通常是 1, 1，或者是 2, 1 (64bit)
            cell_stack: vec![(1, 1)],
        }
    }

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

    fn get_cells(node: &Node, parent_default: (u32, u32)) -> (u32, u32) {
        let addr = Self::get_u32_prop(node, "#address-cells").unwrap_or(parent_default.0);
        let size = Self::get_u32_prop(node, "#size-cells").unwrap_or(parent_default.1);
        (addr, size)
    }
}

impl Visitor for RegExtractor {
    fn enter_node(&mut self, name: &str, node: &mut Node) -> bool {
        // 1. 获取当前节点定义的 cells，这会影响 *子节点* 的 reg 解析
        // 但 reg 属性本身的解析依赖于 *父节点* 的 cells。
        let (parent_addr_cells, parent_size_cells) = *self.cell_stack.last().unwrap();

        // 2. 提取并打印 Reg
        // reg is prop with "reg" name.
        if let Node::Existing { proplist, .. } = node {
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

                let chunk_size = (parent_addr_cells + parent_size_cells) as usize;
                if chunk_size > 0 && !all_cells.is_empty() {
                    for chunk in all_cells.chunks(chunk_size) {
                        if chunk.len() < chunk_size {
                            break;
                        }
                        let start_parts = &chunk[0..parent_addr_cells as usize];
                        let size_parts = &chunk[parent_addr_cells as usize..];

                        let start_str = start_parts
                            .iter()
                            .map(|c| format!("{:08x}", c))
                            .collect::<String>();
                        let size_str = size_parts
                            .iter()
                            .map(|c| format!("{:08x}", c))
                            .collect::<String>();

                        println!("{} 0x{} 0x{}", name, start_str, size_str);
                    }
                }
            }
        }

        // 3. 计算当前节点给自己子节点设定的 cells，并入栈
        let new_cells = Self::get_cells(node, *self.cell_stack.last().unwrap());
        self.cell_stack.push(new_cells);

        true
    }

    fn exit_node(&mut self, _name: &str, _node: &mut Node) {
        self.cell_stack.pop();
    }
}
