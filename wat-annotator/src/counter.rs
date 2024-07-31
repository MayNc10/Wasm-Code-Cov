use regex::Regex;
use wast::core::{ExportKind, Func, Instruction, ModuleField};
use wast::parser::{parse, ParseBuffer};
use wast::token::{Index, Span};
use wast::{component::*, Wat};
use wast::{parser, Error};

use crate::offset_tracker::OffsetTracker;

const INSTANTIATION_REGEX_STR: &str = r"core instance \(;[0-9]+;\) \(instantiate [0-9]+";
const INC_FUNC_NAME: &str = "inc-counter";
const INC_MODULE_NAME: &str = "inc-counter-module";

fn get_span(f: &ComponentField) -> Option<Span> {
    match f {
        ComponentField::CoreModule(cm) => Some(cm.span),
        ComponentField::CoreInstance(ci) => Some(ci.span),
        ComponentField::CoreType(ct) => Some(ct.span),
        ComponentField::Component(nc) => Some(nc.span),
        ComponentField::Instance(i) => Some(i.span),
        ComponentField::Alias(a) => Some(a.span),
        ComponentField::Type(t) => Some(t.span),
        ComponentField::CanonicalFunc(cf) => Some(cf.span),
        ComponentField::CoreFunc(cf) => Some(cf.span),
        ComponentField::Func(f) => Some(f.span),
        ComponentField::Start(_s) => None,
        ComponentField::Import(ci) => Some(ci.span),
        ComponentField::Export(ce) => Some(ce.span),
        ComponentField::Custom(c) => Some(c.span),
        ComponentField::Producers(_p) => None,
    }
}

fn get_module_span(f: &ModuleField) -> Option<Span> {
    match f {
        ModuleField::Type(f) => Some(f.span),
        ModuleField::Rec(f) => Some(f.span),
        ModuleField::Import(f) => Some(f.span),
        ModuleField::Func(f) => Some(f.span),
        ModuleField::Table(f) => Some(f.span),
        ModuleField::Memory(f) => Some(f.span),
        ModuleField::Global(f) => Some(f.span),
        ModuleField::Export(f) => Some(f.span),
        ModuleField::Start(_) => None,
        ModuleField::Elem(f) => Some(f.span),
        ModuleField::Data(f) => Some(f.span),
        ModuleField::Tag(f) => Some(f.span),
        ModuleField::Custom(_) => None,
    }
}

pub fn find_idxs<'a>(wat: &'a Wat) -> Option<Vec<Index<'a>>> {
    let comp = match wat {
        Wat::Component(c) => c,
        Wat::Module(_) => return None,
    };

    let component_fields = match &comp.kind {
        ComponentKind::Text(f) => f,
        ComponentKind::Binary(_) => return None,
    };

    let mut idxs = Vec::new();

    for field in component_fields {
        match field {
            ComponentField::Alias(_) => {
                //eprintln!("Hit alias");
            }
            ComponentField::CanonicalFunc(cf) => {
                //eprintln!("Hit cf");
                // A canonical function can be a lowering of a component function
                // in which case, we need to get the span
                match &cf.kind {
                    CanonicalFuncKind::Lower(cl) => {
                        // I think this span is the span where the idx is used, not where its defined
                        if let Index::Num(..) = cl.func.idx {
                            idxs.push(cl.func.idx);
                        }
                    }
                    _ => {}
                }
            }
            ComponentField::Component(_) => {
                //eprintln!("Hit component");
            }
            ComponentField::CoreFunc(cf) => {
                //eprintln!("Hit core func");
                // Even though this is a 'core func', its in the top level of the component, so i think it uses the function idxs there
                if let CoreFuncKind::Lower(cl) = &cf.kind {
                    // I think this span is the span where the idx is used, not where its defined
                    if let Index::Num(..) = cl.func.idx {
                        idxs.push(cl.func.idx);
                    }
                }
                if let CoreFuncKind::ResourceDrop(rd) = &cf.kind {
                    if let Index::Num(..) = rd.ty {
                        idxs.push(rd.ty);
                    }
                }
            }
            ComponentField::CoreInstance(_) => {
                //eprintln!("Hit core instance");
            }
            ComponentField::CoreModule(_) => {
                //eprintln!("Hit core module");
            }
            ComponentField::CoreType(_) => {
                //eprintln!("Hit core type");
            }
            ComponentField::Custom(_) => {
                //eprintln!("Hit custom");
            }
            ComponentField::Export(_) => {
                //eprintln!("Hit export");
            }
            ComponentField::Func(_) => {
                //eprintln!("Hit func");
            }
            ComponentField::Import(_) => {
                //eprintln!("Hit import");
            }
            ComponentField::Instance(_) => {
                //eprintln!("Hit instance");
            }
            ComponentField::Producers(_) => {
                //eprintln!("Hit producers");
            }
            ComponentField::Start(s) => {
                //eprintln!("Hit start");
                if let Index::Num(..) = s.func {
                    idxs.push(s.func);
                }
            }
            ComponentField::Type(t) => {
                //eprintln!("Hit type");
                if let TypeDef::Resource(r) = &t.def {
                    if let Some(dtor) = &r.dtor {
                        if let Index::Num(..) = dtor.idx {
                            idxs.push(dtor.idx);
                        }
                    }
                }
            }
        }
    }
    //panic!("erm ... what the bug");

    Some(idxs)
}

pub fn get_fields<'a, 'b: 'a>(comp: &'b Wat<'a>) -> Option<&'a Vec<ComponentField<'a>>> {
    match comp {
        Wat::Module(_) => None,
        Wat::Component(comp) => match &comp.kind {
            ComponentKind::Binary(_) => None,
            ComponentKind::Text(v) => Some(v),
        },
    }
}

pub fn add_scaffolding(wat: String) -> parser::Result<String> {
    // Things to do: (in order)
    // Add import statement for the inc counter function (in type and import section)
    // Add import statements within each module
    // Add function calls wherever we want
    // Bump the index of any instance reference
    // Bump the index of any component function
    // Bump the index of any core function
    // Add canon lower of inc counter func (right after module sections)
    // Add instance exporting that core function
    let mut output = wat.clone();
    let mut buf = ParseBuffer::new(&wat)?;
    let buf = buf.track_instr_spans(true);
    let wat = parse::<Wat>(buf)?;
    let mut total_increment = OffsetTracker::new();

    add_inc_import_section(&wat, &mut output, &mut total_increment)?;
    add_imports_in_module(&wat, &mut output, &mut total_increment)?;
    {
        let bl = bump_core_func_idxs(&wat, &mut output, &mut total_increment)?;
        // process blacklisted functions
        let bl = process_blacklist(&wat, bl)?;
        add_func_calls(&wat, &mut output, &mut total_increment, bl)?;
    }

    bump_instance_idxs(&wat, &mut output, &mut total_increment)?;
    bump_comp_func_idxs(&wat, &mut output, &mut total_increment)?;
    bump_type_idxs(&wat, &mut output, &mut total_increment)?;
    add_instantiaion_arg(&wat, &mut output, &mut total_increment)?;
    //panic!("erm.. what the bug");
    add_canon_lower_and_instance(&wat, &mut output, &mut total_increment)?;
    Ok(output)
}

pub fn add_inc_import_section(
    wat: &Wat,
    output: &mut String,
    total_increment: &mut OffsetTracker,
) -> parser::Result<()> {
    let mut was_last_ty_import_alias = false;
    let mut offset = 0;
    'field: for field in get_fields(&wat).ok_or(Error::new(
        wat.span(),
        "Input WAT file could not be parsed (may be binary or module)".to_string(),
    ))? {
        match field {
            ComponentField::Type(_) | ComponentField::Import(_) | ComponentField::Alias(_) => {
                was_last_ty_import_alias = true;
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
        "(import \"{0}\" (func ${0} (param \"idx\" s32)))",
        INC_FUNC_NAME
    );
    total_increment.add_to_string(output, offset, &msg);

    Ok(())
}

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
            // FIXME: Account for modules that end after imports (somehow)
            if offset == 0 {
                eprintln!("Module had no fields after imports");
                continue 'comp;
            }
            let msg = format!(
                "(import \"{0}\" \"{1}\" (func ${1} (param i32)))\n",
                INC_MODULE_NAME, INC_FUNC_NAME
            );

            total_increment.add_to_string(output, offset, &msg);
        }
    }

    Ok(())
}

pub fn add_func_calls<'a>(
    wat: &'a Wat,
    output: &mut String,
    total_increment: &mut OffsetTracker,
    blacklist: Vec<(Index<'a>, &'a Func<'a>)>,
) -> parser::Result<()> {
    let mut counter_idx = 0;
    for field in get_fields(&wat).ok_or(Error::new(
        wat.span(),
        "Input WAT file could not be parsed (may be binary or module)".to_string(),
    ))? {
        if let ComponentField::CoreModule(m) = field {
            // parse module fields
            if let CoreModuleKind::Inline { fields } = &m.kind {
                for field in fields {
                    if let ModuleField::Func(func) = field {
                        if blacklist
                            .iter()
                            .filter(|(_, f)| f.span == func.span)
                            .next()
                            .is_some()
                        {
                            continue;
                        }
                        eprintln!("Func defined @{}", func.span.offset());

                        if let wast::core::FuncKind::Inline {
                            locals: _,
                            expression,
                        } = &func.kind
                        {
                            let instrs = &expression.instrs;
                            let spans = expression.instr_spans.as_ref().unwrap();
                            for idx in 0..instrs.len() {
                                match instrs[idx] {
                                    // We can add more later
                                    Instruction::Block(_)
                                    | Instruction::If(_)
                                    | Instruction::Else(_)
                                    | Instruction::Loop(_) => {
                                        // insert line here
                                        let msg = format!(
                                            "i32.const {} call ${}\n",
                                            counter_idx, INC_FUNC_NAME
                                        );
                                        counter_idx += 1;

                                        total_increment.add_to_string(
                                            output,
                                            spans[idx].offset() - 1,
                                            &msg,
                                        );
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

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

pub fn bump_comp_func_idxs(
    wat: &Wat,
    output: &mut String,
    total_increment: &mut OffsetTracker,
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
                    eprintln!("comp: {:?}, args: {:?}", component, args);
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

pub fn process_blacklist<'a, 'b: 'a>(
    wat: &'a Wat,
    blacklist: Vec<Index<'a>>,
) -> parser::Result<Vec<(Index<'a>, &'a Func<'a>)>> {
    // logic:
    // we have a vector of export references
    // that label the name of the export and the module where it was defined
    // we have a queue of functions to get done
    // we go through every function in the queue
    // finding its moduledef and which function it corresponds to
    // by finding the export statement that exports the name we'ere looking for

    let queue = map_idx_to_module(wat, blacklist)?;
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
        eprintln!("Blacklisting func id: {:?}, name: {:?}", func.id, func.name);

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

            let debug = func.id.is_some_and(|id| id.name() == "allocate_stack");
            if debug {
                eprintln!("IN CABI REALLOC");
            }

            //  the todos can be here bc like
            // if we already have a funcref then this kinda has to be a core inline module
            // but we can rewrite later
            if let Index::Num(num, _) = mod_idx {
                if let CoreModuleKind::Inline { fields } = &mods[num as usize].kind {
                    for instr in &*expression.instrs {
                        if let Instruction::Call(f_idx) = instr {
                            if debug {
                                eprintln!("FUNCTION CALL IN CABI REALLOC, IDX: {:?}", f_idx);
                            }

                            // find the function reference, which will be part of the current module
                            let new_func = idx_to_func(*f_idx, fields);
                            if new_func.is_none() {
                                // Most likely an import
                                // For now, we can just decide to skip adding it
                                // We would do this anyway bc once it calls out to an import it's leaving the instance anyway
                                if debug {
                                    eprintln!("DIDNT FIND FUNCTION IDX IN MODULE");
                                }
                                continue;
                            } else if debug {
                                eprintln!(
                                    "IN CABI REALLOC FUNCTION WAS FOUND, NEW FUNC ID: {:?}",
                                    new_func.unwrap().id
                                )
                            }
                            let new_func = new_func.unwrap();
                            queue.push((mod_idx, new_func));
                        }
                    }
                } else {
                    todo!()
                }
            } else {
                eprintln!("Module index: ${:?}", mod_idx);
                todo!()
            }

            blacklist.push((mod_idx, func));
            eprintln!(
                "Finished blacklisting func id: {:?}, name: {:?}",
                func.id, func.name
            );
        }
    }

    Ok(blacklist)
}

fn map_idx_to_module<'a, 'b: 'a>(
    wat: &'a Wat,
    blacklist: Vec<Index<'a>>,
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
                eprintln!("TODO: parse core func");
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

pub fn bump_type_idxs(
    wat: &Wat,
    output: &mut String,
    total_increment: &mut OffsetTracker,
) -> parser::Result<()> {
    // Things to bump
    // canon resource.drop
    // type( func (result $ty))
    // func (type $ty)
    // i think thats it

    // TODO compute this instead of hardcoding
    let LOWER_BOUND: u32 = 18;
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
                    total_increment.increment_idx(output, rd.ty, Some(LOWER_BOUND));
                }
            }
            // make this a function in this scope ig
            ComponentField::Type(t) => {
                let idxs = idxs_in_type(&t);
                for idx in idxs {
                    total_increment.increment_idx(output, idx, Some(LOWER_BOUND));
                }
            }
            ComponentField::Func(f) => {
                if let FuncKind::Lift { ty, .. } = &f.kind {
                    if let ComponentTypeUse::Ref(r) = ty {
                        total_increment.increment_idx(output, r.idx, Some(LOWER_BOUND));
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

pub fn add_instantiaion_arg(
    wat: &Wat,
    output: &mut String,
    total_increment: &mut OffsetTracker,
) -> parser::Result<()> {
    for field in get_fields(&wat).ok_or(Error::new(
        wat.span(),
        "Input WAT file could not be parsed (may be binary or module)".to_string(),
    ))? {
        match field {
            ComponentField::CoreInstance(ci) => match &ci.kind {
                CoreInstanceKind::Instantiate { .. } => {
                    eprintln!("Core instance: {ci:?}, offset: {}", ci.span.offset());
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
