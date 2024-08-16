//! This binary provides command line tools to finish the whole process of modifying a wasm file and running it to test coverage
//!

#![warn(missing_docs)]

use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

use clap::{ArgGroup, Parser};
use colored::Colorize;
use wasmprinter::{Config, PrintFmtWrite};
use wast::core::EncodeOptions;
use wast::parser::{parse, ParseBuffer};
use wast::Wat;

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[clap(group(
    ArgGroup::new("input")
        .args(&["path", "bytes"])
))]
struct Cli {
    #[arg(short, long, value_name = "VERBOSE")]
    verbose: bool,

    #[arg(short, long, value_name = "FILE")]
    path: PathBuf,

    #[arg(short, long, value_name = "BUILD_DIRECTORY")]
    build_dir: PathBuf,

    #[arg(short, long, value_name = "OUTPUT_FILES")]
    output_files: Vec<PathBuf>,

    #[arg(short, long, value_name = "WORLD_NAME")]
    world: Option<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    if !cli.build_dir.exists() {
        if cli.verbose {
            println!("{} Creating build directory", "WCOV:".red());
        }
        fs::create_dir_all(&cli.build_dir)?;
    } else {
        assert!(cli.build_dir.is_dir())
    }

    if cli.verbose {
        println!("{} Converting binary to WAT", "WCOV:".red());
    }
    let binary = fs::read(cli.path.clone())?;
    let mut wat = PrintFmtWrite(String::new());
    let mut printer_cfg = Config::new();
    printer_cfg.print_offsets(true);
    printer_cfg.print(&binary, &mut wat)?;
    let wat = wat.0;
    if cli.verbose {
        println!("{} Modifying WAT", "WCOV".red());
    }
    let (output_wat, data) =
        wat_annotator::modify_wasm(None, Some(wat), Some(cli.path), cli.verbose)?;

    let buf = ParseBuffer::new(&output_wat)?;
    let mut output_wat = parse::<Wat>(&buf)?;
    if cli.verbose {
        println!("{} Encoding WAT to WASM", "WCOV:".red());
    }
    let opts = EncodeOptions::default();
    let output_binary = opts.encode_wat(&mut output_wat)?;

    if cli.verbose {
        println!("{} Creating output paths", "WCOV".red());
    }
    // create paths
    let output_paths = cli
        .output_files
        .iter()
        .map(|f| {
            cli.build_dir
                .join(format!("{}.gcov", f.file_name().unwrap().to_str().unwrap()))
        })
        .collect::<Vec<_>>();

    if cli.verbose {
        println!("{} Calling Runner", "WCOV".red());
    }
    runner::run(
        output_binary,
        Some(data),
        Some(HashMap::new()),
        Some(cli.output_files),
        Some(output_paths),
        cli.verbose,
    )
}
