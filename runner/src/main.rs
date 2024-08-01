use colored::Colorize;
use component::{Component, ResourceTable};
use wasmtime::*;
use wasmtime_wasi::{
    bindings::sync::exports::wasi::cli::run::Guest, WasiCtx, WasiCtxBuilder, WasiView,
};
use wat_annotator::CounterType;

struct MyState {
    ctx: WasiCtx,
    table: ResourceTable,
    counters: Vec<i32>,
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

use std::{fs, io};
use std::{io::Read, path::PathBuf};

use clap::{ArgGroup, Parser};
#[derive(Parser)]
#[clap(group(
    ArgGroup::new("input")
        .args(&["path", "text"])
))]
struct Cli {
    #[arg(short, long, value_name = "FILE")]
    path: Option<PathBuf>,

    #[arg(short, long, value_name = "BYTES")]
    bytes: Option<Vec<u8>>,
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
        },
    );

    linker
        .root()
        .func_wrap("inc-counter", |mut store, args: (i32, i32, i32)| {
            let (idx, ty, line_num) = (
                args.0 as usize,
                CounterType::from_i32(args.1).unwrap(),
                args.1,
            );
            let counters = &mut store.data_mut().counters;
            if counters.len() <= idx {
                counters.extend(
                    ConstantIterator::<i32>::new_default_value(idx - counters.len() + 1)
                        .into_iter(),
                );
            }
            println!(
                "{} {} {} {} {}",
                "RUNNER HOST:".red(),
                format!("Accessed idx #{}, type:", idx).dimmed(),
                format!("%{}", ty).green(),
                format!(", source line number:").dimmed(),
                format!("@{}", line_num).yellow(),
            );
            Ok(())
        })?;

    let component = Component::new(&engine, &bytes)?;

    let instance = linker.instantiate(&mut store, &component)?;
    let mut exports = instance.exports(&mut store);
    let guest = Guest::new(&mut exports.instance("wasi:cli/run@0.2.0").ok_or(
        wasmtime::Error::msg("couldn't find export instance wasi:cli/run"),
    )?)?;
    drop(exports); // sucks -_-

    guest
        .call_run(&mut store)?
        .map_err(|_| wasmtime::Error::msg("running code returned error"))
}
