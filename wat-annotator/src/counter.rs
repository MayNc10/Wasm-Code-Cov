use regex::Regex;
use wast::parser;
use wast::parser::ParseBuffer;
use wast::Wat;

const REGEX_STR: &str = r"(?m)^\s*(loop|if|else|block)"; // ^\s*loop

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
    let buf = ParseBuffer::new(&output)?;
    let _module = parser::parse::<Wat>(&buf)?;
    Ok(output)
}
