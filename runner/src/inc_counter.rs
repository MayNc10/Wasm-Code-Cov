//! This module contains the code for the `inc-counter` function that modified Wasm component will call out to
use std::fmt::Display;

use colored::Colorize;
use wasmtime::StoreContextMut;
use wat_annotator::CounterType;

use crate::{gcov::GCovFile, store, ConstantIterator};

/// The `inc-counter` function that modified Wasm component will call out to
pub fn inc_counter(
    mut store: StoreContextMut<store::MyState>,
    args: (i32, i32, i32, i32, i32),
    verbose: bool,
) -> wasmtime::Result<()> {
    let (idx, ty, file_idx, line_num, col_num) = (
        args.0 as usize,
        CounterType::from_i32(args.1).unwrap(),
        args.2 as usize,
        args.3,
        args.4,
    );
    let counters = &mut store.data_mut().counters;
    if counters.len() <= idx {
        counters.extend(ConstantIterator::<i32>::new_default_value(
            idx - counters.len() + 1,
        ));
    }
    let data = store.data_mut();
    if let Some(map) = data.gcov_files.as_mut() {
        let debug_data = data.debug_data.as_ref().unwrap();
        let path = &debug_data.file_map[file_idx];
        if !map.contains_key(path) {
            map.insert(path.clone(), GCovFile::new(debug_data, file_idx));
        }
        let gcov_file = map.get_mut(path).unwrap();
        gcov_file.increment(line_num as u64, col_num as u64);
    }

    let file = if let Some(debug_data) = &store.data().debug_data {
        Box::new(debug_data.file_map[file_idx].display()) as Box<dyn Display>
    } else {
        Box::new(format!("IDX#{}", file_idx)) as Box<dyn Display>
    };

    if verbose {
        println!(
            "{} {} {} {} {}",
            "RUNNER:".red(),
            format!("Accessed idx #{}, type:", idx).dimmed(),
            format!("%{}", ty).green(),
            ", source line number:".dimmed(),
            format!("@{}:{}:{}", file, line_num, col_num).yellow(),
        );
    }

    Ok(())
}
