use crate::dts::tree::{Data, Node, Property, Cell};
use crate::visitors::Visitor;
use std::collections::HashMap;
use std::fmt::Write;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DependencyInfo {
    pub child_path: String,
    pub parent_label: String,
}

pub struct DependencyExtractor {
    // 当前路径栈
    path_stack: Vec<String>,
    // 收集到的依赖关系
    dependencies: Vec<DependencyInfo>,
    // 节点路径映射：name -> full_path
    label_map: HashMap<String, String>,
}

impl DependencyExtractor {
    pub fn new() -> Self {
        Self {
            path_stack: Vec::new(),
            dependencies: Vec::new(),
            label_map: HashMap::new(),
        }
    }

    pub fn output(&self) -> String {
        // 对依赖进行排序以保证输出确定性
        // 注意：这里我们不能直接修改 self.dependencies，因为 output 是 &self
        // 所以我们克隆并排序
        let mut deps = self.dependencies.iter().collect::<Vec<_>>();
        deps.sort();
        
        let mut output = String::new();
        for dep in deps {
            let target_path = self.label_map.get(&dep.parent_label).cloned().unwrap_or_else(|| format!("label:{}", dep.parent_label));
            writeln!(output, "{} -> {}", dep.child_path, target_path).unwrap();
        }
        output
    }

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
        if path.is_empty() { "/" .to_string() } else { path }
    }

    // 检查常见的依赖属性
    fn check_dependencies(&mut self, node: &Node, current_path: &str) {
        if let Node::Existing { proplist, .. } = node {
            // 常见的依赖属性列表
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
        // 更新路径栈
        // 根节点名为 "" 或 "/"
        let node_name = if name.is_empty() { "/" } else { name };
        self.path_stack.push(node_name.to_string());
        
        let current_path = self.get_current_path();

        // 记录 label -> path 映射
        if let Node::Existing { labels, .. } = node {
            for label in labels {
                self.label_map.insert(label.clone(), current_path.clone());
            }
        }

        // 收集依赖
        self.check_dependencies(node, &current_path);

        true
    }

    fn exit_node(&mut self, _name: &str, _node: &Node) {
        self.path_stack.pop();
    }
}
