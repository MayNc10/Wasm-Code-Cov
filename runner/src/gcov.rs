use std::collections::HashMap;

pub struct SrcLineMap {
    map: HashMap<u64, u64>,
}

impl SrcLineMap {
    pub fn new() -> SrcLineMap {
        SrcLineMap {
            map: HashMap::new(),
        }
    }
    pub fn add_to_line(&mut self, line: u64) {
        self.map.entry(line).and_modify(|e| *e += 1).or_insert(1);
    }
}
