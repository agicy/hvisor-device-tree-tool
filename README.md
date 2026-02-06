# hvisor-device-tree-tool

A command-line tool to manipulate Device Tree Source (DTS) files.

[中文文档](README_zh.md)

## Features

- **Sort**: Reorders nodes topologically based on their phandle dependencies.
- **Extract Registers**: Extracts register address and size information from nodes.
- **Extract Interrupts**: Extracts interrupt configuration and resolves interrupt controller hierarchies.
- **Extract Pinctrl**: Extracts pin controller configurations (pin muxing and drive strength).
- **Extract Device Pinctrl**: Extracts device-to-pinctrl usage mappings, including GPIO and referenced pinctrl groups.
- **Dependency Analysis**: Analyzes and outputs node dependency relationships (e.g., clocks, power domains).
- **Filter**: Removes nodes marked as `status = "disabled"`.

## Building

To build the project, ensure you have [Rust](https://www.rust-lang.org/tools/install) installed, then run:

```bash
cargo build --release
```

The executable will be located at `target/release/hvisor-device-tree-tool`.

## Usage

```bash
hvisor-device-tree-tool <COMMAND> [INPUT_FILE]
```

If `[INPUT_FILE]` is not provided, the tool reads from standard input (stdin).

### Commands

#### 1. Sort
Sorts nodes topologically based on their references (phandles). This ensures that if Node A references Node B, Node B appears before Node A (where applicable/possible within the tree structure).

```bash
hvisor-device-tree-tool sort input.dts
```

#### 2. Extract Registers
Extracts `reg` property information (address and size) from nodes. It correctly handles `#address-cells` and `#size-cells` from parent nodes.

```bash
hvisor-device-tree-tool extract-regs input.dts
```

**Output format:** `full_path,start_hex,size_hex`

#### 3. Extract Interrupts
Extracts interrupt information, resolving `interrupt-parent` and `interrupt-controller` hierarchies to provide a flat view of interrupts.

```bash
hvisor-device-tree-tool extract-interrupts input.dts
```

**Output format:** `full_path,name,chunk_size,cells_hex,parent_full_path`

#### 4. Extract Pinctrl
Extracts pinctrl configurations, grouping them by device and scheme.

```bash
hvisor-device-tree-tool extract-pinctrl input.dts
```

**Output format:**
```
DeviceAlias,SchemeCount
gpio<Bank>,<Pin>,<Mux>,<Config>,...
```

#### 5. Extract Device Pinctrl
Extracts device-to-pinctrl usage mappings, including referenced pinctrl groups and GPIO-style properties.

```bash
hvisor-device-tree-tool extract-device-pinctrl input.dts
```

**Output format:** `device_path,pinctrl_path,bank,pin,mux,config`

#### 6. Dependency
Extracts dependency relationships between nodes based on common properties like `clocks`, `power-domains`, `iommus`, etc.

```bash
hvisor-device-tree-tool dependency input.dts
```

**Output format:** `child_path -> parent_path_or_label`

#### 7. Filter
Removes nodes that have `status = "disabled"`.

```bash
hvisor-device-tree-tool filter input.dts
```

#### 8. Extract RK3588 IRQs
Extracts GIC interrupt numbers for RK3588, adding 32 (SPI offset) to the interrupt number.

```bash
./scripts/extract_rk3588_irqs.sh input.dts
```

**Output format:** `HexIRQ, // NodePath`

#### 9. Extract Top-Level Registers
Extracts register information (path, start, size) for top-level nodes only (e.g. `/serial@1000`), sorted by start address.

```bash
./scripts/extract_top_level_regs.sh input.dts
```

**Output format:** `path,start_hex,size_hex`

## Testing

Run unit and integration tests with:

```bash
cargo test
```
