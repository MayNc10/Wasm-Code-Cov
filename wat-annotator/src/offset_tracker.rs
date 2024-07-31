use core::str;

use regex::{Match, Regex};
use wast::token::Index;

// Makes it more readable?
type Location = usize;
type Offset = usize;

pub struct OffsetTracker {
    offsets: Vec<(Location, Offset)>,
}

// TODO
// Currently we implement things by counting by index
// and preserving the order of the offsets
// but if we used filter iterators like in the slice fn
// Then we dont have to preserve order
// change this!
impl OffsetTracker {
    pub fn new() -> OffsetTracker {
        OffsetTracker {
            offsets: Vec::new(),
        }
    }

    // TODO rename this
    fn find_break_idx(&self, original_loc: Location) -> Option<usize> {
        let mut break_idx = None;
        for idx in 0..self.offsets.len() {
            let (off_loc, _) = self.offsets[idx];
            if off_loc > original_loc {
                break_idx = Some(idx);
                break;
            }
        }

        break_idx
    }

    // Should somehow link this struct to specific string
    // TODO!
    pub fn add_to_string(&mut self, s: &mut String, original_loc: Location, msg: &str) {
        // figure out how much offset to add
        let idx = self.find_break_idx(original_loc);
        let end = match idx {
            Some(idx) => {
                self.offsets
                    .insert(idx, (original_loc, msg.as_bytes().len()));
                idx
            }
            None => {
                self.offsets.push((original_loc, msg.as_bytes().len()));
                self.offsets.len() - 1
            }
        };
        let loc = original_loc
            + self.offsets[..end]
                .iter()
                .map(|(_, offset)| offset)
                .fold(0, |acc, offset| acc + offset);
        s.insert_str(loc, msg);
    }

    // I feel kinda gross putting very implementation specific code here, but like
    // idk
    pub fn increment_idx(&mut self, output: &mut String, idx: Index, lower_bound: Option<u32>) {
        match idx {
            Index::Num(num, span) => {
                if num >= lower_bound.unwrap_or(0) {
                    let end = match self.find_break_idx(idx.span().offset()) {
                        Some(i) => i,
                        None => self.offsets.len(),
                    };
                    let loc = idx.span().offset()
                        + self.offsets[..end]
                            .iter()
                            .map(|(_, offset)| offset)
                            .fold(0, |acc, offset| acc + offset);

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
                    if end == self.offsets.len() {
                        self.offsets.push(tup);
                    } else {
                        self.offsets.insert(end, tup);
                    }
                }
            }
            Index::Id(_) => {}
        }
    }

    pub fn modify_with_regex_match<F>(
        &mut self,
        output: &mut String,
        re: &Regex,
        loc: Location,
        f: F,
    ) where
        F: FnOnce(&mut String, Location, Location) -> (Location, Offset),
    {
        let idx = self.find_break_idx(loc);
        let end = match idx {
            Some(idx) => idx,
            None => self.offsets.len(),
        };
        let loc = loc
            + self.offsets[..end]
                .iter()
                .map(|(_, offset)| offset)
                .fold(0, |acc, offset| acc + offset);
        let slice = str::from_utf8_mut(unsafe { output[loc..].as_bytes_mut() }).unwrap();
        let m = re.find(slice);
        if let Some(m) = m {
            let (start, end) = (m.start(), m.end());
            let pair = f(output, loc + start, loc + end);
            if idx.is_some() {
                self.offsets.insert(idx.unwrap(), pair);
            } else {
                self.offsets.push(pair);
            }
        }
    }

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
