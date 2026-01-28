use std::io::Read;
use std::path::PathBuf;
use std::process;

use crate::visitors::Walker;
use clap::{Parser, Subcommand};
use visitors::filter::FilterDisabled;
use visitors::interrupts::InterruptsExtractor;
use visitors::reg_extractor::RegExtractor;
use visitors::sorter::SortByReference;

mod dts;
mod visitors;

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
    /// Filter disabled nodes
    Filter {
        /// Input DTS file (optional, defaults to stdin)
        input: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Sort { input } => {
            let mut tree = parse_dts(input.as_ref());
            let mut sorter = SortByReference::new();
            Walker::walk(&mut tree.root, "/", &mut sorter);
            println!("{:#?}", tree);
        }
        Commands::ExtractRegs { input } => {
            let mut tree = parse_dts(input.as_ref());
            let mut extractor = RegExtractor::new();
            Walker::walk(&mut tree.root, "/", &mut extractor);
        }
        Commands::ExtractInterrupts { input } => {
            let mut tree = parse_dts(input.as_ref());
            let mut extractor = InterruptsExtractor::new();
            Walker::walk(&mut tree.root, "/", &mut extractor);
        }
        Commands::Filter { input } => {
            let mut tree = parse_dts(input.as_ref());
            let mut filter = FilterDisabled::new();
            Walker::walk(&mut tree.root, "/", &mut filter);
            println!("{:#?}", tree);
        }
    }
}

fn parse_dts(path: Option<&PathBuf>) -> dts::tree::DTInfo {
    let mut buffer = Vec::new();

    match path {
        Some(p) => {
            let mut file = std::fs::File::open(p).unwrap_or_else(|err| {
                eprintln!("Error opening file {:?}: {}", p, err);
                process::exit(1);
            });
            file.read_to_end(&mut buffer).unwrap_or_else(|err| {
                eprintln!("Error reading file {:?}: {}", p, err);
                process::exit(1);
            });
        }
        None => {
            // Read from stdin
            std::io::stdin().read_to_end(&mut buffer).unwrap_or_else(|err| {
                eprintln!("Error reading from stdin: {}", err);
                process::exit(1);
            });
        }
    }

    match dts::parser::parse_dt(&buffer) {
        Ok(result) => match result {
            dts::parser::ParseResult::Complete(mut tree, amends) => {
                // Apply amendments to the tree
                // Note: merge_amends might panic if references are invalid,
                // which is acceptable for this tool for now.
                tree.merge_amends(&amends);
                tree
            }
            dts::parser::ParseResult::RemainingInput(mut tree, amends, rem) => {
                eprintln!("Warning: remaining input: {}", String::from_utf8_lossy(rem));
                tree.merge_amends(&amends);
                tree
            }
        },
        Err(e) => {
            eprintln!("Error parsing DTS: {:?}", e);
            process::exit(1);
        }
    }
}
