use std::borrow::Cow;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::PathBuf;

use clap::{ArgGroup, Parser};

use wat_annotator::annotate::add_scaffolding;
use wat_annotator::parse_cli;

#[derive(Parser)]
#[clap(group(
    ArgGroup::new("input")
        .args(&["path", "text"])
))]
pub struct Cli {
    #[arg(short, long, value_name = "FILE")]
    pub path: Option<PathBuf>,

    #[arg(short, long, value_name = "TEXT")]
    pub text: Option<String>,

    #[arg(short, long, value_name = "BINARY_FILE")]
    pub binary_path: Option<PathBuf>,

    #[arg(short, long, value_name = "DATA_OUTPUT_PATH")]
    pub data_output_path: Option<PathBuf>,

    #[arg(short, long, value_name = "VERBOSE")]
    pub verbose: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let (output, data) = parse_cli(cli.path, cli.text, cli.binary_path, cli.verbose)?;

    if let Some(path) = cli.data_output_path {
        let mut f = File::create(path).unwrap();
        write!(f, "{}", serde_json::to_string(&data).unwrap()).unwrap();
    }
    println!("{}", output);
    Ok(())
}
