use crate::dts::tree::{DTInfo, Node, Property, Data, Cell};
use crate::visitors::Visitor;
use crate::visitors::pinctrl::{PinctrlExtractor, PinConfig};
use std::collections::HashMap;
use std::fmt::Write;

/// Information about a device pin usage.
#[derive(Debug, Clone)]
pub struct DevicePin {
    pub device_name: String,
    pub pinctrl_name: String,
    pub config: PinConfig,
}

/// A visitor that extracts device pinctrl usage.
pub struct DevicePinctrlExtractor<'a> {
    tree: &'a DTInfo,
    pub pins: Vec<DevicePin>,
    gpio_banks: HashMap<String, u32>, // Label -> Bank ID
    path_stack: Vec<String>,
    label_map: HashMap<String, String>, // Label -> Full Path
}

impl<'a> DevicePinctrlExtractor<'a> {
    pub fn new(tree: &'a DTInfo) -> Self {
        let mut gpio_banks = HashMap::new();
        let mut label_map = HashMap::new();

        // Build label map
        Self::build_label_map(&tree.root, "/", &mut label_map);
        
        // Parse aliases to build gpio mapping
        if let Ok(aliases) = tree.get_node_by_path("/aliases") {
            if let Node::Existing { proplist, .. } = aliases {
                for (name, prop) in proplist {
                    if name.starts_with("gpio") {
                        let suffix = &name[4..];
                        if let Ok(bank_id) = suffix.parse::<u32>() {
                             if let Property::Existing { val: Some(data), .. } = prop {
                                 for d in data {
                                     match d {
                                         Data::Reference(label, _) => {
                                             gpio_banks.insert(label.clone(), bank_id);
                                         },
                                         // Sometimes it might be Data::String in some DTS parsers? 
                                         // But dts-parser typically uses Reference for &gpio0
                                         _ => {}
                                     }
                                 }
                             }
                        }
                    }
                }
            }
        }

        Self {
            tree,
            pins: Vec::new(),
            gpio_banks,
            path_stack: Vec::new(),
            label_map,
        }
    }
    
    fn build_label_map(node: &Node, current_path: &str, map: &mut HashMap<String, String>) {
        if let Node::Existing { labels, children, .. } = node {
            for label in labels {
                map.insert(label.clone(), current_path.to_string());
            }

            for (name, child) in children {
                let child_path = if current_path == "/" {
                    format!("/{}", name)
                } else {
                    format!("{}/{}", current_path, name)
                };
                Self::build_label_map(child, &child_path, map);
            }
        }
    }

    pub fn output(&self) -> String {
        let mut output = String::new();
        for pin in &self.pins {
            writeln!(output, "{},{},{},{},{},{}", 
                pin.device_name, 
                pin.pinctrl_name.replace(',', "_"), 
                pin.config.bank, 
                pin.config.pin, 
                pin.config.mux, 
                pin.config.config
            ).unwrap();
        }
        output
    }

    fn extract_from_node(&mut self, device_name: &str, label: &str, pinctrl_node: &Node) {
        if let Node::Existing { proplist: pinctrl_props, .. } = pinctrl_node {
            if let Some(Property::Existing { val: Some(pins_data), .. }) = pinctrl_props.get("rockchip,pins") {
                let parsed_pins = PinctrlExtractor::parse_rockchip_pins(pins_data);
                for pin_config in parsed_pins {
                    self.pins.push(DevicePin {
                        device_name: device_name.to_string(),
                        pinctrl_name: label.to_string(),
                        config: pin_config,
                    });
                }
            }
        }
    }
}

impl<'a> Visitor for DevicePinctrlExtractor<'a> {
    fn enter_node(&mut self, name: &str, node: &Node) -> bool {
        self.path_stack.push(name.to_string());
        
        // Construct full path
        let full_path = if self.path_stack.len() == 1 && self.path_stack[0] == "/" {
            "/".to_string()
        } else {
            format!("/{}", self.path_stack[1..].join("/"))
        };

        // We only care about Existing nodes with properties
        if let Node::Existing { proplist, .. } = node {
            // Use the node name as the device name
            let device_name = &full_path;

            for (prop_name, prop) in proplist {
                // 1. Check for pinctrl-N properties
                // e.g. pinctrl-0, pinctrl-1
                if prop_name.starts_with("pinctrl-") {
                    let suffix = &prop_name[8..];
                    if suffix.chars().all(|c| c.is_digit(10)) {
                         if let Property::Existing { val: Some(data), .. } = prop {
                            for d in data {
                                match d {
                                    Data::Cells(_, cells) => {
                                        for cell in cells {
                                            if let Cell::Ref(label, _) = cell {
                                                if let Ok(pinctrl_node) = self.tree.get_node_by_label(label) {
                                                    let pinctrl_path = self.label_map.get(label).cloned().unwrap_or(label.to_string());
                                                    self.extract_from_node(device_name, &pinctrl_path, pinctrl_node);
                                                }
                                            }
                                        }
                                    },
                                    Data::Reference(label, _) => {
                                        if let Ok(pinctrl_node) = self.tree.get_node_by_label(label) {
                                            let pinctrl_path = self.label_map.get(label).cloned().unwrap_or(label.to_string());
                                            self.extract_from_node(device_name, &pinctrl_path, pinctrl_node);
                                        }
                                    },
                                    _ => {}
                                }
                            }
                        }
                    }
                }

                // 2. Check for gpios properties
                // e.g. gpios, reset-gpios, enable-gpios, snps,reset-gpio
                if prop_name.ends_with("gpios") || prop_name.ends_with("gpio") {
                    if let Property::Existing { val: Some(data), .. } = prop {
                         for d in data {
                             if let Data::Cells(_, cells) = d {
                                 // Simple heuristic parsing for <&phandle pin flags>
                                 // We look for Cell::Ref followed by Cell::Num
                                 let mut iter = cells.iter();
                                 while let Some(cell) = iter.next() {
                                     if let Cell::Ref(label, _) = cell {
                                         if let Some(bank_id) = self.gpio_banks.get(label) {
                                             // Found a GPIO reference
                                             // The next cell should be the pin number
                                             if let Some(Cell::Num(pin)) = iter.next() {
                                                 let pin = *pin as u32;
                                                 
                                                 // Try to get flags if available (consume it)
                                                 let flags_str = if let Some(Cell::Num(flags)) = iter.next() {
                                                     format!("0x{:x}", flags)
                                                 } else {
                                                     "0x0".to_string()
                                                 };

                                                 self.pins.push(DevicePin {
                                                     device_name: device_name.to_string(),
                                                     pinctrl_name: prop_name.clone(),
                                                     config: PinConfig {
                                                         bank: *bank_id,
                                                         pin: pin,
                                                         mux: 0, // GPIO mode
                                                         config: flags_str,
                                                     }
                                                 });
                                             }
                                         }
                                     }
                                 }
                             }
                         }
                    }
                }
            }
        }
        true
    }

    fn exit_node(&mut self, _name: &str, _node: &Node) {
        self.path_stack.pop();
    }
}
