// Entry point for the hvisor-device-tree-tool application.
//
// This file delegates the execution to the CLI module.
use hvisor_device_tree_tool::cli;

fn main() {
    cli::run();
}
