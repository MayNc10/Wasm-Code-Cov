#![warn(missing_docs)]
use std::fmt::Display;

pub mod annotate;
pub mod data;
pub mod debug;
pub mod offset_tracker;
pub mod utils;

#[repr(i32)]
pub enum CounterType {
    Block = 0,
    If,
    Else,
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
    pub fn from_i32(n: i32) -> Option<CounterType> {
        if n >= 0 && n < NUM_TYPES {
            Some(unsafe { std::mem::transmute(n) })
        } else {
            None
        }
    }
}
