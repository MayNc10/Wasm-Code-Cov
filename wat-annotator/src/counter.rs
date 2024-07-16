use std::mem;
use std::ops::Range;

use regex::Regex;
use wasmparser::{Chunk, ComponentInstance, Parser, Payload};
use wast::core::{ModuleField, ModuleKind};
use wast::parser::ParseBuffer;
use wast::token::{Index, Span};
use wast::{component::*, Wat};
use wast::{parser, Error};

const COUNTER_REGEX_STR: &str = r"(?m)^\s*(loop|if|else|block)";
const MODULE_REGEX_STR: &str = r"\(core module";
const INSTANTIATION_REGEX_STR: &str = r"core instance \(;[0-9]+;\) \(instantiate [0-9]+";
const MODULE_NAME: &str = "counter-warm-code-cov";
const INSTANCE_NAME: &str = "counter-warm-code-cov-instance";
const PAGE_SIZE: usize = 64 * (2 << 10); // 64 KB
const BUFFER_NAME: &str = "counter-buffer";
const NUM_COUNTERS_NAME: &str = "num-counters";
const INC_FUNC_NAME: &str = "increment-counter";
const GET_FUNC_NAME: &str = "get-counter";

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
        } else if let Some(Index::Num(num, span)) = item.get_idx_if_avaliable() {
            let old = num.to_string();
            for _ in 0..old.as_bytes().len() {
                output.remove(span.offset() + byte_shift);
            }
            let new = (num + 1).to_string();
            output.insert_str(span.offset() + byte_shift, &new);
            byte_shift += new.as_bytes().len() - old.as_bytes().len();
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

// code that doesn't work
/*
// parse file to skip modules
    let parser = Parser::new(0);
    let mut parse_iter = parser.parse_all(output.as_bytes());
    // parse modules
    let mut module_byte_ranges = Vec::new();
    let mut instantiations_and_byte_ranges = Vec::new();
    while let Some(Ok(payload)) = parse_iter.next() {
        match payload {
            Payload::ModuleSection {
                parser: _,
                unchecked_range,
            } => {
                module_byte_ranges.push(unchecked_range);
            }
            Payload::ComponentInstanceSection(r) => {
                let mut iter = r.into_iter_with_offsets();
                while let Some(Ok((offset, instantiation))) = iter.next() {
                    if let ComponentInstance::Instantiate { .. } = instantiation {
                        instantiations_and_byte_ranges.push((offset, instantiation));
                    }
                }
            }
            _ => {
                panic!("hi");
                println!("Payload: {:?}", payload);
            }
        }
    }
    drop(parse_iter);

*/
