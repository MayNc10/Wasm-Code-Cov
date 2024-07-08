use std::mem;

use regex::Regex;
use wast::parser;
use wast::parser::ParseBuffer;
use wast::Wat;

const REGEX_STR: &str = r"(?m)^\s*(loop|if|else|block)"; // ^\s*loop
const MODULE_NAME: &str = "counter-warm-code-cov";
const PAGE_SIZE: usize = 64 * (2 << 10); // 64 KB
const BUFFER_NAME: &str = "counter-buffer";
const NUM_COUNTERS_NAME: &str = "num-counters";
const INC_FUNC_NAME: &str = "increment-counter";
const GET_FUNC_NAME: &str = "get-counter";

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
    code.push_str(format!(" (param $idx i32) (result i64) local.get $idx i32.const {size_of} i32.mul i64.load)").as_str());
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

pub fn insert_counters<'a>(wat: String) -> parser::Result<String> {
    let mut output = wat.clone();

    // find all matches
    let re = Regex::new(REGEX_STR).unwrap();
    let matches = re.find_iter(&wat);
    // insert comments
    let mut offset = 0;
    let mut counter_num = 0;
    for m in matches {
        let msg = format!(";; inc counter #{}\n", counter_num);
        output.insert_str(m.start() + offset, &msg);
        offset += msg.as_bytes().len();
        counter_num += 1;
    }
    // Insert counter module at the end
    let counter_module = create_counter_module(counter_num);
    output.truncate(output.len() - 2); // get rid of last paren
    //return Ok(output);
    // indent over all lines
    for line in counter_module.split('\n') {
        output.push_str(format!("   {}\n", line).as_str());
    }
    output.push_str(")\n");

    let buf = ParseBuffer::new(&output)?;
    let _module = parser::parse::<Wat>(&buf)?;
    Ok(output)
}
