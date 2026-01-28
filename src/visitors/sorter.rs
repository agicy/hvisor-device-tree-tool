use crate::dts::tree::{Data, Node, Property};
use crate::visitors::Visitor;
use indexmap::IndexMap;
use petgraph::algo::toposort;
use petgraph::graph::DiGraph;
use std::collections::HashMap;

pub struct SortByReference;

enum Dependency {
    Label(String),
}

impl SortByReference {
    pub fn new() -> Self {
        Self
    }

    fn topological_sort_map(children: &IndexMap<String, Node>) -> IndexMap<String, Node> {
        let mut graph = DiGraph::<&str, ()>::new();
        let mut indices = HashMap::new();

        for name in children.keys() {
            indices.insert(name.as_str(), graph.add_node(name.as_str()));
        }

        // Collect phandles and labels
        let mut phandle_map = HashMap::new(); // phandle -> name
        let mut label_map = HashMap::new(); // label -> name

        for (name, node) in children {
            if let Some(phandle) = Self::get_phandle(node) {
                phandle_map.insert(phandle, name.as_str());
            }
            if let Node::Existing { labels, .. } = node {
                for label in labels {
                    label_map.insert(label.clone(), name.as_str());
                }
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
                        // Avoid self-cycles
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
                // Add any nodes that might have been missed
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
                children.clone()
            }
        }
    }

    fn get_phandle(node: &Node) -> Option<u32> {
        if let Node::Existing { proplist, .. } = node {
            if let Some(Property::Existing {
                val: Some(data), ..
            }) = proplist.get("phandle")
            {
                if data.len() == 1 {
                    if let Data::Cells(_bits, cells) = &data[0] {
                        if cells.len() == 1 {
                            if let crate::dts::tree::Cell::Num(c) = cells[0] {
                                return Some(c as u32);
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn find_dependencies(node: &Node) -> Vec<Dependency> {
        let mut deps = Vec::new();
        if let Node::Existing { proplist, .. } = node {
            for prop in proplist.values() {
                if let Property::Existing {
                    val: Some(data), ..
                } = prop
                {
                    for d in data {
                        if let Data::Reference(label, _) = d {
                            deps.push(Dependency::Label(label.clone()));
                        }
                    }
                }
            }
        }
        deps
    }
}

impl Visitor for SortByReference {
    fn enter_node(&mut self, _name: &str, node: &mut Node) -> bool {
        if let Node::Existing { children, .. } = node {
            if children.len() > 1 {
                let sorted = Self::topological_sort_map(children);
                *children = sorted;
            }
        }
        true
    }
}
