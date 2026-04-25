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

# Run extract-regs, filter for top-level nodes (e.g. /serial@1000 but not /cpus/cpu@0)
# Use grep to match paths starting with / followed by characters that do not contain /
# Then use awk for formatting and sorting prep.
$HDTT extract-regs "$DTS_FILE" | grep -E "^/[^/]+," | awk -F',' '{
    path=$1
    start=$2
    size=$3
    
    # Normalize start hex: remove 0x
    hex = start
    sub(/^0x/, "", hex)
    # Handle empty start
    if (hex != "") {
        # Output: length hex_string start,size, // path
        printf "%d %s %s,%s, // %s\n", length(hex), hex, start, size, path
    }
}' | sort -k1,1n -k2,2 | cut -d' ' -f3-
