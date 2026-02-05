use crate::dts::tree::Node;

pub mod dependency;
pub mod filter;
pub mod interrupts;
pub mod reg_extractor;
pub mod sorter;
pub mod writer;

/// 访问者 Trait
/// 允许在进入节点和退出节点时执行逻辑
pub trait Visitor {
    /// 进入节点前调用
    /// 返回 false 表示不再深入该节点的子节点
    fn enter_node(&mut self, _name: &str, _node: &mut Node) -> bool {
        true
    }

    /// 退出节点后调用（子节点已访问完毕）
    fn exit_node(&mut self, _name: &str, _node: &mut Node) {}
}

/// 遍历器
pub struct Walker;

impl Walker {
    pub fn walk(node: &mut Node, name: &str, visitor: &mut impl Visitor) {
        // Enter 钩子
        if visitor.enter_node(name, node) {
            // 递归遍历子节点
            if let Node::Existing { children, .. } = node {
                // sort keys to ensure deterministic order if needed, or just iterate
                // HashMap iteration order is arbitrary. For deterministic output, usually we want sorted.
                // But Walker is generic.
                // Let's iterate. If order matters, use Sorter visitor first.
                for (child_name, child_node) in children {
                    Walker::walk(child_node, child_name, visitor);
                }
            }
        }
        // Exit 钩子
        visitor.exit_node(name, node);
    }
}
