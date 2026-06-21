#!/bin/bash

# Check if input file is provided
if [ -z "$1" ]; then
    echo "Usage: $0 <dts_file>"
    exit 1
fi

DTS_FILE="$1"

# Determine HDTT command
if command -v hvisor-device-tree-tool &> /dev/null; then
    HDTT="hvisor-device-tree-tool"
elif [ -f "./hvisor-device-tree-tool" ]; then
    HDTT="./hvisor-device-tree-tool"
elif [ -f "./target/release/hvisor-device-tree-tool" ]; then
    HDTT="./target/release/hvisor-device-tree-tool"
elif [ -f "./target/debug/hvisor-device-tree-tool" ]; then
    HDTT="./target/debug/hvisor-device-tree-tool"
else
    echo "Error: hvisor-device-tree-tool not found."
    exit 1
fi

# Run extract-interrupts, keep GIC SPI entries, extract columns, and process.
# Expected format: full_path,name,chunk_size,cell1,cell2,cell3,parent_full_path
# ARM GIC uses cell1 as interrupt type: 0 means SPI, 1 means PPI.
# hvisor configs use the global interrupt ID, so only SPI offsets get +32.

$HDTT extract-interrupts "$DTS_FILE" \
| grep -iE "gic|interrupt-controller" \
| awk -F',' '$3 == 3 && $4 == "0x0" {print $1, $5}' \
| while read -r path irq; do
    final_irq=$(($irq + 32))
    printf "%d 0x%x, // %s\n" "$final_irq" "$final_irq" "$path"
done \
| sort -n -k1,1 \
| cut -d' ' -f2-
