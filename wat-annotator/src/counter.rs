use std::mem;
use std::ops::Range;

use regex::Regex;
use wasmparser::{Chunk, ComponentInstance, Parser, Payload};
use wast::core::{Instruction, ModuleField, ModuleKind};
use wast::kw::exnref;
use wast::parser::{parse, ParseBuffer};
use wast::token::{Index, Span};
use wast::{component::*, Wat};
use wast::{parser, Error};

const COUNTER_REGEX_STR: &str = r"(?m)^\s*(loop|if|else|block)";
const MODULE_REGEX_STR: &str = r"\(core module";
const INSTANTIATION_REGEX_STR: &str = r"core instance \(;[0-9]+;\) \(instantiate [0-9]+";
const INDEXNUM_REGEX_STR: &str = r"\(;(?P<idx>[0-9]+);\)";
const INDEXNUM_REPLACE_REGEX_STR: &str = r"$$$idx";
const MODULE_NAME: &str = "counter-warm-code-cov";
const INSTANCE_NAME: &str = "counter-warm-code-cov-instance";
const PAGE_SIZE: usize = 64 * (2 << 10); // 64 KB
const BUFFER_NAME: &str = "counter-buffer";
const NUM_COUNTERS_NAME: &str = "num-counters";
const GET_FUNC_NAME: &str = "get-counter";
const INC_FUNC_NAME: &str = "inc-counter";
/// FIXME: This should be failiable instead of lying for start and producers
fn get_span(f: &ComponentField) -> Span {
    match f {
        ComponentField::CoreModule(cm) => cm.span,
        ComponentField::CoreInstance(ci) => ci.span,
        ComponentField::CoreType(ct) => ct.span,
        ComponentField::Component(nc) => nc.span,
        ComponentField::Instance(i) => i.span,
        ComponentField::Alias(a) => a.span,
        ComponentField::Type(t) => t.span,
        ComponentField::CanonicalFunc(cf) => cf.span,
        ComponentField::CoreFunc(cf) => cf.span,
        ComponentField::Func(f) => f.span,
        ComponentField::Start(_s) => Span::from_offset(0),
        ComponentField::Import(ci) => ci.span,
        ComponentField::Export(ce) => ce.span,
        ComponentField::Custom(c) => c.span,
        ComponentField::Producers(_p) => Span::from_offset(usize::max_value()),
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
        ModuleField::Start(f) => None,
        ModuleField::Elem(f) => Some(f.span),
        ModuleField::Data(f) => Some(f.span),
        ModuleField::Tag(f) => Some(f.span),
        ModuleField::Custom(f) => None,
    }
}

// pulling this out to make it clearer
// using tabs for now, maybe switch asp
pub fn create_counter_module(num_counters: usize) -> String {
    let size_of = mem::size_of::<i64>();
    let buffer_size = num_counters * size_of;
    let mut code = String::new();
    let num_pages = buffer_size / PAGE_SIZE + 1;

    // module declare
    code.push_str("(core module $");
    code.push_str(MODULE_NAME);
    code.push('\n');
    // allocate buffer
    code.push_str(" (memory $");
    code.push_str(BUFFER_NAME);
    code.push_str(format!(" {})", num_pages).as_str());
    code.push('\n');
    // store static size of buffer
    code.push_str(" (global $");
    code.push_str(NUM_COUNTERS_NAME);
    code.push_str(" i64 ");
    code.push_str(format!("(i64.const {}))", num_counters).as_str());
    code.push('\n');
    // define function for incrementing counter
    code.push_str(" (func $");
    code.push_str(INC_FUNC_NAME);
    code.push_str(format!(" (param $idx i32) local.get $idx i32.const {size_of} i32.mul local.get $idx i32.const {size_of} i32.mul i64.load i64.const 1 i64.add i64.store)").as_str());
    code.push('\n');
    // define function for getting index
    code.push_str(" (func $");
    code.push_str(GET_FUNC_NAME);
    code.push_str(
        format!(
            " (param $idx i32) (result i64) local.get $idx i32.const {size_of} i32.mul i64.load)"
        )
        .as_str(),
    );
    code.push('\n');
    // export functions
    code.push_str(format!(" (export \"{0}\" (func ${0}))\n", INC_FUNC_NAME).as_str());
    code.push_str(format!(" (export \"{0}\" (func ${0}))\n", GET_FUNC_NAME).as_str());
    // export size
    code.push_str(format!(" (export \"{0}\" (global ${0}))", NUM_COUNTERS_NAME).as_str());
    // end module
    code.push_str(")\n");
    code
}

pub fn create_module_instance() -> String {
    format!(
        "   (core instance ${} (instantiate ${}))\n",
        INSTANCE_NAME, MODULE_NAME
    )
}

pub fn create_instance_import() -> String {
    format!(
        "   (with \"{}\" (instance ${}))\n",
        MODULE_NAME, INSTANCE_NAME
    )
}

pub fn create_import_statements() -> String {
    let mut code = String::new();
    code.push_str(
        format!(
            " (import \"{}\" \"{1}\" (func ${1} (param i32)))\n",
            MODULE_NAME, INC_FUNC_NAME
        )
        .as_str(),
    );
    code.push_str(
        format!(
            " (import \"{}\" \"{1}\" (func ${1} (param i32) (result i64)))\n",
            MODULE_NAME, GET_FUNC_NAME
        )
        .as_str(),
    );
    code
}

#[derive(Clone, Copy)]
enum ItemType<'a> {
    InstanceIndex(Index<'a>),
    Alias(Index<'a>),
    InstanceSpan(Span),
}

impl<'a> ItemType<'a> {
    fn span(&self) -> Span {
        match self {
            Self::Alias(idx) | Self::InstanceIndex(idx) => idx.span(),
            Self::InstanceSpan(span) => *span,
        }
    }
    /// Some enum variants don't have an index, in which case they return None
    fn get_idx_if_avaliable(&self) -> Option<Index> {
        match self {
            Self::Alias(idx) | Self::InstanceIndex(idx) => Some(*idx),
            Self::InstanceSpan(_) => None,
        }
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

pub fn increment_idx(
    output: &mut String,
    idx: Index,
    byte_shift: usize,
    lower_bound: Option<u32>,
) -> usize {
    match idx {
        Index::Num(num, span) => {
            if num >= lower_bound.unwrap_or(0) {
                let old = num.to_string();
                for _ in 0..old.as_bytes().len() {
                    output.remove(span.offset() + byte_shift);
                }
                let new = (num + 1).to_string();
                output.insert_str(span.offset() + byte_shift, &new);
                byte_shift + new.as_bytes().len() - old.as_bytes().len()
            } else {
                0
            }
        }
        Index::Id(_) => 0,
    }
}

pub fn get_fields<'a>(comp: &'a Wat) -> Option<&'a Vec<ComponentField<'a>>> {
    match comp {
        Wat::Module(_) => None,
        Wat::Component(comp) => match &comp.kind {
            ComponentKind::Binary(_) => None,
            ComponentKind::Text(v) => Some(v),
        },
    }
}

pub fn add_scaffolding(mut wat: String) -> parser::Result<String> {
    // Things to do: (in order)
    // Add import statement for the inc counter function (in type and import section)
    // Add import statements within each module
    // Add function calls wherever we want
    // Bump the index of any instance reference
    // Bump the index of any component function
    // Bump the index of any core function
    // Add canon lower of inc counter func (right after module sections)
    // Add instance exporting that core function
    wat = add_inc_import_section(wat)?;
    wat = add_imports_in_module(wat)?;
    wat = add_func_calls(wat)?;
    wat = bump_instance_idxs(wat)?;
    wat = bump_comp_func_idxs(wat)?;
    wat = bump_core_func_idxs(wat)?;
    wat = bump_type_idxs(wat)?;
    wat = add_canon_lower_and_instance(wat)?;
    Ok(wat)
}

pub fn add_inc_import_section(wat: String) -> parser::Result<String> {
    let mut output = wat.clone();
    let buf = ParseBuffer::new(&wat)?;
    let wat = parse::<Wat>(&buf)?;
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
                if was_last_ty_import_alias {
                    offset = get_span(field).offset();
                    break 'field;
                }
            }
        }
    }

    let msg = format!(
        "(import \"{0}\" (func ${0} (param \"idx\" s32)))",
        INC_FUNC_NAME
    );
    output.insert_str(offset, &msg);

    Ok(output)
}

pub fn add_imports_in_module(wat: String) -> parser::Result<String> {
    let mut output = wat.clone();
    let buf = ParseBuffer::new(&wat)?;
    let wat = parse::<Wat>(&buf)?;
    let mut offset = 0;
    let mut total_increment = 0;
    for field in get_fields(&wat).ok_or(Error::new(
        wat.span(),
        "Input WAT file could not be parsed (may be binary or module)".to_string(),
    ))? {
        if let ComponentField::CoreModule(m) = field {
            // parse module fields
            if let CoreModuleKind::Inline { fields } = &m.kind {
                let mut was_last_import = false;
                'module: for field in fields {
                    if let ModuleField::Import(_) = field {
                        was_last_import = true
                    } else if was_last_import {
                        if let Some(span) = get_module_span(field) {
                            offset = span.offset();
                            break 'module;
                        }
                    }
                }
            }
            // FIXME: Account for modules that end after imports (somehow)
            if offset == 0 {
                return Err(Error::new(
                    m.span,
                    "Module had no fields after imports".to_string(),
                ));
            }
            let msg = format!(
                "(import \"{0}\" (func ${0} (param \"idx\" i32)))",
                INC_FUNC_NAME
            );
            output.insert_str(offset + total_increment, &msg);
            total_increment += msg.as_bytes().len();
        }
    }

    Ok(output)
}

// todo actually add function calls lol
pub fn add_func_calls(wat: String) -> parser::Result<String> {
    let mut output = wat.clone();
    let mut buf = ParseBuffer::new(&wat)?;
    let buf = buf.track_instr_spans(true);
    let wat = parse::<Wat>(buf)?;
    let mut total_increment = 0;
    for field in get_fields(&wat).ok_or(Error::new(
        wat.span(),
        "Input WAT file could not be parsed (may be binary or module)".to_string(),
    ))? {
        if let ComponentField::CoreModule(m) = field {
            // parse module fields
            if let CoreModuleKind::Inline { fields } = &m.kind {
                for field in fields {
                    if let ModuleField::Func(func) = field {
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
                                        let msg = format!(";; hai :3");
                                        output.insert_str(
                                            spans[idx].offset() + total_increment,
                                            &msg,
                                        );
                                        total_increment += msg.as_bytes().len();
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

    Ok(output)
}

pub fn bump_instance_idxs(wat: String) -> parser::Result<String> {
    // instance idxs are used in:
    // alias export statements
    // intantiation args

    let mut output = wat.clone();
    let mut buf = ParseBuffer::new(&wat)?;
    let wat = parse::<Wat>(&buf)?;
    let mut total_increment = 0;
    for field in get_fields(&wat).ok_or(Error::new(
        wat.span(),
        "Input WAT file could not be parsed (may be binary or module)".to_string(),
    ))? {
        match field {
            ComponentField::Alias(a) => match a.target {
                AliasTarget::Export { instance: idx, .. }
                | AliasTarget::CoreExport { instance: idx, .. } => {
                    // TODO Figure out lower bound flexibly
                    total_increment += increment_idx(&mut output, idx, total_increment, None);
                }
                _ => {}
            },
            ComponentField::Instance(i) => {
                if let InstanceKind::Instantiate { component: _, args } = &i.kind {
                    for arg in args {
                        if let InstantiationArgKind::Item(cek) = &arg.kind {
                            // Other things here need to be bumped, but that happens in other functions
                            if let ComponentExportKind::Instance(i) = cek {
                                // TODO Figure out lower bound flexibly
                                total_increment +=
                                    increment_idx(&mut output, i.idx, total_increment, None);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok(output)
}

pub fn bump_comp_func_idxs(wat: String) -> parser::Result<String> {
    todo!()
}

pub fn bump_core_func_idxs(wat: String) -> parser::Result<String> {
    todo!()
}

pub fn bump_type_idxs(wat: String) -> parser::Result<String> {
    // Things to bump
    // canon resource.drop
    // type( func (result $ty))
    // func (type $ty)
    // i think thats it

    // TODO compute this instead of hardcoding
    let LOWER_BOUND: u32 = 18;
    let mut output = wat.clone();
    let mut buf = ParseBuffer::new(&wat)?;
    let wat = parse::<Wat>(&buf)?;
    let mut total_increment = 0;
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
                            ComponentTypeDecl::CoreType(ct) => {
                                todo!()
                            }
                            ComponentTypeDecl::Type(ty) => idxs.append(&mut idxs_in_type(ty)),
                            ComponentTypeDecl::Alias(_) => {
                                todo!()
                            }
                            ComponentTypeDecl::Import(i) => {
                                todo!()
                            }
                            ComponentTypeDecl::Export(e) => {
                                todo!()
                            }
                        }
                    }
                }
                TypeDef::Instance(i) => {
                    todo!()
                }
                TypeDef::Resource(r) => {
                    todo!()
                }
            }
            idxs
        }

        match field {
            ComponentField::CoreFunc(f) => {
                if let CoreFuncKind::ResourceDrop(rd) = &f.kind {
                    total_increment +=
                        increment_idx(&mut output, rd.ty, total_increment, Some(LOWER_BOUND));
                }
            }
            // make this a function in this scope ig
            ComponentField::Type(t) => {
                let idxs = idxs_in_type(&t);
                for idx in idxs {
                    total_increment +=
                        increment_idx(&mut output, idx, total_increment, Some(LOWER_BOUND));
                }
            }
            ComponentField::Func(f) => {
                if let FuncKind::Lift { ty, .. } = &f.kind {
                    if let ComponentTypeUse::Ref(r) = ty {
                        total_increment +=
                            increment_idx(&mut output, r.idx, total_increment, Some(LOWER_BOUND));
                    }
                }
            }
            _ => {}
        }
    }

    Ok(output)
}

pub fn add_canon_lower_and_instance(wat: String) -> parser::Result<String> {
    todo!()
}

pub fn insert_counters<'a>(wat: String) -> parser::Result<String> {
    let mut output = wat.clone();

    // TODO: Rewrite this using the wast system instead of regexes
    // find all matches
    let counter_re = Regex::new(COUNTER_REGEX_STR).unwrap();
    let matches = counter_re.find_iter(&wat);
    // insert comments
    let mut counter_num = 0;
    {
        let mut offset = 0;
        for m in matches {
            let msg = format!(";; inc counter #{}\n", counter_num); //format!("i32.const {} call ${}\n", counter_num, INC_FUNC_NAME);
            output.insert_str(m.start() + offset, &msg);
            offset += msg.as_bytes().len();
            counter_num += 1;
        }
    }
    let output_dup = output.clone();
    let buf = ParseBuffer::new(&output_dup)?;
    let component = parser::parse::<Wat>(&buf)?;
    // Insert import statement
    let mut last_import_span = None;
    let mut last_range = 0..1;
    let mut byte_shift = 0;
    for field in get_fields(&component).unwrap() {
        if let ComponentField::Import(i) = field {
            last_import_span = Some(i);
        } else if let ComponentField::Type(_) = field {
        } else if let ComponentField::Alias(_) = field {
        } else if last_import_span.is_some() {
            last_range = last_import_span.unwrap().span.offset()..get_span(field).offset();
            last_import_span = None;
        } else if let ComponentField::CoreFunc(cf) = field {
            if let CoreFuncKind::ResourceDrop(rd) = &cf.kind {
                // todo: don't hardcode this lol
                byte_shift += increment_idx(&mut output, rd.ty, byte_shift, Some(18));
            }
        }
    }
    // insert test statement
    output.insert_str(
        last_range.end - 1,
        "(import \"inc-counter\" (func $inc-count (param \"idx\" s32))) \n",
    );

    let buf = ParseBuffer::new(&output).unwrap();
    let component = parser::parse::<Wat>(&buf).unwrap();
    Ok(output)
}
/*
pub fn insert_counters<'a>(wat: String) -> parser::Result<String> {
    let mut output = wat.clone();

    // TODO: Rewrite this using the wast system instead of regexes
    // find all matches
    let counter_re = Regex::new(COUNTER_REGEX_STR).unwrap();
    let matches = counter_re.find_iter(&wat);
    // insert comments
    let mut counter_num = 0;
    {
        let mut offset = 0;
        for m in matches {
            let msg = format!("i32.const {} call ${}\n", counter_num, INC_FUNC_NAME); //format!(";; inc counter #{}\n", counter_num);
            output.insert_str(m.start() + offset, &msg);
            offset += msg.as_bytes().len();
            counter_num += 1;
        }
    }

    // use wast to find modules
    let output_2 = output.clone();
    let buf = ParseBuffer::new(&output_2).unwrap();
    let wat = parser::parse::<Wat>(&buf)?;
    let component = match wat {
        Wat::Component(c) => c,
        Wat::Module(_) => {
            return parser::Result::Err(Error::new(
                Span::from_offset(0),
                "Expected a component, got a module".to_string(),
            ))
        }
    };
    let component_fields = match component.kind {
        ComponentKind::Text(v) => v,
        ComponentKind::Binary(_) => {
            return parser::Result::Err(Error::new(
                Span::from_offset(0),
                "Component was in binary form".to_string(),
            ))
        }
    };

    // Find module offsets
    let mut module_byte_ranges: Vec<Range<usize>> = Vec::new();
    let mut last_import_byte_ranges = Vec::new();
    let mut instantiation_idxs: Vec<Index> = Vec::new();
    let mut aliases = Vec::new();
    let mut instatiation_ranges = Vec::new();
    let mut was_last_module = false; // To get the end of spans

    for field in component_fields {
        if was_last_module {
            let span = get_span(&field);
            let start = module_byte_ranges.last().unwrap().start;
            *module_byte_ranges.last_mut().unwrap() = start..span.offset();
            was_last_module = false;
        }

        match field {
            ComponentField::CoreModule(m) => {
                // I don't think we want core module imports for now
                match m.kind {
                    CoreModuleKind::Inline {
                        fields: module_fields,
                    } => {
                        module_byte_ranges.push(m.span.offset()..(m.span.offset() + 1));
                        was_last_module = true;
                        let mut last_import: Option<Span> = None;

                        for field in module_fields {
                            // Fix this to check for func or end of module
                            if last_import.is_some()
                                && !matches!(field, ModuleField::Import(..))
                                && get_module_span(&field).is_some()
                            {
                                let start = last_import.unwrap().offset();
                                last_import_byte_ranges
                                    .push(start..get_module_span(&field).unwrap().offset());
                                last_import = None;
                            }

                            if let ModuleField::Import(i) = field {
                                last_import = Some(i.span);
                            }
                        }
                    }
                    CoreModuleKind::Import { .. } => {}
                }
            }
            ComponentField::CoreInstance(ci) => {
                if let CoreInstanceKind::Instantiate { module, args } = ci.kind {
                    instatiation_ranges.push(ci.span);
                    for arg in args {
                        if let CoreInstantiationArgKind::Instance(i_ref) = arg.kind {
                            instantiation_idxs.push(i_ref.idx)
                        }
                    }
                }
            }
            ComponentField::Alias(a) => match a.target {
                AliasTarget::CoreExport { instance, .. } => {
                    aliases.push(instance);
                }
                _ => {}
            },
            _ => {}
        }
    }

    // bump indexes
    let mut items: Vec<_> = instantiation_idxs
        .into_iter()
        .map(|idx| ItemType::InstanceIndex(idx))
        .collect();
    items.extend(aliases.iter().map(|&idx| ItemType::Alias(idx)));
    items.extend(
        instatiation_ranges
            .iter()
            .map(|&span| ItemType::InstanceSpan(span)),
    );
    items.sort_by(|ii1, ii2| ii1.span().cmp(&ii2.span()));
    let mut byte_shift = 0;

    let re_instance = Regex::new(INSTANTIATION_REGEX_STR).unwrap();

    for item in items {
        if let ItemType::InstanceSpan(s) = item {
            // find where to offset
            let m = re_instance
                .find(std::str::from_utf8(&output.as_bytes()[s.offset() + byte_shift..]).unwrap())
                .unwrap();

            let msg = format!("\n{}", create_instance_import());
            output.insert_str(s.offset() + byte_shift + m.end(), &msg);
            byte_shift += msg.as_bytes().len();
        } else if let Some(idx) = item.get_idx_if_avaliable() {
            byte_shift += increment_idx(&mut output, idx, byte_shift);
        } else {
            unreachable!()
        }
    }

    // insert module import statement
    /*
    let mut module_offset = 0;
    for range in &module_byte_ranges {
        // find where the next line starts
        let next_line =
            output[range.start + module_offset..].find('\n').unwrap() + "\n".as_bytes().len();
        let import_stmt = create_import_statements();
        for line in import_stmt.split('\n') {
            let line = format!("    {line}\n");
            output.insert_str(range.start + module_offset + next_line, &line);
            module_offset += line.as_bytes().len();
        }
    }
    */

    // Insert import at the end of the import blocks
    let mut import_offset = 0;
    for range in &last_import_byte_ranges {
        let import_stmt = create_import_statements();
        for line in import_stmt.split('\n') {
            let line = format!("    {line}\n");
            output.insert_str(range.end - 1 + import_offset, &line);
            import_offset += line.as_bytes().len();
        }
    }

    // Insert counter module at the end
    let counter_module = create_counter_module(counter_num);
    let mut byte_offset = module_byte_ranges.last().unwrap().end - 1 + import_offset;
    for line in counter_module.split('\n') {
        let line = format!("    {line}\n");
        output.insert_str(byte_offset, &line);
        byte_offset += line.as_bytes().len();
    }
    // insert instantiation
    let counter_instance = create_module_instance();
    for line in counter_instance.split('\n') {
        let line = format!("    {line}\n");
        output.insert_str(byte_offset, &line);
        byte_offset += line.as_bytes().len();
    }

    //let buf = ParseBuffer::new(&output)?;
    //let _module = parser::parse::<Wat>(&buf)?;
    Ok(output)
}
 */
