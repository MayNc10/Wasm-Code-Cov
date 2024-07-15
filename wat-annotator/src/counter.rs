use std::mem;
use std::ops::Range;

use regex::Regex;
use wasmparser::{Chunk, ComponentInstance, Parser, Payload};
use wast::core::ModuleKind;
use wast::parser::ParseBuffer;
use wast::token::{Index, Span};
use wast::{component::*, Wat};
use wast::{parser, Error};

const COUNTER_REGEX_STR: &str = r"(?m)^\s*(loop|if|else|block)";
const MODULE_REGEX_STR: &str = r"\(core module";
const MODULE_NAME: &str = "counter-warm-code-cov";
const INSTANCE_NAME: &str = "counter-warm-code-cov-instance";
const PAGE_SIZE: usize = 64 * (2 << 10); // 64 KB
const BUFFER_NAME: &str = "counter-buffer";
const NUM_COUNTERS_NAME: &str = "num-counters";
const INC_FUNC_NAME: &str = "increment-counter";
const GET_FUNC_NAME: &str = "get-counter";

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
        ComponentField::Start(s) => Span::from_offset(0),
        ComponentField::Import(ci) => ci.span,
        ComponentField::Export(ce) => ce.span,
        ComponentField::Custom(c) => c.span,
        ComponentField::Producers(p) => Span::from_offset(usize::max_value()),
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

#[derive(Clone, Copy)]
enum ItemType {
    Instance,
    Alias,
}

struct ItemIdx<'a> {
    pub idx: Index<'a>,
    pub item_type: ItemType,
}

impl<'a> ItemIdx<'a> {
    fn from_idxs(idxs: &[Index<'a>], ty: ItemType) -> Vec<ItemIdx<'a>> {
        idxs.iter()
            .map(|&idx| ItemIdx { idx, item_type: ty })
            .collect()
    }
}

pub fn insert_counters<'a>(wat: String) -> parser::Result<String> {
    let mut output = wat.clone();

    // find all matches
    let counter_re = Regex::new(COUNTER_REGEX_STR).unwrap();
    let matches = counter_re.find_iter(&wat);
    // insert comments
    let mut counter_num = 0;
    {
        let mut offset = 0;
        for m in matches {
            let msg = format!(";; inc counter #{}\n", counter_num);
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
    let mut instantiations: Vec<Index> = Vec::new();
    let mut aliases = Vec::new();
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
                    CoreModuleKind::Inline { .. } => {
                        module_byte_ranges.push(m.span.offset()..(m.span.offset() + 1));
                        was_last_module = true;
                    }
                    CoreModuleKind::Import { .. } => {}
                }
            }
            ComponentField::CoreInstance(ci) => {
                if let CoreInstanceKind::Instantiate { module, args } = ci.kind {
                    for arg in args {
                        if let CoreInstantiationArgKind::Instance(i_ref) = arg.kind {
                            instantiations.push((i_ref.idx))
                        }
                    }
                }
            }
            ComponentField::Instance(i) => {}
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
    let mut indexes = ItemIdx::from_idxs(&instantiations, ItemType::Instance);
    indexes.append(&mut ItemIdx::from_idxs(&aliases, ItemType::Alias));
    indexes.sort_by(|ii1, ii2| ii1.idx.span().cmp(&ii2.idx.span()));
    let mut byte_shift = 0;

    for item_idx in indexes {
        if let Index::Num(num, span) = item_idx.idx {
            let old = num.to_string();
            for _ in 0..old.as_bytes().len() {
                output.remove(span.offset() + byte_shift);
            }
            let new = (num + 1).to_string();
            output.insert_str(span.offset() + byte_shift, &new);
            byte_shift += new.as_bytes().len() - old.as_bytes().len();
        }
    }

    // Insert counter module at the end
    let counter_module = create_counter_module(counter_num);
    let mut byte_offset = module_byte_ranges.last().unwrap().end - 1;
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
