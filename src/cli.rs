use std::path::PathBuf;
use std::process;

use crate::dts;
use crate::dts::tree::{Data, Node, Property};
use crate::visitors::{
    dependency::DependencyExtractor, filter::NodeFilter, interrupts::InterruptsExtractor,
    reg_extractor::RegExtractor, sorter::SortByReference, Walker,
};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "hvisor-device-tree-tool")]
#[command(version = "0.1.0")]
#[command(about = "A tool to manipulate device tree source files", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Sort nodes by phandle reference
    Sort {
        /// Input DTS file (optional, defaults to stdin)
        input: Option<PathBuf>,
    },
    /// Extract register information
    ExtractRegs {
        /// Input DTS file (optional, defaults to stdin)
        input: Option<PathBuf>,
    },
    /// Extract interrupt information
    ExtractInterrupts {
        /// Input DTS file (optional, defaults to stdin)
        input: Option<PathBuf>,
    },
    /// Extract dependency information
    Dependency {
        /// Input DTS file (optional, defaults to stdin)
        input: Option<PathBuf>,
    },
    /// Filter disabled nodes
    Filter {
        /// Input DTS file (optional, defaults to stdin)
        input: Option<PathBuf>,
    },
}

pub fn run() {
    let cli = Cli::parse();

    let get_tree = |input: Option<&PathBuf>| {
        dts::parse_dts(input).unwrap_or_else(|e| {
            eprintln!("{}", e);
            process::exit(1);
        })
    };

    match cli.command {
        Commands::Sort { input } => {
            let mut tree = get_tree(input.as_ref());
            let mut sorter = SortByReference::new();
            Walker::walk(&mut tree.root, "/", &mut sorter);
            println!("{:#?}", tree);
        }
        Commands::ExtractRegs { input } => {
            let mut tree = get_tree(input.as_ref());
            let mut extractor = RegExtractor::new();
            Walker::walk(&mut tree.root, "/", &mut extractor);
        }
        Commands::ExtractInterrupts { input } => {
            let mut tree = get_tree(input.as_ref());
            let mut extractor = InterruptsExtractor::new();
            Walker::walk(&mut tree.root, "/", &mut extractor);
        }
        Commands::Dependency { input } => {
            let tree = get_tree(input.as_ref());
            let mut extractor = DependencyExtractor::new();
            Walker::walk(&tree.root, "/", &mut extractor);
            println!("{}", extractor.output());
        }
        Commands::Filter { input } => {
            let mut tree = get_tree(input.as_ref());
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
            Walker::walk(&mut tree.root, "/", &mut filter);
            println!("{:#?}", tree);
        }
    }
}
