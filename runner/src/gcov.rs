//! This module provides support for emmiting .gcov files to be used with GCov visualizers

use std::{collections::HashMap, fmt::Display, fs, path::PathBuf, sync::Arc};

use wat_annotator::data::DebugDataArc;

/// A type alias for a u64, used to make what certain arguments are used for more obvious. This type is used for the line number in a source file.
pub type LineIndex = u64;
/// The same idea as before, but for column indices
pub type ColumnIndex = u64;

/// A line in a GCov program
/// This is an enum of a bunch of diffetent states, but it could (and will!) be rewritten to be simpler
pub enum Line {
    /// A Line with no counter blocks
    Empty,
    /// A line with one counter blocks
    Singlet((ColumnIndex, u64)),
    /// A line with many counter blocks
    Plural(HashMap<ColumnIndex, u64>),
}

impl Line {
    /// Create an empty line
    pub fn empty() -> Line {
        Line::Empty
    }
    /// Create a line with a specified column index as the first block
    pub fn new(idx: ColumnIndex) -> Line {
        Line::Singlet((idx, 1))
    }
    /// Increment the number of counters for the block at `idx`
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
    /// Get the total number of counters for all blocks in this line
    pub fn total_counters(&self) -> u64 {
        match self {
            Line::Empty => 0,
            Line::Singlet((_, count)) => *count,
            Line::Plural(map) => map.keys().sum(),
        }
    }
    /// Ge the number of unique blocks on this line
    pub fn num_blocks(&self) -> u64 {
        match self {
            Line::Empty => 0,
            Line::Singlet(_) => 1,
            Line::Plural(map) => map.keys().count() as u64,
        }
    }
}

/// A struct representing a `.gcov` file
pub struct GCovFile {
    src_file: Arc<PathBuf>,
    counters: HashMap<LineIndex, (Line, u64)>,
}

impl GCovFile {
    /// Create a new GCov file representing the source code in `src_file`
    pub fn new(src_file: Arc<PathBuf>, data: &DebugDataArc) -> GCovFile {
        let counters = data
            .blocks_per_line
            .iter()
            .map(|(idx, count)| (*idx, (Line::empty(), *count)))
            .collect();
        GCovFile { src_file, counters }
    }
    /// Increment a counter for the block at [`line_idx`]:[`column_idx`]
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
        let s = fs::read_to_string(self.src_file.as_path()).map_err(|_| std::fmt::Error)?;
        let info_lines = s
            .split('\n')
            .enumerate()
            .map(|(idx, str_line)| {
                if let Some((line, num_blocks)) = self.counters.get(&(idx as u64)) {
                    let block_diff = *num_blocks - line.num_blocks();
                    let total_counters = line.total_counters();
                    let star = if block_diff > 0 {
                        format!("*{}", block_diff)
                    } else {
                        "".to_string()
                    };
                    let count = if total_counters > 0 {
                        format!("{total_counters}")
                    } else {
                        "-".to_string()
                    };
                    (format!("{}{}:", count, star), (idx, str_line))
                } else {
                    ("-:".to_string(), (idx, str_line))
                }
            })
            .collect::<Vec<_>>();
        let max_len = info_lines.iter().map(|(s, _)| s.len()).max().unwrap();
        for (info, (idx, str)) in info_lines {
            let width = max_len - info.len();
            writeln!(f, "{:width$} {}:{}", info, idx, str)?;
        }
        Ok(())
    }
}
