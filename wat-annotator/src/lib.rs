//! This library provides methods to modify and collect debug data about Wat components

#![warn(missing_docs)]
use std::fmt::Display;

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
