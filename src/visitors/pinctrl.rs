use crate::dts::tree::{Cell, Data, Node, Property};
use crate::visitors::Visitor;
use std::fmt::Write;

/// Information about a single pin configuration.
#[derive(Debug, Clone)]
pub struct PinConfig {
    pub bank: u32,
    pub pin: u32,
    pub mux: u32,
    pub config: String,
}

/// Information about a scheme (a group of pins).
#[derive(Debug, Clone)]
pub struct SchemeInfo {
    pub name: String, // e.g. "uart0_xfer"
    pub pins: Vec<PinConfig>,
}

/// Information about a device (a group of schemes).
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub alias: String, // e.g. "uart0"
    pub schemes: Vec<SchemeInfo>,
}

/// A visitor that extracts pinctrl information.
pub struct PinctrlExtractor {
    // Tracks if we are inside the pinctrl node.
    in_pinctrl: bool,
    // Stack to track depth relative to pinctrl node.
    // 0 = pinctrl node itself
    // 1 = device node
    // 2 = scheme node
    depth_stack: usize,
    // Stack to track the current path.
    path_stack: Vec<String>,
    
    // The currently processing device.
    current_device: Option<DeviceInfo>,
    
    // All collected devices.
    pub devices: Vec<DeviceInfo>,
}

impl PinctrlExtractor {
    pub fn new() -> Self {
        Self {
            in_pinctrl: false,
            depth_stack: 0,
            path_stack: Vec::new(),
            current_device: None,
            devices: Vec::new(),
        }
    }

    pub fn output(&self) -> String {
        let mut output = String::new();
        for device in &self.devices {
            writeln!(output, "{},{}", device.alias, device.schemes.len()).unwrap();
            for scheme in &device.schemes {
                writeln!(output, "{},{}", scheme.name, scheme.pins.len()).unwrap();
                for pin in &scheme.pins {
                    writeln!(output, "gpio{},{},{},{}", pin.bank, pin.pin, pin.mux, pin.config).unwrap();
                }
            }
        }
        output
    }

    fn is_pinctrl_node(&self, node: &Node) -> bool {
        if let Node::Existing { proplist, .. } = node {
            // Check compatible string
            if let Some(Property::Existing { val: Some(data), .. }) = proplist.get("compatible") {
                for d in data {
                    if let Data::String(s) = d {
                        if s.contains("pinctrl") {
                            return true;
                        }
                    }
                }
            }
        }
        // Fallback: check name if it is literally "pinctrl" (often the case for root pinctrl)
        if let Node::Existing { name, .. } = node {
            if name.as_str() == "pinctrl" {
                return true;
            }
        }
        false
    }

    pub fn parse_rockchip_pins(data: &[Data]) -> Vec<PinConfig> {
        let mut pins = Vec::new();
        let mut all_cells = Vec::new();

        // Flatten cells
        for d in data {
            if let Data::Cells(_, cells) = d {
                all_cells.extend(cells.iter().cloned());
            }
        }

        // Chunk by 4
        // <bank pin mux config>
        for chunk in all_cells.chunks(4) {
            if chunk.len() == 4 {
                let bank = match &chunk[0] {
                    Cell::Num(n) => *n as u32,
                    _ => 0, // Should be number
                };
                let pin = match &chunk[1] {
                    Cell::Num(n) => *n as u32,
                    _ => 0,
                };
                let mux = match &chunk[2] {
                    Cell::Num(n) => *n as u32,
                    _ => 0,
                };
                let config = match &chunk[3] {
                    Cell::Ref(s, _) => s.clone(),
                    Cell::Num(n) => format!("0x{:x}", n),
                };

                pins.push(PinConfig {
                    bank,
                    pin,
                    mux,
                    config,
                });
            }
        }
        pins
    }
}

impl Visitor for PinctrlExtractor {
    fn enter_node(&mut self, name: &str, node: &Node) -> bool {
        self.path_stack.push(name.to_string());
        
        // Construct full path
        let full_path = if self.path_stack.len() == 1 && self.path_stack[0] == "/" {
            "/".to_string()
        } else {
            format!("/{}", self.path_stack[1..].join("/"))
        };

        // Check if we are entering pinctrl
        if !self.in_pinctrl {
            if self.is_pinctrl_node(node) {
                self.in_pinctrl = true;
                self.depth_stack = 0;
            }
            return true;
        }

        // We are inside pinctrl
        self.depth_stack += 1;

        if self.depth_stack == 1 {
            // This is a potential device node
            // e.g. "uart0", "i2c0"
            // Use full path as alias
            let alias = full_path;
            
            self.current_device = Some(DeviceInfo {
                alias,
                schemes: Vec::new(),
            });
        } else if self.depth_stack == 2 {
            // This is a scheme node
            // e.g. "uart0_xfer"
            if let Some(device) = &mut self.current_device {
                if let Node::Existing { proplist, .. } = node {
                    if let Some(Property::Existing { val: Some(data), .. }) = proplist.get("rockchip,pins") {
                        let pins = Self::parse_rockchip_pins(data);
                        if !pins.is_empty() {
                            device.schemes.push(SchemeInfo {
                                name: full_path,
                                pins,
                            });
                        }
                    }
                }
            }
        }

        true
    }

    fn exit_node(&mut self, _name: &str, _node: &Node) {
        if self.in_pinctrl {
            if self.depth_stack == 1 {
                // Exiting device node
                if let Some(device) = self.current_device.take() {
                    if !device.schemes.is_empty() {
                        self.devices.push(device);
                    }
                }
            } else if self.depth_stack == 0 {
                // Exiting pinctrl node
                self.in_pinctrl = false;
            }
            
            if self.depth_stack > 0 {
                self.depth_stack -= 1;
            }
        }
        self.path_stack.pop();
    }
}
