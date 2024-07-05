use std::{io::Read, path::PathBuf};
use std::io;

use clap::{ArgGroup, Parser};

use wat_annotator::counter::insert_counters;

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
    
    let output = insert_counters(cli.text.unwrap());
    println!("{}", output.unwrap());
    Ok(())
}
