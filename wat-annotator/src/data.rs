use std::path;

use serde::{Deserialize, Serialize};

use crate::debug::WatLineMapper;

#[derive(Serialize, Deserialize)]
pub struct DebugData {
    pub file_map: Vec<path::PathBuf>,
    pub blocks_per_line: Vec<(u64, u64)>, // stores the number of blocks at a specific line of code, needed for knowing if some blocks on a line weren't covered
}
