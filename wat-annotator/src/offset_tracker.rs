use wast::token::Index;

// Makes it more readable?
type Location = usize;
type Offset = usize;

pub struct OffsetTracker {
    offsets: Vec<(Location, Offset)>,
}

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
}
