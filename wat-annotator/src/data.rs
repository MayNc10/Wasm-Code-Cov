use std::{
    collections::HashMap,
    ops::Deref,
    path::{self, Path},
    sync::Arc,
};

use serde::{Deserialize, Serialize};

use crate::debug::SourceDebugInfo;

/// A trait to consildate functions across the two types of DebugData structs
pub trait DebugData {
    /// Gets the file at a specific index in the file map, or returns `None` if the index is out of bounds
    fn file_map_idx(&self, idx: usize) -> Option<&dyn Deref<Target = path::Path>>;
    /// Returns a hashmap mapping file map indices to lists of lines and numbers of counters
    fn blocks_per_line(&self) -> &HashMap<usize, Vec<(u64, u64)>>;
    /// Access the vector of `SourceDebugInfo` structs in this debug data struct
    fn sdi_vec(&self) -> &Vec<SourceDebugInfo>;

    /// Display the counters on each line for every file in the map
    fn print_idxs_for_file(&self) {
        let map = self.blocks_per_line();
        for idx in 0.. {
            if let Some(path) = self.file_map_idx(idx) {
                println!("FileL {}", path.display());
                let counter_list = map.get(&idx).unwrap();
                for (line, count) in counter_list {
                    println!("\t@{}:#{}", line, count);
                }
            } else {
                break;
            }
        }
    }
    /// Get the SDI matching a file path, if one exists
    fn get_sdi_from_file(&self, file: &Path) -> Option<&SourceDebugInfo> {
        self.sdi_vec().iter().find(|sdi| {
            self.file_map_idx(sdi.path_idx)
                .is_some_and(|f| &**f == file)
        })
    }
}

#[derive(Serialize, Deserialize)]
/// The struct contains debugging data that should be passed along to other programs
pub struct DebugDataOwned {
    /// Maps indices to file paths
    pub file_map: Vec<path::PathBuf>,
    /// Contains the number of blocks in a specific line of code
    pub blocks_per_line: HashMap<usize, Vec<(u64, u64)>>, // maps file indxs to lines and number of counters
    /// A list of source debug information structs
    pub sdi_vec: Vec<SourceDebugInfo>, // A list of source debug information structs
}

impl DebugData for DebugDataOwned {
    fn file_map_idx(&self, idx: usize) -> Option<&dyn Deref<Target = path::Path>> {
        if idx < self.file_map.len() {
            Some(&self.file_map[idx])
        } else {
            None
        }
    }

    fn blocks_per_line(&self) -> &HashMap<usize, Vec<(u64, u64)>> {
        &self.blocks_per_line
    }

    fn sdi_vec(&self) -> &Vec<SourceDebugInfo> {
        &self.sdi_vec
    }
}

/// Like `DebugData`, but with an `Arc` wrapper around the paths to prevent excessive cloning
pub struct DebugDataArc {
    /// Maps indices to file paths
    pub file_map: Vec<Arc<path::PathBuf>>,
    /// Contains the number of blocks in a specific line of code
    pub blocks_per_line: HashMap<usize, Vec<(u64, u64)>>, // maps file indxs to lines and number of counters
    /// A list of source debug information structs
    pub sdi_vec: Vec<SourceDebugInfo>, // A list of source debug information structs
}

impl DebugData for DebugDataArc {
    fn file_map_idx(&self, idx: usize) -> Option<&dyn Deref<Target = path::Path>> {
        if idx < self.file_map.len() {
            Some(&*self.file_map[idx])
        } else {
            None
        }
    }

    fn blocks_per_line(&self) -> &HashMap<usize, Vec<(u64, u64)>> {
        &self.blocks_per_line
    }
    fn sdi_vec(&self) -> &Vec<SourceDebugInfo> {
        &self.sdi_vec
    }
}

impl From<DebugDataOwned> for DebugDataArc {
    fn from(value: DebugDataOwned) -> Self {
        DebugDataArc {
            file_map: value.file_map.into_iter().map(Arc::new).collect(),
            blocks_per_line: value.blocks_per_line,
            sdi_vec: value.sdi_vec,
        }
    }
}
