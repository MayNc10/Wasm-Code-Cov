//! This library provides methods to modify and collect debug data about Wat components

#![warn(missing_docs)]
use std::{
    borrow::Cow,
    error::Error,
    fmt::Display,
    fs,
    io::{self, Read},
    path::PathBuf,
};

use annotate::add_scaffolding;
use data::DebugDataOwned;

/// A module for annotating Wat files with the runner harness
pub mod annotate;
/// A module containing the representation of debug data structs
pub mod data;
/// A module for extracting debug information from Wat files
pub mod debug;
/// A module for mapping the offsets in an original file to their positiions in a modified one
pub mod offset_tracker;
/// A module for commonly used utility functions
pub mod utils;

/// Types of counters corresponding to different control flow blocks that we place counters at
#[repr(i32)]
pub enum CounterType {
    /// A Block Wasm instruction
    Block = 0,
    /// An If Wasm instruction
    If,
    /// An Else Wasm instruction
    Else,
    /// A Loop Wasm instruction
    Loop,
}

impl Display for CounterType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CounterType::Block => write!(f, "Block"),
            CounterType::If => write!(f, "If"),
            CounterType::Else => write!(f, "Else"),
            CounterType::Loop => write!(f, "Loop"),
        }
    }
}

// There's gotta be a safer way to do this (probably by using an actual enum type across the FFI border)
const NUM_TYPES: i32 = 4;

impl CounterType {
    /// Transforms an i32 into a counter enum if the i32 is a valid variant, otherwise returns false
    /// This allows us to pass simple i32s over the Wasm/host FFI barrier, instead of variants that make the modified Wasm more complicated
    pub fn from_i32(n: i32) -> Option<CounterType> {
        if n >= 0 && n < NUM_TYPES {
            Some(unsafe { std::mem::transmute(n) })
        } else {
            None
        }
    }
}

/// Takes the same input arguments as the CLI, and outputs either the modified WAT and data, or an error
pub fn modify_wasm(
    path: Option<PathBuf>,
    mut text: Option<String>,
    binary_path: Option<PathBuf>,
    verbose: bool,
) -> Result<(String, DebugDataOwned), Box<dyn Error>> {
    if path.is_none() && text.is_none() {
        // try read text from stdin
        let mut buffer = String::new();
        let mut stdin = io::stdin();
        stdin.read_to_string(&mut buffer)?;
        text = Some(buffer.to_string());
    }

    Ok(add_scaffolding(
        text.unwrap(),
        binary_path.map(|p| Cow::Owned(fs::read(p).unwrap())),
        verbose,
    )?)
}
