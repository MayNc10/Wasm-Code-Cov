use std::{collections::HashMap, path::PathBuf, sync::Arc};

use wasmtime_wasi::{ResourceTable, WasiCtx, WasiView};
use wat_annotator::data::DebugDataArc;

use crate::gcov::GCovFile;

pub struct MyState {
    pub ctx: WasiCtx,
    pub table: ResourceTable,
    pub counters: Vec<i32>,
    pub debug_data: Option<DebugDataArc>,
    pub gcov_files: Option<HashMap<Arc<PathBuf>, GCovFile>>,
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