use colored::Colorize;
use component::{Component, ResourceTable};
use wasmtime::*;
use wasmtime_wasi::{
    bindings::sync::exports::wasi::cli::run::Guest, WasiCtx, WasiCtxBuilder, WasiView,
};

const FILE_BYTES: &[u8] = include_bytes!("../../modified.wat");

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
    fn new(value: T, count: usize) -> ConstantIterator<T> {
        ConstantIterator { value, count }
    }

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

fn main() -> wasmtime::Result<()> {
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
        .func_wrap("inc-counter", |mut store, idx: (i32,)| {
            let idx = idx.0 as usize;
            let counters = &mut store.data_mut().counters;
            if counters.len() <= idx {
                counters.extend(
                    ConstantIterator::<i32>::new_default_value(idx - counters.len() + 1)
                        .into_iter(),
                );
            }
            println!(
                "{} {}",
                "RUNNER HOST:".red(),
                format!("Accessed idx #{}", idx).dimmed()
            );
            Ok(())
        })?;

    let component = Component::new(&engine, FILE_BYTES)?;

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
