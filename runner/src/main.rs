#![warn(missing_docs)]
use colored::Colorize;
use component::{Component, ResourceTable};
use runner::gcov::GCovFile;
use wasmtime::*;
use wasmtime_wasi::{
    bindings::sync::exports::wasi::cli::run::Guest, WasiCtx, WasiCtxBuilder, WasiView,
};
use wat_annotator::data::{DebugData, DebugDataArc};
use wat_annotator::CounterType;

struct MyState {
    ctx: WasiCtx,
    table: ResourceTable,
    counters: Vec<i32>,
    debug_data: Option<DebugDataArc>,
    gcov_files: Option<HashMap<Arc<PathBuf>, GCovFile>>,
}

impl WasiView for MyState {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

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
use std::fmt::Display;
use std::rc::Rc;
use std::sync::Arc;
use std::{fs, io};
use std::{io::Read, path::PathBuf};

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
    output_files: Option<Vec<PathBuf>>,
}

fn main() -> wasmtime::Result<()> {
    let mut cli = Cli::parse();
    if cli.path.is_none() && cli.bytes.is_none() {
        // try read text from stdin
        let mut buffer = Vec::new();
        let mut stdin = io::stdin();
        stdin
            .read_to_end(&mut buffer)
            .map_err(|e| wasmtime::Error::new(e))?;
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
            serde_json::from_slice::<wat_annotator::data::DebugData>(
                &fs::read(p).map_err(wasmtime::Error::new)?,
            )
            .map_err(wasmtime::Error::new)
        })
        .map_or(Ok(None), |v| v.map(Some))?;
    let gcov_files = file_map.as_ref().map(|_| HashMap::new());

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
        },
    );

    linker.root().func_wrap(
        "inc-counter",
        |mut store, args: (i32, i32, i32, i32, i32)| {
            let (idx, ty, file_idx, line_num, col_num) = (
                args.0 as usize,
                CounterType::from_i32(args.1).unwrap(),
                args.2,
                args.3,
                args.4,
            );
            let counters = &mut store.data_mut().counters;
            if counters.len() <= idx {
                counters.extend(
                    ConstantIterator::<i32>::new_default_value(idx - counters.len() + 1)
                        .into_iter(),
                );
            }
            let data = store.data_mut();
            if let Some(map) = data.gcov_files.as_mut() {
                let debug_data = data.debug_data.as_ref().unwrap();
                let path = &debug_data.file_map[file_idx as usize];
                if !map.contains_key(path) {
                    map.insert(path.clone(), GCovFile::new(path.clone(), &debug_data));
                }
                let gcov_file = map.get_mut(path).unwrap();
                gcov_file.increment(line_num as u64, col_num as u64);
            }

            let file = if let Some(debug_data) = &store.data().debug_data {
                Box::new(debug_data.file_map[file_idx as usize].display()) as Box<dyn Display>
            } else {
                Box::new(format!("IDX#{}", file_idx)) as Box<dyn Display>
            };
            println!(
                "{} {} {} {} {}",
                "RUNNER HOST:".red(),
                format!("Accessed idx #{}, type:", idx).dimmed(),
                format!("%{}", ty).green(),
                format!(", source line number:").dimmed(),
                format!("@{}:{}:{}", file, line_num, col_num).yellow(),
            );
            Ok(())
        },
    )?;

    let component = Component::new(&engine, &bytes)?;

    let instance = linker.instantiate(&mut store, &component)?;
    let mut exports = instance.exports(&mut store);
    let guest = Guest::new(&mut exports.instance("wasi:cli/run@0.2.0").ok_or(
        wasmtime::Error::msg("couldn't find export instance wasi:cli/run"),
    )?)?;
    drop(exports); // sucks -_-

    guest
        .call_run(&mut store)?
        .map_err(|_| wasmtime::Error::msg("running code returned error"))?;

    if let Some(outputs) = cli.output_files {
        for file in outputs {
            if let Some(gcov) = store
                .data()
                .gcov_files
                .as_ref()
                .unwrap()
                .get(&file.canonicalize().unwrap())
            {
                println!("{}:\n{}", file.display(), gcov);
            } else {
                eprintln!(
                    "Requested output file not found in source files! Requested file: {}, source files: {:?}",
                    file.display(), store.data().gcov_files.as_ref().unwrap().keys()
                );
            }
        }
    }
    Ok(())
}
