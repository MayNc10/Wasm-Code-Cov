//! Runner Binary
//! This binary provides commands to run a modified wasm file and extract debug information to calculate code coverage with

#![warn(missing_docs)]

use std::{collections::HashMap, error::Error, fs, io::Read, path::PathBuf};

use clap::{ArgGroup, Parser};
#[derive(Parser)]
#[clap(group(
    ArgGroup::new("input")
        .args(&["path", "bytes"])
))]
struct Cli {
    #[arg(short, long, value_name = "FILE")]
    path: Option<PathBuf>,

    #[arg(short, long, value_name = "BYTES")]
    bytes: Option<Vec<u8>>,

    #[arg(short, long, value_name = "FILE_MAP_PATH")]
    data_path: Option<PathBuf>,
    // make this require data path
    #[arg(short, long, value_name = "OUTPUT_FILES")]
    files_to_output: Option<Vec<PathBuf>>,

    #[arg(short, long, value_name = "OUTPUT_FILES")]
    output: Option<Vec<PathBuf>>,

    #[arg(short, long, value_name = "TRACEFILE_PATH")]
    tracefile_path: Option<PathBuf>,

    verbose: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut cli = Cli::parse();
    if cli.path.is_none() && cli.bytes.is_none() {
        // try read text from stdin
        let mut buffer = Vec::new();
        let mut stdin = std::io::stdin();
        stdin.read_to_end(&mut buffer)?;
        cli.bytes = Some(buffer);
    }

    let bytes = if let Some(bytes) = cli.bytes {
        bytes
    } else if let Some(path) = cli.path {
        fs::read(path).map_err(wasmtime::Error::new)?
    } else {
        unreachable!()
    };

    let file_map = cli
        .data_path
        .map(|p| {
            serde_json::from_slice::<wat_annotator::data::DebugDataOwned>(
                &fs::read(p).map_err(wasmtime::Error::new)?,
            )
            .map_err(wasmtime::Error::new)
        })
        .map_or(Ok(None), |v| v.map(Some))?;
    let gcov_files = file_map.as_ref().map(|_| HashMap::new());

    runner::run(
        bytes,
        file_map,
        gcov_files,
        cli.files_to_output,
        cli.output,
        cli.tracefile_path,
        cli.verbose,
    )
}
