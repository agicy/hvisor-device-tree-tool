use std::path::PathBuf;
use std::process;

use crate::dts;
use crate::dts::tree::{Data, Node, Property};
use crate::visitors::writer::DtsWriter;
use crate::visitors::{
    Walker, dependency::DependencyExtractor, filter::NodeFilter, interrupts::InterruptsExtractor,
    pinctrl::PinctrlExtractor, reg_extractor::RegExtractor, sorter::SortByReference,
};
use clap::{Parser, Subcommand};

// Command-line interface definition for the application.
//
// This struct uses `clap` to parse command-line arguments and subcommands.
//
// Fields:
//   command: The subcommand to execute.
#[derive(Parser)]
#[command(name = "hvisor-device-tree-tool")]
#[command(version = "0.1.0")]
#[command(about = "A tool to manipulate device tree source files", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

// Available subcommands for the application.
#[derive(Subcommand)]
enum Commands {
    // Sort nodes by phandle reference.
    //
    // This command reorders nodes based on their dependencies (phandles).
    Sort {
        // Input DTS file path. If not provided, reads from stdin.
        input: Option<PathBuf>,
    },
    // Extract register information.
    //
    // This command parses the DTS and extracts register addresses and sizes.
    ExtractRegs {
        // Input DTS file path. If not provided, reads from stdin.
        input: Option<PathBuf>,
    },
    // Extract interrupt information.
    //
    // This command parses the DTS and extracts interrupt configurations.
    ExtractInterrupts {
        // Input DTS file path. If not provided, reads from stdin.
        input: Option<PathBuf>,
    },
    // Extract dependency information.
    //
    // This command analyzes the dependencies between nodes.
    Dependency {
        // Input DTS file path. If not provided, reads from stdin.
        input: Option<PathBuf>,
    },
    // Extract pinctrl information.
    //
    // This command parses the DTS and extracts pinctrl configurations.
    ExtractPinctrl {
        // Input DTS file path. If not provided, reads from stdin.
        input: Option<PathBuf>,
    },
    // Filter disabled nodes.
    //
    // This command removes nodes that have `status = "disabled"`.
    Filter {
        // Input DTS file path. If not provided, reads from stdin.
        input: Option<PathBuf>,
    },
}

// Executes the CLI application.
//
// This function parses the command-line arguments and dispatches the execution
// to the appropriate handler based on the selected subcommand.
pub fn run() {
    let cli = Cli::parse();

    // Helper function to parse the DTS file or stdin.
    //
    // Arguments:
    //   input: Optional path to the input file.
    //
    // Returns:
    //   The parsed DTS tree. Exits the process if parsing fails.
    let get_tree = |input: Option<&PathBuf>| {
        dts::parse_dts(input).unwrap_or_else(|e| {
            eprintln!("{}", e);
            process::exit(1);
        })
    };

    match cli.command {
        Commands::Sort { input } => {
            let tree = get_tree(input.as_ref());
            let mut sorter = SortByReference::new();
            // Walk the tree to sort nodes.
            Walker::walk(&tree.root, "/", &mut sorter);
            if let Some(new_root) = sorter.root {
                // Use DtsWriter to print the sorted tree to stdout.
                let mut buffer = Vec::new();
                let mut writer = DtsWriter::new(&mut buffer, true);
                Walker::walk(&new_root, "/", &mut writer);
                println!("{}", String::from_utf8_lossy(&buffer));
            }
        }
        Commands::ExtractRegs { input } => {
            let tree = get_tree(input.as_ref());
            let mut extractor = RegExtractor::new();
            // Walk the tree to extract register information.
            Walker::walk(&tree.root, "/", &mut extractor);
            println!("{}", extractor.output());
        }
        Commands::ExtractInterrupts { input } => {
            let tree = get_tree(input.as_ref());
            let mut extractor = InterruptsExtractor::new();
            // Walk the tree to extract interrupt information.
            Walker::walk(&tree.root, "/", &mut extractor);
            println!("{}", extractor.output());
        }
        Commands::Dependency { input } => {
            let tree = get_tree(input.as_ref());
            let mut extractor = DependencyExtractor::new();
            // Walk the tree to extract dependencies.
            Walker::walk(&tree.root, "/", &mut extractor);
            println!("{}", extractor.output());
        }
        Commands::ExtractPinctrl { input } => {
            let tree = get_tree(input.as_ref());
            let mut extractor = PinctrlExtractor::new();
            // Walk the tree to extract pinctrl information.
            Walker::walk(&tree.root, "/", &mut extractor);
            println!("{}", extractor.output());
        }
        Commands::Filter { input } => {
            let tree = get_tree(input.as_ref());
            // Define a predicate to identify disabled nodes.
            let predicate = |node: &Node| -> bool {
                if let Node::Existing { proplist, .. } = node {
                    if let Some(Property::Existing {
                        val: Some(data), ..
                    }) = proplist.get("status")
                    {
                        for d in data {
                            if let Data::String(s) = d {
                                if s == "disabled" {
                                    return true;
                                }
                            }
                        }
                    }
                }
                false
            };
            let mut filter = NodeFilter::new(predicate);
            // Walk the tree to filter out disabled nodes.
            Walker::walk(&tree.root, "/", &mut filter);
            if let Some(new_root) = filter.root {
                // Use DtsWriter to print the filtered tree to stdout.
                let mut buffer = Vec::new();
                let mut writer = DtsWriter::new(&mut buffer, true);
                Walker::walk(&new_root, "/", &mut writer);
                println!("{}", String::from_utf8_lossy(&buffer));
            }
        }
    }
}
