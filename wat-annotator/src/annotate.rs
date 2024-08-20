use core::str;
use std::borrow::Cow;

use regex::Regex;
use wast::core::{ExportKind, Func, Instruction, ModuleField};
use wast::parser::{parse, ParseBuffer};
use wast::token::Index;
use wast::{component::*, Wat};
use wast::{parser, Error};

use crate::data::DebugDataOwned;
use crate::debug::{find_code_offsets, read_dbg_info, SourceDebugInfo, WatLineMapper};
use crate::offset_tracker::OffsetTracker;
use crate::utils::*;

const INSTANTIATION_REGEX_STR: &str = r"core instance \(;[0-9]+;\) \(instantiate [0-9]+";
const BINARY_OFFSET_REGEX_STR: &str = r"(?P<whole>\(;@(?P<hex>[0-9a-f]+)\s*;\))";
const INC_FUNC_NAME: &str = "inc-counter";
const INC_MODULE_NAME: &str = "inc-counter-module";
// Is there a good way to ensure that these are always compatible? maybe a macro
const INC_FUNC_DESC_COMP: &str =
    "(param \"idx\" s32) (param \"type\" s32) (param \"file-idx\" s32) (param \"line-num\" s32) (param \"column\" s32)";
const INC_FUNC_DESC_CORE: &str = "(param i32) (param i32) (param i32) (param i32) (param i32)";

/// Accepts the text of a Wat file (and optionally the bytes of a binary Wasm file), and outputs a modified Wat file, as well as some debugging information
/// If the binary file is not provided, this function will compile it from the Wat tezt (this adds extra time)
pub fn add_scaffolding(
    wat_text: String,
    binary: Option<Cow<[u8]>>,
    verbose: bool,
) -> parser::Result<(String, DebugDataOwned)> {
    // Things to do: (in order)
    // Add import statement for the inc counter function (in type and import section)
    // Add import statements within each module
    // Add function calls wherever we want
    // Bump the index of any instance reference
    // Bump the index of any component function
    // Bump the index of any core function
    // Add canon lower of inc counter func (right after module sections)
    // Add instance exporting that core function
    let mut output = wat_text.clone();
    let mut buf = ParseBuffer::new(&wat_text)?;
    let buf = buf.track_instr_spans(true);
    let wat = parse::<Wat>(buf)?;
    let mut total_increment = OffsetTracker::new();

    let binary = if binary.is_some() {
        binary.unwrap()
    } else {
        Cow::Owned(parse::<Wat>(&ParseBuffer::new(&wat_text)?)?.encode()?)
    };

    let mut wat_mapper = WatLineMapper::new(
        find_code_offsets(&binary)
            .map_err(|_| Error::new(wat.span(), "Error reading binary file".to_string()))?,
    );
    read_dbg_info(&wat, &wat_text, &mut wat_mapper, verbose)?;

    let type_idx_bound = add_inc_import_section(&wat, &mut output, &mut total_increment)?;
    add_imports_in_module(&wat, &mut output, &mut total_increment)?;
    {
        let bl = bump_core_func_idxs(&wat, &mut output, &mut total_increment)?;
        // process blacklisted functions
        let bl = process_blacklist(&wat, bl, verbose)?;
        add_func_calls(
            &wat,
            &mut output,
            &mut total_increment,
            bl,
            &wat_mapper,
            &wat_text,
            verbose,
        )?;
    }

    bump_instance_idxs(&wat, &mut output, &mut total_increment)?;
    bump_comp_func_idxs(&wat, &mut output, &mut total_increment, verbose)?;
    bump_type_idxs(&wat, &mut output, &mut total_increment, type_idx_bound)?;
    add_instantiaion_arg(&wat, &mut output, &mut total_increment, verbose)?;
    //panic!("erm.. what the bug");
    add_canon_lower_and_instance(&wat, &mut output, &mut total_increment)?;
    Ok((output, wat_mapper.into_debug_data()))
}

/// Adds the instructions that import the host functions
pub fn add_inc_import_section(
    wat: &Wat,
    output: &mut String,
    total_increment: &mut OffsetTracker,
) -> parser::Result<u32> {
    let mut was_last_ty_import_alias = false;
    let mut offset = 0;
    let mut type_idx = 0;
    'field: for field in get_fields(&wat).ok_or(Error::new(
        wat.span(),
        "Input WAT file could not be parsed (may be binary or module)".to_string(),
    ))? {
        match field {
            ComponentField::Type(_) | ComponentField::Import(_) | ComponentField::Alias(_) => {
                was_last_ty_import_alias = true;
                if let ComponentField::Type(_) = field {
                    type_idx += 1;
                } else if let ComponentField::Alias(alias) = field {
                    if let AliasTarget::Export { kind, .. } = alias.target {
                        if matches!(kind, ComponentExportAliasKind::Type) {
                            type_idx += 1;
                        }
                    }
                }
            }
            _ => {
                if was_last_ty_import_alias && get_span(field).is_some() {
                    offset = get_span(field).unwrap().offset() - 1;
                    break 'field;
                }
            }
        }
    }

    let msg = format!(
        "(import \"{0}\" (func ${0} {1}))",
        INC_FUNC_NAME, INC_FUNC_DESC_COMP
    );
    total_increment.add_to_string(output, offset, &msg);

    Ok(type_idx)
}

/// Adds function imports to each inline module
pub fn add_imports_in_module(
    wat: &Wat,
    output: &mut String,
    total_increment: &mut OffsetTracker,
) -> parser::Result<()> {
    'comp: for field in get_fields(&wat).ok_or(Error::new(
        wat.span(),
        "Input WAT file could not be parsed (may be binary or module)".to_string(),
    ))? {
        if let ComponentField::CoreModule(m) = field {
            let mut offset = 0;
            // parse module fields
            if let CoreModuleKind::Inline { fields } = &m.kind {
                let mut was_last_import = false;
                'module: for field in fields {
                    if let ModuleField::Import(_) = field {
                        was_last_import = true
                    } else if was_last_import {
                        was_last_import = false;
                        if let Some(span) = get_module_span(field) {
                            offset = span.offset() - 1;
                            break 'module;
                        }
                    }
                }
            }
            if offset == 0 {
                continue 'comp;
            }
            let msg = format!(
                "(import \"{0}\" \"{1}\" (func ${1} {2}))\n",
                INC_MODULE_NAME, INC_FUNC_NAME, INC_FUNC_DESC_CORE
            );

            total_increment.add_to_string(output, offset, &msg);
        }
    }

    Ok(())
}

/// Adds function calls to control flow instructions
/// The blacklist argument specifies functions that should not have calls inserted
/// This is mostly used to ensure that `realloc` functions don't call other functions outside their instances, which is an error in Webassembly
pub fn add_func_calls<'a>(
    wat: &'a Wat,
    output: &mut String,
    total_increment: &mut OffsetTracker,
    blacklist: Vec<(Index<'a>, &'a Func<'a>)>,
    map: &WatLineMapper,
    text: &str,
    verbose: bool,
) -> parser::Result<()> {
    let mut counter_idx = 0;
    let mut inline_mod_idx = 0;
    let mut sdi_iter = None;
    let mut line_addrs_inserted = Vec::new();
    let binary_offset_re = Regex::new(BINARY_OFFSET_REGEX_STR).unwrap();
    for field in get_fields(&wat).ok_or(Error::new(
        wat.span(),
        "Input WAT file could not be parsed (may be binary or module)".to_string(),
    ))? {
        if let ComponentField::CoreModule(m) = field {
            // parse module fields
            if let CoreModuleKind::Inline { fields } = &m.kind {
                'fields: for field in fields {
                    if let ModuleField::Func(func) = field {
                        if blacklist
                            .iter()
                            .filter(|(_, f)| f.span == func.span)
                            .next()
                            .is_some()
                        {
                            continue;
                        }
                        if verbose {
                            println!("Func defined @{}", func.span.offset());
                        }

                        if let wast::core::FuncKind::Inline {
                            locals: _,
                            expression,
                        } = &func.kind
                        {
                            let _instrs = &expression.instrs;
                            let spans = expression.instr_spans.as_ref().unwrap();
                            if spans.len() == 0 {
                                continue 'fields;
                            }

                            let lines = map
                                .lines()
                                .iter()
                                .filter(|dli| dli.code_module_idx == inline_mod_idx);
                            if let Some(mod_offset) = map.get_code_addr(inline_mod_idx) {
                                for line in lines {
                                    if !sdi_iter.as_ref().is_some_and(|sdi_iter: &Vec<_>| {
                                        sdi_iter.iter().next().is_some_and(
                                            |n: &&SourceDebugInfo| n.path_idx == line.path_idx,
                                        )
                                    }) {
                                        sdi_iter = Some(
                                            map.sdi_vec
                                                .iter()
                                                .filter(|sdi| sdi.path_idx == line.path_idx)
                                                .collect(),
                                        );
                                    }

                                    let func_at = sdi_iter
                                        .as_ref()
                                        .unwrap()
                                        .iter()
                                        .filter_map(|sdi| {
                                            sdi.functions
                                                .iter()
                                                .filter(|sdi_func| line.address == sdi_func.3)
                                                .next()
                                        })
                                        .next();

                                    if line_addrs_inserted.contains(&(line.address)) {
                                        continue;
                                    }

                                    let true_bin_addr = mod_offset as u64 + line.address;
                                    let txt_line = str::from_utf8(
                                        if let Some(end) = spans.last().map(|s| s.offset()) {
                                            &text.as_bytes()[func.span.offset()..end + 1]
                                        } else {
                                            &text.as_bytes()[func.span.offset()..]
                                        },
                                    )
                                    .unwrap();

                                    let hex_iter =
                                        binary_offset_re.captures_iter(txt_line).map(|c| {
                                            let m = c.name("hex").unwrap();
                                            let bin_offset =
                                                u64::from_str_radix(m.as_str(), 16).unwrap();
                                            let m_whole = c.name("whole").unwrap();
                                            let txt_offset = m_whole.end();
                                            (bin_offset, txt_offset)
                                        });

                                    let text_offset = if func_at.is_some() {
                                        let hexes = hex_iter.collect::<Vec<_>>();

                                        // If the byte ranges "bound" or "surround" the function address, we know this is the function
                                        if hexes
                                            .iter()
                                            .filter(|(off, _)| {
                                                *off >= func_at.unwrap().3 + mod_offset as u64
                                            })
                                            .count()
                                            > 0
                                        {
                                            if verbose {
                                                println!("USING FUNC START, spans: {}, func: {}, name: {}, dli: {:?}", 
                                                spans.first().unwrap().offset() , func.span.offset(), func_at.unwrap().2, line);
                                            }

                                            spans.first().unwrap().offset()
                                        } else {
                                            let Some(text_offset) = hexes
                                                .iter()
                                                .filter(|(x, _)| *x == true_bin_addr)
                                                .min_by(|(b1, _), (b2, _)| b1.cmp(b2))
                                            else {
                                                continue;
                                            };
                                            text_offset.1 + func.span.offset()
                                        }
                                    } else {
                                        let Some(text_offset) = hex_iter
                                            .filter(|(x, _)| *x == true_bin_addr)
                                            .min_by(|(b1, _), (b2, _)| b1.cmp(b2))
                                        else {
                                            continue;
                                        };
                                        text_offset.1 + func.span.offset()
                                    };

                                    let msg = format!(
                                        "i32.const {} i32.const {} i32.const {} i32.const {} i32.const {} call ${}\n",
                                        counter_idx, 0, line.path_idx, line.line, line.column, INC_FUNC_NAME
                                    );

                                    counter_idx += 1;

                                    total_increment.add_to_string(output, text_offset, &msg);

                                    line_addrs_inserted.push(line.address);
                                }
                            }
                        }
                    }
                }

                inline_mod_idx += 1;
            }
        }
    }

    Ok(())
}

/// Increase all instance indices to ensure they point to the correct instances
pub fn bump_instance_idxs(
    wat: &Wat,
    output: &mut String,
    total_increment: &mut OffsetTracker,
) -> parser::Result<()> {
    // instance idxs are used in:
    // alias export statements
    // instantiation args

    for field in get_fields(&wat).ok_or(Error::new(
        wat.span(),
        "Input WAT file could not be parsed (may be binary or module)".to_string(),
    ))? {
        match field {
            ComponentField::Alias(a) => match a.target {
                AliasTarget::CoreExport { instance: idx, .. } => {
                    // TODO Figure out lower bound flexibly

                    total_increment.increment_idx(output, idx, None);
                }
                AliasTarget::Export { instance: _idx, .. } => {}
                _ => {}
            },
            ComponentField::Instance(i) => {
                if let InstanceKind::Instantiate { component: _, args } = &i.kind {
                    for arg in args {
                        if let InstantiationArgKind::Item(cek) = &arg.kind {
                            // Other things here need to be bumped, but that happens in other functions
                            if let ComponentExportKind::Instance(i) = cek {
                                // TODO Figure out lower bound flexibly
                                total_increment.increment_idx(output, i.idx, None);
                            }
                        }
                    }
                }
            }
            ComponentField::CoreInstance(i) => match &i.kind {
                CoreInstanceKind::Instantiate { module: _, args } => {
                    for arg in args {
                        match &arg.kind {
                            // I think this is the right match for an instance arg
                            CoreInstantiationArgKind::Instance(iref) => {
                                total_increment.increment_idx(output, iref.idx, None);
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            },

            _ => {}
        }
    }

    Ok(())
}

/// Increase all component function indices to ensure they point to the correct functions
pub fn bump_comp_func_idxs(
    wat: &Wat,
    output: &mut String,
    total_increment: &mut OffsetTracker,
    verbose: bool,
) -> parser::Result<()> {
    // What to bump
    // canon lower <idx>
    // (instantiate $instance (with "func" (func <idx>)))
    // (instantiate $instance (export "func" (func <idx>)))

    for field in get_fields(&wat).ok_or(Error::new(
        wat.span(),
        "Input WAT file could not be parsed (may be binary or module)".to_string(),
    ))? {
        match field {
            ComponentField::CoreFunc(cf) => match &cf.kind {
                CoreFuncKind::Lower(cl) => total_increment.increment_idx(output, cl.func.idx, None),
                _ => {}
            },
            ComponentField::CoreInstance(i) => match &i.kind {
                CoreInstanceKind::Instantiate { module: _, args } => {
                    for arg in args {
                        match &arg.kind {
                            // I think this is the right match for an instance arg
                            CoreInstantiationArgKind::Instance(_) => {
                                // no-op
                            }
                            CoreInstantiationArgKind::BundleOfExports(_, exports) => {
                                for export in exports {
                                    match &export.item.kind {
                                        ExportKind::Func => {
                                            total_increment.increment_idx(
                                                output,
                                                export.item.idx,
                                                None,
                                            );
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
                CoreInstanceKind::BundleOfExports(exps) => {
                    for export in exps {
                        match export.item.kind {
                            ExportKind::Func => {
                                total_increment.increment_idx(output, export.item.idx, None);
                            }
                            _ => {}
                        }
                    }
                }
            },
            ComponentField::Instance(i) => match &i.kind {
                InstanceKind::Instantiate { component, args } => {
                    if verbose {
                        println!("comp: {:?}, args: {:?}", component, args);
                    }
                    for arg in args {
                        match &arg.kind {
                            // I think this is the right match for an instance arg
                            InstantiationArgKind::Item(item) => match item {
                                ComponentExportKind::Func(cf) => {
                                    total_increment.increment_idx(output, cf.idx, None);
                                }
                                _ => {}
                            },
                            InstantiationArgKind::BundleOfExports(_, exports) => {
                                for export in exports {
                                    match &export.kind {
                                        ComponentExportKind::Func(cf) => {
                                            total_increment.increment_idx(output, cf.idx, None);
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    Ok(())
}

/// Increase all core function indices to ensure they point to the correct functions
pub fn bump_core_func_idxs<'a, 'b: 'a, 'c>(
    wat: &'b Wat,
    output: &'c mut String,
    total_increment: &'c mut OffsetTracker,
) -> parser::Result<Vec<Index<'a>>> {
    // What to bump
    // (realloc <funcidx>)
    // ((canon lift (core func <funcidx>)))

    let mut bl = Vec::new();
    for field in get_fields(&wat).ok_or(Error::new(
        wat.span(),
        "Input WAT file could not be parsed (may be binary or module)".to_string(),
    ))? {
        match field {
            ComponentField::CoreFunc(cf) => match &cf.kind {
                CoreFuncKind::Lower(cl) => {
                    // The func contained is a comp func, we want to find the realloc optioon
                    for opt in &cl.opts {
                        match opt {
                            CanonOpt::Realloc(re) => {
                                total_increment.increment_idx(output, re.idx, None);
                                bl.push(re.idx);
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            },
            ComponentField::Func(f) => match &f.kind {
                FuncKind::Lift { ty: _, info } => {
                    total_increment.increment_idx(output, info.func.idx, None);
                }
                _ => {}
            },
            _ => {}
        }
    }

    Ok(bl)
}

/// Take an initial function blacklist and extend it
/// Functions are blacklisted because they should not make external calls
/// This also means that functions they call should *also* not make external calls
/// This function adds functions to the blacklist if they are called by blacklisted functions
/// This happens recursively until all blacklisted functions are listed
pub fn process_blacklist<'a, 'b: 'a>(
    wat: &'a Wat,
    blacklist: Vec<Index<'a>>,
    verbose: bool,
) -> parser::Result<Vec<(Index<'a>, &'a Func<'a>)>> {
    // logic:
    // we have a vector of export references
    // that label the name of the export and the module where it was defined
    // we have a queue of functions to get done
    // we go through every function in the queue
    // finding its moduledef and which function it corresponds to
    // by finding the export statement that exports the name we'ere looking for

    let queue = map_idx_to_module(wat, blacklist, verbose)?;
    let mut blacklist: Vec<(Index, &Func)> = Vec::new();
    let fields = get_fields(&wat).ok_or(Error::new(
        wat.span(),
        "Input WAT file could not be parsed (may be binary or module)".to_string(),
    ))?;
    // create a module map
    let mut mods = Vec::new();
    for field in fields {
        if let ComponentField::CoreModule(m) = field {
            mods.push(m);
        }
    }
    // helper function
    fn idx_to_func<'a, 'b: 'a, 'c: 'b>(
        idx: Index<'a>,
        fields: &'c Vec<ModuleField<'a>>,
    ) -> Option<&'a Func<'a>> {
        let mut func = None;
        match idx {
            Index::Id(id) => {
                for field in fields {
                    if let ModuleField::Func(f) = field {
                        if let Some(f_id) = f.id {
                            if f_id == id {
                                func = Some(f);
                                break;
                            }
                        }
                    }
                }
            }
            Index::Num(num, _) => {
                let mut idx = 0;
                for field in fields {
                    if let ModuleField::Func(f) = field {
                        if idx == num {
                            func = Some(f);
                            break;
                        }
                        idx += 1;
                    }
                }
            }
        }
        func
    }
    // map exports to function idxs
    let mut queue = queue
        .into_iter()
        .map(|(mod_idx, export_name)| {
            if let Index::Num(num, _) = mod_idx {
                if let CoreModuleKind::Inline { fields } = &mods[num as usize].kind {
                    // find the export we're looking for
                    let mut func = None;
                    for field in fields {
                        if let ModuleField::Export(exp) = field {
                            if exp.name == export_name && exp.kind == ExportKind::Func {
                                // do code
                                let func_idx = exp.item;
                                func = idx_to_func(func_idx, fields);
                                if func.is_some() {
                                    break;
                                }
                            }
                        }
                    }
                    let func = func.unwrap();
                    (mod_idx, func)
                } else {
                    todo!()
                }
            } else {
                todo!()
            }
        })
        .collect::<Vec<_>>();
    // Now we have an iterator of module references and functions
    // Now we go through each func,
    while let Some((mod_idx, func)) = queue.pop() {
        if verbose {
            println!("Blacklisting func id: {:?}, name: {:?}", func.id, func.name);
        }

        if let wast::core::FuncKind::Inline {
            locals: _,
            expression,
        } = &func.kind
        {
            // we can do this comparison based on spans i think
            // bc they should be unique to the function
            if blacklist
                .iter()
                .filter(|(_, f)| f.span == func.span)
                .next()
                .is_some()
            {
                continue;
            }

            //  the todos can be here bc like
            // if we already have a funcref then this kinda has to be a core inline module
            // but we can rewrite later
            if let Index::Num(num, _) = mod_idx {
                if let CoreModuleKind::Inline { fields } = &mods[num as usize].kind {
                    for instr in &*expression.instrs {
                        if let Instruction::Call(f_idx) = instr {
                            // find the function reference, which will be part of the current module
                            let new_func = idx_to_func(*f_idx, fields);
                            if new_func.is_none() {
                                // Most likely an import
                                // For now, we can just decide to skip adding it
                                // We would do this anyway bc once it calls out to an import it's leaving the instance anyway
                                continue;
                            }
                            let new_func = new_func.unwrap();
                            queue.push((mod_idx, new_func));
                        }
                    }
                } else {
                    todo!()
                }
            } else {
                println!("Module index: ${:?}", mod_idx);
                todo!()
            }

            blacklist.push((mod_idx, func));
            if verbose {
                println!(
                    "Finished blacklisting func id: {:?}, name: {:?}",
                    func.id, func.name
                );
            }
        }
    }

    Ok(blacklist)
}

/// Given a blacklist of export indices, map them to a blacklist of indices and functio names
fn map_idx_to_module<'a, 'b: 'a>(
    wat: &'a Wat,
    blacklist: Vec<Index<'a>>,
    verbose: bool,
) -> parser::Result<Vec<(Index<'a>, &'a str)>> {
    let mut out = Vec::new();
    let mut core_func_idx = 0;
    let mut core_instances: Vec<Option<&ItemRef<_>>> = Vec::new();
    for field in get_fields(&wat).ok_or(Error::new(
        wat.span(),
        "Input WAT file could not be parsed (may be binary or module)".to_string(),
    ))? {
        match field {
            ComponentField::Alias(a) => match a.target {
                AliasTarget::CoreExport {
                    instance,
                    name,
                    kind,
                } => {
                    if kind == ExportKind::Func {
                        if blacklist
                            .iter()
                            .filter(|idx| match idx {
                                Index::Num(num, _) => *num == core_func_idx,
                                Index::Id(_) => todo!(),
                            })
                            .next()
                            .is_some()
                        {
                            // do something with the instance info
                            let idx = match instance {
                                Index::Num(num, _) => num,
                                Index::Id(_) => todo!(),
                            } as usize;
                            let instance_ref: &ItemRef<_> = core_instances[idx].unwrap();
                            out.push((instance_ref.idx, name));
                        }
                        core_func_idx += 1;
                    }
                }
                _ => {}
            },
            ComponentField::CoreFunc(_) => {
                if verbose {
                    eprintln!("TODO: parse core func");
                }
                core_func_idx += 1;
            }
            ComponentField::CoreInstance(i) => match &i.kind {
                CoreInstanceKind::Instantiate { module, .. } => {
                    core_instances.push(Some(module));
                }
                CoreInstanceKind::BundleOfExports(_) => {
                    core_instances.push(None);
                }
            },
            _ => {}
        }
    }
    Ok(out)
}

/// Increase all type indices to ensure they point to the correct types
pub fn bump_type_idxs(
    wat: &Wat,
    output: &mut String,
    total_increment: &mut OffsetTracker,
    lower_bound: u32,
) -> parser::Result<()> {
    // Things to bump
    // canon resource.drop
    // type( func (result $ty))
    // func (type $ty)
    // i think thats it

    // TODO compute this instead of hardcoding
    for field in get_fields(&wat).ok_or(Error::new(
        wat.span(),
        "Input WAT file could not be parsed (may be binary or module)".to_string(),
    ))? {
        // function to recurse for type structs
        fn idxs_in_type<'a>(ty: &'a Type) -> Vec<Index<'a>> {
            let mut idxs = Vec::new();
            match &ty.def {
                TypeDef::Defined(_) => (),
                TypeDef::Func(f) => {
                    for param in f.params.iter() {
                        if let ComponentValType::Ref(idx) = param.ty {
                            idxs.push(idx);
                        }
                    }
                    for result in f.results.iter() {
                        if let ComponentValType::Ref(idx) = result.ty {
                            idxs.push(idx);
                        }
                    }
                }
                TypeDef::Component(c) => {
                    for decl in &c.decls {
                        match decl {
                            ComponentTypeDecl::CoreType(_ct) => {}
                            ComponentTypeDecl::Type(ty) => idxs.append(&mut idxs_in_type(ty)),
                            ComponentTypeDecl::Alias(_) => {}
                            ComponentTypeDecl::Import(_i) => {}
                            ComponentTypeDecl::Export(_e) => {}
                        }
                    }
                }
                TypeDef::Instance(_i) => {}
                TypeDef::Resource(_r) => {}
            }
            idxs
        }

        match field {
            ComponentField::CoreFunc(f) => {
                if let CoreFuncKind::ResourceDrop(rd) = &f.kind {
                    total_increment.increment_idx(output, rd.ty, Some(lower_bound));
                }
            }
            // make this a function in this scope ig
            ComponentField::Type(t) => {
                let idxs = idxs_in_type(&t);
                for idx in idxs {
                    total_increment.increment_idx(output, idx, Some(lower_bound));
                }
            }
            ComponentField::Func(f) => {
                if let FuncKind::Lift { ty, .. } = &f.kind {
                    if let ComponentTypeUse::Ref(r) = ty {
                        total_increment.increment_idx(output, r.idx, Some(lower_bound));
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

/// Add the wrapper instance to the instatiation calls of all other instances
pub fn add_instantiaion_arg(
    wat: &Wat,
    output: &mut String,
    total_increment: &mut OffsetTracker,
    verbose: bool,
) -> parser::Result<()> {
    for field in get_fields(&wat).ok_or(Error::new(
        wat.span(),
        "Input WAT file could not be parsed (may be binary or module)".to_string(),
    ))? {
        match field {
            ComponentField::CoreInstance(ci) => match &ci.kind {
                CoreInstanceKind::Instantiate { .. } => {
                    if verbose {
                        println!("Core instance: {ci:?}, offset: {}", ci.span.offset());
                    }
                    // parse with regex
                    let msg = format!(
                        "(with \"{}\" (instance ${}))",
                        INC_MODULE_NAME, INC_MODULE_NAME
                    );
                    let re = Regex::new(INSTANTIATION_REGEX_STR).unwrap();
                    let c = |s: &mut String, _, end| {
                        s.insert_str(end, &msg);
                        (end, msg.len())
                    };
                    total_increment.modify_with_regex_match(output, &re, ci.span.offset(), c);
                }
                _ => {}
            },
            _ => {}
        }
    }
    Ok(())
}

/// Ad the functions to lower the imported function and wrap it in an instance
pub fn add_canon_lower_and_instance(
    wat: &Wat,
    output: &mut String,
    total_increment: &mut OffsetTracker,
) -> parser::Result<()> {
    let canon_lower = format!("(core func ${0} (canon lower (func ${0})))", INC_FUNC_NAME);
    let instantiate = format!(
        "(core instance ${0} (export \"{1}\" (func ${1})))",
        INC_MODULE_NAME, INC_FUNC_NAME
    );

    let mut has_passed_modules = false;
    let mut offset = None;
    for field in get_fields(&wat).ok_or(Error::new(
        wat.span(),
        "Input WAT file could not be parsed (may be binary or module)".to_string(),
    ))? {
        match field {
            ComponentField::CoreModule(_) => has_passed_modules = true,
            _ => {
                if has_passed_modules && get_span(field).is_some() {
                    offset = Some(get_span(field).unwrap().offset());
                    break;
                }
            }
        }
    }
    let offset = offset.unwrap() - 1;
    let msg = format!("{}\n{}\n", canon_lower, instantiate);
    total_increment.add_to_string(output, offset, &msg);

    Ok(())
}
