use crate::dts::tree::{Cell, Data, Node, Property};
use crate::visitors::Visitor;
use indexmap::IndexMap;
use petgraph::algo::toposort;
use petgraph::graph::DiGraph;
use std::collections::HashMap;

pub struct SortByReference {
    stack: Vec<Node>,
    pub root: Option<Node>,
}

enum Dependency {
    Label(String),
}

impl SortByReference {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            root: None,
        }
    }

    // Recursively collect all labels in the subtree rooted at `node`
    fn collect_labels_recursive(node: &Node, labels_out: &mut Vec<String>) {
        if let Node::Existing { labels, children, .. } = node {
            labels_out.extend(labels.clone());
            for child in children.values() {
                Self::collect_labels_recursive(child, labels_out);
            }
        }
    }

    fn topological_sort_map(children: &IndexMap<String, Node>) -> IndexMap<String, Node> {
        let mut graph = DiGraph::<&str, ()>::new();
        let mut indices = HashMap::new();

        for name in children.keys() {
            indices.insert(name.as_str(), graph.add_node(name.as_str()));
        }

        // Collect phandles and labels (including deep labels)
        // We map each label found in a child's subtree to that child's name.
        let mut label_map = HashMap::new(); // label -> child_name

        for (name, node) in children {
            let mut labels = Vec::new();
            Self::collect_labels_recursive(node, &mut labels);
            
            for label in labels {
                label_map.insert(label, name.as_str());
            }
        }

        // Add edges
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
                // Just in case
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
                // Optionally print a warning
                eprintln!("Warning: Cycle detected during topological sort, keeping original order.");
                children.clone()
            }
        }
    }

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
        // We need to visit children manually to rebuild the tree in exit_node
        // Visitor trait automatically visits children, but here we are building a new tree
        // The Visitor implementation in lib.rs iterates over children.
        // Our enter_node pushes a new node to the stack.
        // Then walk visits children.
        // enter_node for child pushes child to stack.
        // exit_node for child pops child and adds to parent (which is on top of stack).
        
        true
    }

    fn exit_node(&mut self, name: &str, _node: &Node) {
        if let Some(mut new_node) = self.stack.pop() {
            // Sort children before adding to parent
            if let Node::Existing { children, .. } = &mut new_node {
                // If children were populated by recursive calls, they are already in the `children` map.
                // But wait, the standard Walker doesn't populate our `new_node`.
                // It just calls enter/exit.
                // So when we are in exit_node(child), we need to add `new_node` (the child) to `parent`.
                // The `parent` is the current top of the stack.
                
                // However, `new_node` here has empty children because we cleared them in `enter_node`.
                // And since `enter_node` cleared them, and we didn't add anything back...
                // Ah, the logic in `exit_node` below adds the popped node to the parent.
                // So `parent` (on stack) accumulates children.
                
                // BUT: `new_node` itself (the one we just popped) should have accumulated ITS children
                // during the visits between its enter and exit.
                // Let's verify:
                // enter(parent) -> push parent (empty children)
                //   enter(child) -> push child (empty children)
                //   exit(child) -> pop child. parent is on stack. add child to parent.
                // exit(parent) -> pop parent.
                
                // So `new_node` (the popped one) DOES have children populated.
                // Now we want to sort them.
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

// 扩展方法，用于生成排序后的输出
// 这需要结合 Writer 的逻辑。
// 但是 Sorter 本身只负责修改 Tree。
// CLI 命令逻辑是：Sort -> 打印。
// 如果用户想要 "sort 预期的逻辑是根据 dependency 的结果对节点排序，然后输出的是设备树。"
// 那么我们应该提供一个 output 方法，或者在 CLI 中，sort 完之后调用 Writer。
// 
// 当前实现是 Sorter 是一个 Visitor，它 *in-place* 修改了 tree。
// 如果我们想要输出，可以在 walk 结束后，再 walk 一遍 Writer。
// 或者 Sorter 本身包含一个 Writer？
// 通常做法是 pipeline： Parser -> Sorter (modify tree) -> Writer (output tree)。
// 
// 用户的 prompt 暗示 "sort 预期的逻辑... 然后输出的是设备树"。
// 这可能意味着 `test_sorter` 应该验证输出的 DTS 内容。
// 目前 `test_sorter` 只是打印了节点名。
// 我们应该修改 `test_sorter` 来使用 `DtsWriter` 输出完整的树，并与 `expected` 对比。
