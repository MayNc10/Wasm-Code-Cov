use std::{
    collections::HashMap,
    fmt::{write, Display},
    fs,
    path::PathBuf,
};

use wat_annotator::data::DebugData;

pub type LineIndex = u64;
pub type ColumnIndex = u64;

pub enum Line {
    Empty,
    Singlet((ColumnIndex, u64)),
    Plural(HashMap<ColumnIndex, u64>),
}

impl Line {
    pub fn empty() -> Line {
        Line::Empty
    }
    pub fn new(idx: ColumnIndex) -> Line {
        Line::Singlet((idx, 1))
    }
    pub fn increment(&mut self, idx: ColumnIndex) {
        match self {
            Line::Plural(map) => drop(map.entry(idx).and_modify(|e| *e += 1).or_insert(1)),
            Line::Singlet((self_idx, counter)) => {
                if *self_idx == idx {
                    *counter += 1;
                } else {
                    *self = Line::Plural(HashMap::from([(*self_idx, *counter), (idx, 1)]))
                }
            }
            Line::Empty => *self = Line::Singlet((idx, 1)),
        }
    }
}

pub struct GCovFile {
    src_file: PathBuf,
    counters: HashMap<LineIndex, (Line, u64)>,
}

impl GCovFile {
    pub fn new(src_file: PathBuf, data: &DebugData) -> GCovFile {
        let counters = data
            .blocks_per_line
            .iter()
            .map(|(idx, count)| (*idx, (Line::empty(), *count)))
            .collect();
        GCovFile { src_file, counters }
    }
    pub fn increment(&mut self, line_idx: LineIndex, column_idx: ColumnIndex) {
        self.counters
            .get_mut(&line_idx)
            .unwrap()
            .0
            .increment(column_idx)
    }
}

// Will allow us to write into an output file
impl Display for GCovFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = fs::read_to_string(&self.src_file).map_err(|_| std::fmt::Error)?;
        for (idx, str_line) in s.split('\n').enumerate() {
            if let Some((line, num_counters)) = self.counters.get(&(idx as u64)) {
                match line {
                    Line::Singlet((_, count)) => {
                        let star = if *num_counters > 1 { "*" } else { "" };
                        writeln!(f, "{}{}: {}:{}", count, star, idx, str_line)?;
                    }
                    Line::Plural(map) => {
                        let count: u64 = map.values().sum();
                        let star = if *num_counters as usize > map.keys().count() {
                            "*"
                        } else {
                            ""
                        };
                        writeln!(f, "{}{}: {}:{}", count, star, idx, str_line)?;
                    }
                    Line::Empty => {
                        writeln!(f, "-: {}:{}", idx, str_line)?;
                    }
                }
            } else {
                writeln!(f, "-: {}:{}", idx, str_line)?;
            }
        }
        Ok(())
    }
}
