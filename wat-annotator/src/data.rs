use std::{
    ops::{Deref, DerefMut},
    path,
    rc::Rc,
    sync::Arc,
};

use serde::{Deserialize, Serialize};

use crate::debug::WatLineMapper;

#[derive(Serialize, Deserialize)]
pub struct DebugData {
    pub file_map: Vec<path::PathBuf>,
    pub blocks_per_line: Vec<(u64, u64)>, // stores the number of blocks at a specific line of code, needed for knowing if some blocks on a line weren't covered
}

pub struct DebugDataArc {
    pub file_map: Vec<Arc<path::PathBuf>>,
    pub blocks_per_line: Vec<(u64, u64)>, // stores the number of blocks at a specific line of code, needed for knowing if some blocks on a line weren't covered
}

impl From<DebugData> for DebugDataArc {
    fn from(value: DebugData) -> Self {
        DebugDataArc {
            file_map: value.file_map.into_iter().map(|p| Arc::new(p)).collect(),
            blocks_per_line: value.blocks_per_line,
        }
    }
}
