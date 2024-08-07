use std::borrow::Cow;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::PathBuf;

use clap::{ArgGroup, Parser};
use serde::Serialize;

use wat_annotator::annotate::add_scaffolding;

#[derive(Parser)]
#[clap(group(
    ArgGroup::new("input")
        .args(&["path", "text"])
))]
struct Cli {
    #[arg(short, long, value_name = "FILE")]
    path: Option<PathBuf>,

    #[arg(short, long, value_name = "TEXT")]
    text: Option<String>,

    #[arg(short, long, value_name = "BINARY_FILE")]
    binary_path: Option<PathBuf>,

    #[arg(short, long, value_name = "DATA_OUTPUT_PATH")]
    data_output_path: Option<PathBuf>,
}

fn main() -> io::Result<()> {
    let mut cli = Cli::parse();
    if cli.path.is_none() && cli.text.is_none() {
        // try read text from stdin
        let mut buffer = String::new();
        let mut stdin = io::stdin();
        stdin.read_to_string(&mut buffer)?;
        cli.text = Some(buffer.to_string());
    }

    let (output, file_map) = add_scaffolding(
        cli.text.unwrap(),
        cli.binary_path.map(|p| Cow::Owned(fs::read(p).unwrap())),
    )
    .unwrap();
    if let Some(path) = cli.data_output_path {
        let mut f = File::create(path).unwrap();
        write!(f, "{}", serde_json::to_string(&file_map).unwrap()).unwrap();
    }
    println!("{}", output);
    Ok(())
}
