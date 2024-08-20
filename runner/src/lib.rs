//! This library contains support for running modified wasm files produced with `wat-annotator`
//! Most of the code in this library is for prodiving gcov-formatted output

#![warn(missing_docs)]

use std::error::Error;
use std::{fs, path::PathBuf};

pub mod gcov;
pub mod inc_counter;
pub mod lcov;
pub mod store;

use component::{Component, ResourceTable};
use gcov::GCovFile;
use store::MyState;
use wasmtime::*;
use wasmtime_wasi::{bindings::sync::exports::wasi::cli::run::Guest, WasiCtxBuilder};
use wat_annotator::data::*;

// There's definitely a faster way to write this, but I like writing code :3

struct ConstantIterator<T: Copy + Clone> {
    value: T,
    count: usize,
}

impl<T: Copy + Clone> ConstantIterator<T> {
    fn new_default_value(count: usize) -> ConstantIterator<T>
    where
        T: Default,
    {
        ConstantIterator {
            value: T::default(),
            count,
        }
    }
}

// I enjoy that this type exists now
impl<T: Copy + Clone> Iterator for ConstantIterator<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.count == 0 {
            None
        } else {
            self.count -= 1;
            Some(self.value)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.count, Some(self.count))
    }
}

use std::collections::HashMap;

use std::sync::Arc;

/// Takes all the same arguments as the CLI tool, and runs a Wasm component under testing
pub fn run(
    bytes: Vec<u8>,
    file_map: Option<DebugDataOwned>,
    gcov_files: Option<HashMap<Arc<PathBuf>, GCovFile>>,
    files_to_output: Option<Vec<PathBuf>>,
    output: Option<Vec<PathBuf>>,
    tracefile_path: Option<PathBuf>,
    verbose: bool,
) -> Result<(), Box<dyn Error>> {
    let engine = Engine::default();

    let mut linker = component::Linker::<MyState>::new(&engine);
    wasmtime_wasi::add_to_linker_sync(&mut linker)?;

    let wasi = WasiCtxBuilder::new().inherit_stdio().build();

    let mut store = Store::new(
        &engine,
        MyState {
            ctx: wasi,
            table: ResourceTable::new(),
            counters: Vec::new(),
            debug_data: file_map.map(Into::into),
            gcov_files,
            verbose,
        },
    );

    let inc_counter = |store: StoreContextMut<MyState>, args| {
        let verbose = store.data().verbose;
        inc_counter::inc_counter(store, args, verbose)
    };

    linker.root().func_wrap("inc-counter", inc_counter)?;

    let component = Component::new(&engine, &bytes)?;

    let instance = linker.instantiate(&mut store, &component)?;
    let mut exports = instance.exports(&mut store);
    let guest = Guest::new(&mut exports.instance("wasi:cli/run@0.2.0").ok_or(
        wasmtime::Error::msg("couldn't find export instance wasi:run!"),
    )?)?;
    drop(exports); // sucks -_-

    guest
        .call_run(&mut store)?
        .map_err(|_| wasmtime::Error::msg("running code returned error"))?;

    if let Some(outputs) = files_to_output {
        if let Some(output_files) = output {
            assert_eq!(output_files.len(), outputs.len());
            for (idx, file) in outputs.iter().enumerate() {
                if let Some(gcov) = store
                    .data()
                    .gcov_files
                    .as_ref()
                    .unwrap()
                    .get(&file.canonicalize().unwrap())
                {
                    fs::write(output_files[idx].as_path(), format!("{}", gcov)).unwrap();
                } else {
                    eprintln!(
                        "Requested output file not found in source files! Requested file: {}, source files: {:?}",
                        file.display(), store.data().gcov_files.as_ref().unwrap().keys()
                    );
                }
            }
        } else {
            for path in &outputs {
                let gcov =
                    &store.data().gcov_files.as_ref().unwrap()[&path.canonicalize().unwrap()];
                println!("{}:\n{}", path.display(), gcov);
            }
        }

        // create tracefile
        if tracefile_path.is_some()
            && store.data().gcov_files.is_some()
            && store.data().debug_data.is_some()
        {
            let mut source_files = Vec::new();

            let files = store.data().gcov_files.as_ref().unwrap();
            let debug_data = store.data().debug_data.as_ref().unwrap();
            for file_path in &outputs {
                let file_path = file_path.canonicalize().unwrap();
                let gcov = files.get(&file_path).unwrap();
                if verbose {
                    println!("Adding file to tracefile: {}", file_path.display());
                }
                if let Some(sdi) = debug_data.get_sdi_from_file(&file_path) {
                    if verbose {
                        println!("Creating SDI");
                    }
                    let source_file = lcov::SourceFile::new(&gcov, &sdi);
                    source_files.push(source_file);
                }
            }
            let tracefile = lcov::TraceFile::new(Some("tracefile"), source_files);
            let path = tracefile_path.unwrap();
            fs::write(path.as_path(), format!("{}", tracefile)).unwrap();
        }
    }
    Ok(())
}
