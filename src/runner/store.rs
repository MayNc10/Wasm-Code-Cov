//! This module provides the `MyState` struct for storing state while running a Wasm component
use std::{collections::HashMap, path::PathBuf, sync::Arc};

use wasmtime_wasi::{ResourceTable, WasiCtx, WasiView};
use crate::annotator::data::DebugDataArc;

use crate::runner::gcov::GCovFile;

/// This struct holds all the state for running a wasm component under testing
pub struct MyState {
    /// The context of the running wasi environment
    pub ctx: WasiCtx,
    /// The table of Wasm resources
    pub table: ResourceTable,
    /// The counter vector
    pub counters: Vec<i32>,
    /// If debug data was provided, it is stored here
    pub debug_data: Option<DebugDataArc>,
    /// A map of paths to gcov annotated versions
    pub gcov_files: Option<HashMap<Arc<PathBuf>, GCovFile>>,
    /// Whether the runner should print debug output
    pub verbose: bool,
}

impl WasiView for MyState {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}
