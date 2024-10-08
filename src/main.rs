//! This binary provides command line tools to finish the whole process of modifying a wasm file and running it to test coverage
//!

#![warn(missing_docs)]

use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

use clap::{ArgGroup, Parser};
use wasmprinter::{Config, PrintFmtWrite};
use wast::core::EncodeOptions;
use wast::parser::{parse, ParseBuffer};
use wast::Wat;
use wcov::noise::NoiseLevel;
use wcov::printer::println_wcov_dbg;

const ABOUT: &str = "wcov is a tool for generating lcov/gcov style coverage reports from Wasm modules or components. 
To test Wasm code, run wcov --path <WASM_FILE> --build-dir <BUILD_DIR> --output-files <OUTPUT_FILES>. 
wcov will embed debugging host functions into the Wasm code found in <WASM_FILE>, will run the modified code in a debugging environment, 
collect counter information on the source files listed in <OUTPUT_FILES>, and place generated reports in <BUILD_DIR>"; 

#[derive(Parser)]
#[command(version, about, long_about = Some(ABOUT))]
#[clap(group(
    ArgGroup::new("input")
        .args(&["path", "bytes"])
))]
#[clap(group(
    ArgGroup::new("output_noise_level")
        .args(&["verbose", "quiet"])
))]
struct Cli {
    #[arg(short, long, value_name = "VERBOSE")]
    verbose: bool,

    #[arg(short, long, value_name = "QUIET")]
    quiet: bool,

    #[arg(short, long, value_name = "FILE")]
    path: PathBuf,

    #[arg(short, long, value_name = "BUILD_DIRECTORY")]
    build_dir: PathBuf,

    #[arg(short, long, value_name = "OUTPUT_FILES")]
    output_files: Vec<PathBuf>,

    #[arg(short, long, value_name = "WORLD_NAME")]
    world: Option<String>,

    #[arg(short, long, value_name = "DUMP_DATA")]
    dump_data: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    if !cli.build_dir.exists() {
        if cli.verbose {
            println_wcov_dbg("Creating build directory");
        }
        fs::create_dir_all(&cli.build_dir)?;
    } else {
        assert!(cli.build_dir.is_dir())
    }
    let noise_level = NoiseLevel::from_settings(cli.verbose, cli.quiet);

    if noise_level.debug() {
        println_wcov_dbg("Converting binary to WAT");
    }
    let binary = fs::read(cli.path.clone())?;
    let mut wat = PrintFmtWrite(String::new());
    let mut printer_cfg = Config::new();
    printer_cfg.print_offsets(true);
    printer_cfg.print(&binary, &mut wat)?;
    let wat = wat.0;
    if noise_level.debug() {
        println_wcov_dbg("Modifying WAT")
    }
    let (output_wat, data) =
        wcov::annotator::modify_wasm(None, Some(wat), Some(cli.path), noise_level)?;

    if cli.dump_data {
        // output data to build folder
        let json_path = cli.build_dir.join("data.json");
        fs::write(json_path, serde_json::to_string_pretty(&data)?)?;
        let wat_path = cli.build_dir.join("src.wat");
        fs::write(wat_path, &output_wat)?;
    }
    let buf = ParseBuffer::new(&output_wat)?;
    let mut output_wat = parse::<Wat>(&buf)?;
    if noise_level.debug() {
        println_wcov_dbg("Encoding WAT to WASM")
    }
    let opts = EncodeOptions::default();
    let output_binary = opts.encode_wat(&mut output_wat)?;

    if noise_level.debug() {
        println_wcov_dbg("Creating output paths");
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

    let tracefile_path = cli.build_dir.join("wcov.info");

    if noise_level.debug() {
        println_wcov_dbg("Calling runner");
    }
    wcov::runner::run(
        output_binary,
        Some(data),
        Some(HashMap::new()),
        Some(cli.output_files),
        Some(output_paths),
        Some(tracefile_path),
        noise_level,
    )
}
