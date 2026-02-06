# hvisor-device-tree-tool

一个用于处理设备树源码（DTS）文件的命令行工具。

[English Documentation](README.md)

## 功能特性

- **排序 (Sort)**：根据 phandle 依赖关系对节点进行拓扑排序。
- **提取寄存器 (Extract Registers)**：提取节点的寄存器地址和大小信息。
- **提取中断 (Extract Interrupts)**：提取中断配置，并解析中断控制器的层级关系。
- **提取 Pinctrl (Extract Pinctrl)**：提取引脚控制器配置（引脚复用和驱动强度）。
- **提取设备 Pinctrl (Extract Device Pinctrl)**：提取设备到 pinctrl 使用关系的映射，包括 GPIO 和引用的 pinctrl 分组。
- **依赖分析 (Dependency Analysis)**：分析并输出节点间的依赖关系（如时钟、电源域等）。
- **过滤 (Filter)**：移除 `status = "disabled"` 的节点。

## 构建指南

请确保已安装 [Rust](https://www.rust-lang.org/tools/install) 环境，然后运行以下命令进行构建：

```bash
cargo build --release
```

编译生成的可执行文件位于 `target/release/hvisor-device-tree-tool`。

## 使用说明

```bash
hvisor-device-tree-tool <子命令> [输入文件]
```

如果未提供 `[输入文件]`，工具将从标准输入 (stdin) 读取内容。

### 子命令

#### 1. Sort (排序)
根据引用（phandle）对节点进行拓扑排序。这确保了如果节点 A 引用了节点 B，那么节点 B 会出现在节点 A 之前（在树结构允许的情况下）。

```bash
hvisor-device-tree-tool sort input.dts
```

#### 2. Extract Registers (提取寄存器)
提取节点的 `reg` 属性信息（起始地址和大小）。它能正确处理父节点的 `#address-cells` 和 `#size-cells`。

```bash
hvisor-device-tree-tool extract-regs input.dts
```

**输出格式**：`完整路径,起始地址(Hex),大小(Hex)`

#### 3. Extract Interrupts (提取中断)
提取中断信息，并解析 `interrupt-parent` 和 `interrupt-controller` 层级，提供扁平化的中断视图。

```bash
hvisor-device-tree-tool extract-interrupts input.dts
```

**输出格式**：`完整路径,名称,块大小,单元值(Hex),父节点完整路径`

#### 4. Extract Pinctrl (提取 Pinctrl)
提取 Pinctrl 配置，按设备和方案进行分组。

```bash
hvisor-device-tree-tool extract-pinctrl input.dts
```

**输出格式**：
```
设备别名,方案个数
gpio<Bank>,<Pin>,<Mux>,<Config>,...
```

#### 5. Extract Device Pinctrl (提取设备 Pinctrl)
提取设备到 pinctrl 使用关系的映射，包括引用的 pinctrl 分组和 GPIO 类属性。

```bash
hvisor-device-tree-tool extract-device-pinctrl input.dts
```

**输出格式**：`设备路径,pinctrl 路径,bank,pin,mux,config`

#### 6. Dependency (依赖分析)
提取节点间的依赖关系，基于常见的属性如 `clocks`, `power-domains`, `iommus` 等。

```bash
hvisor-device-tree-tool dependency input.dts
```

**输出格式**：`子节点路径 -> 父节点路径或标签`

#### 7. Filter (过滤)
移除所有包含 `status = "disabled"` 属性的节点。

```bash
hvisor-device-tree-tool filter input.dts
```

#### 8. Extract RK3588 IRQs (提取 RK3588 中断)
提取 RK3588 的 GIC 中断号，并将中断号加上 32（SPI 偏移量）。

```bash
./scripts/extract_rk3588_irqs.sh input.dts
```

**输出格式**：`十六进制中断号, // 节点路径`

#### 9. Extract Top-Level Registers (提取顶层寄存器)
仅提取顶层节点（如 `/serial@1000`）的寄存器信息（路径、起始地址、大小），并按起始地址排序。

```bash
./scripts/extract_top_level_regs.sh input.dts
```

**输出格式**：`path,start_hex,size_hex`

## 测试

运行单元测试和集成测试：

```bash
cargo test
```
