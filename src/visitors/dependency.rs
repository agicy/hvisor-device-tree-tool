use crate::dts::tree::{Data, Node, Property, Cell};
use crate::visitors::Visitor;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

pub struct DependencyExtractor {
    // 当前路径栈
    path_stack: Vec<String>,
    // 收集到的依赖关系 (child_path, parent_name/phandle)
    // 由于是 name reference，这里存 parent_name
    dependencies: Vec<(String, String)>,
    // 节点路径映射：name -> full_path
    // 用于将引用名解析为完整路径
    // 注意：这里假设 name 是唯一的，或者我们能通过 label 找到。
    // 题目要求 "全部用名字引用"，即 &clk, &gic。
    // 在 parser 中，&clk 会被解析为 Cell::Ref("clk") 或 Data::Reference("clk")。
    // 我们需要维护 label -> full_path 的映射。
    label_map: HashMap<String, String>,
    
    // 临时存储所有节点，以便第二遍遍历？
    // 或者我们可以在 enter_node 时记录 label，
    // 但依赖关系可能引用尚未访问的节点吗？
    // "全部用名字引用" 通常意味着引用 label。
    // DTS 编译时会解析 label。
    // 如果我们只是解析源文件，label 映射到路径需要我们在遍历过程中建立。
    // 我们可以分两步：
    // 1. 第一遍遍历建立 label -> path 映射。
    // 2. 第二遍遍历解析依赖。
    // 
    // 或者，我们可以收集所有依赖关系 (child_path, target_label)，
    // 然后在最后统一解析 target_label 为 target_path。
    
    pub output: String,
}

impl DependencyExtractor {
    pub fn new() -> Self {
        Self {
            path_stack: Vec::new(),
            dependencies: Vec::new(),
            label_map: HashMap::new(),
            output: String::new(),
        }
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
                                self.dependencies.push((current_path.to_string(), name.clone()));
                            }
                            Data::Cells(_, cells) => {
                                for c in cells {
                                    if let Cell::Ref(name, _) = c {
                                        self.dependencies.push((current_path.to_string(), name.clone()));
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
    fn enter_node(&mut self, name: &str, node: &mut Node) -> bool {
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

    fn exit_node(&mut self, _name: &str, _node: &mut Node) {
        self.path_stack.pop();
    }
}

// 扩展方法，用于在遍历结束后生成输出
impl DependencyExtractor {
    pub fn generate_output(&mut self) {
        // 对依赖进行排序以保证输出确定性
        self.dependencies.sort();
        
        for (child, target_label) in &self.dependencies {
            let target_path = self.label_map.get(target_label).cloned().unwrap_or_else(|| format!("label:{}", target_label));
            writeln!(self.output, "{} -> {}", child, target_path).unwrap();
        }
    }
}
