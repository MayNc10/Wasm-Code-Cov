use core::str;
use regex::Regex;
use wast::token::Index;

// Makes it more readable?
type Location = usize;
type Offset = usize;

/// A struct for tracking modifications to a file to map offsets between the original and modified versions
pub struct OffsetTracker {
    offsets: Vec<(Location, Offset)>,
}

impl OffsetTracker {
    /// create a new `OffsetTracker`
    pub fn new() -> OffsetTracker {
        OffsetTracker {
            offsets: Vec::new(),
        }
    }

    fn get_real_loc(&self, start: Location) -> Location {
        start
            + self
                .offsets
                .iter()
                .filter(|(loc, _)| *loc <= start)
                .map(|(_, offset)| *offset)
                .sum::<usize>()
    }

    // Should somehow link this struct to specific string
    // TODO!
    /// Insert a string `msg` into another string `s` at position `original_loc`
    pub fn add_to_string(&mut self, s: &mut String, original_loc: Location, msg: &str) {
        let loc = self.get_real_loc(original_loc);
        s.insert_str(loc, msg);
        self.offsets.push((original_loc, msg.as_bytes().len()));
    }

    // I feel kinda gross putting very implementation specific code here, but like
    // idk
    /// Increment a specific `Index`, with a lower bound to control whether the index should be increased or not
    pub fn increment_idx(&mut self, output: &mut String, idx: Index, lower_bound: Option<u32>) {
        match idx {
            Index::Num(num, _) => {
                if num >= lower_bound.unwrap_or(0) {
                    let loc = self.get_real_loc(idx.span().offset());

                    let old = num.to_string();
                    for _ in 0..old.as_bytes().len() {
                        output.remove(loc);
                    }
                    let new = (num + 1).to_string();
                    output.insert_str(loc, &new);
                    let tup = (
                        idx.span().offset(),
                        new.as_bytes().len() - old.as_bytes().len(),
                    );
                    self.offsets.push(tup);
                }
            }
            Index::Id(_) => {}
        }
    }

    /// Match a regex against a modified string, and use the regex information to change the string
    /// The provided function should take in the string, the start of the match, and the end of the match, and should return a (location, offset) tuple telling the tracker how the string was modified
    pub fn modify_with_regex_match<F>(
        &mut self,
        output: &mut String,
        re: &Regex,
        loc: Location,
        f: F,
    ) where
        F: FnOnce(&mut String, Location, Location) -> (Location, Offset),
    {
        let loc = self.get_real_loc(loc);
        let slice = str::from_utf8_mut(unsafe { output[loc..].as_bytes_mut() }).unwrap();
        let m = re.find(slice);
        if let Some(m) = m {
            let (start, end) = (m.start(), m.end());
            let pair = f(output, loc + start, loc + end);
            self.offsets.push(pair);
        }
    }

    /// Slice the string `output` starting at `start`
    pub fn get_slice_from<'a>(&self, output: &'a String, start: Location) -> &'a str {
        let loc = start
            + self
                .offsets
                .iter()
                .filter(|(loc, _)| *loc <= start)
                .map(|(_, offset)| *offset)
                .sum::<usize>();
        str::from_utf8(&output.as_bytes()[loc..]).unwrap()
    }
}
