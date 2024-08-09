use std::{collections::HashMap, path, sync::Arc};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
/// The struct contains debugging data that should be passed along to other programs
pub struct DebugData {
    /// Maps indices to file paths
    pub file_map: Vec<path::PathBuf>,
    /// Contains the number of blocks in a specific line of code
    pub blocks_per_line: HashMap<usize, Vec<(u64, u64)>>, // maps file indxs to lines and number of counters
}

/// Like `DebugData`, but with an `Arc` wrapper around the paths to prevent excessive cloning
pub struct DebugDataArc {
    /// Maps indices to file paths
    pub file_map: Vec<Arc<path::PathBuf>>,
    /// Contains the number of blocks in a specific line of code
    pub blocks_per_line: HashMap<usize, Vec<(u64, u64)>>, // maps file indxs to lines and number of counters
}

impl From<DebugData> for DebugDataArc {
    fn from(value: DebugData) -> Self {
        DebugDataArc {
            file_map: value.file_map.into_iter().map(|p| Arc::new(p)).collect(),
            blocks_per_line: value.blocks_per_line,
        }
    }
}
