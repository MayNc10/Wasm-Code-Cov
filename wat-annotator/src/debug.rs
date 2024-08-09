use core::str;
use std::collections::HashMap;
use std::path;

use serde::{Deserialize, Serialize};
use wasmparser::{BinaryReaderError, Parser, Payload::*};
use wast::core::{Custom, ModuleField};
use wast::{component::*, Wat};
use wast::{parser, Error};

use crate::data::DebugData;
use crate::utils::*;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
/// This struct represents debugging infomation about a specific line of Wasm code
pub struct DebugLineInfo {
    /// The address within the `code` section of the module
    pub address: u64,
    /// The index in the path table corresponding to the source file for this line
    pub path_idx: usize,
    /// The source line
    pub line: u64,
    /// The source column
    pub column: u64,
    /// The index of the inline core module where the compiled code this represents is located
    pub code_module_idx: usize,
}

/// This struct contains overall debugging information for a Webassembly file
pub struct WatLineMapper {
    code_offsets: Vec<usize>,
    lines: Vec<DebugLineInfo>,
    file_map: Vec<path::PathBuf>,
}

impl WatLineMapper {
    /// Create a new `WatLineMapper` from a list of offsets of code sections
    pub fn new(offsets: Vec<usize>) -> WatLineMapper {
        WatLineMapper {
            code_offsets: offsets,
            lines: Vec::new(),
            file_map: Vec::new(),
        }
    }
    /// Add a debug line
    pub fn add_line(&mut self, line: DebugLineInfo) {
        if !self.lines().contains(&line) {
            self.lines.push(line);
        } else {
            panic!("duplicate lines???");
        }
    }
    /// Gets the all the currently held debug lines
    pub fn lines(&self) -> &Vec<DebugLineInfo> {
        &self.lines
    }
    /// The function gets the source triplet (file, line, column) of an instruction in code, given
    /// The inline module idx is how many inline modules have been seen before this, and the binary offset is grabbed from the comments at the start of each line
    pub fn get_source_triplet(
        &self,
        inline_module_idx: usize,
        binary_offset: u64,
    ) -> Option<&DebugLineInfo> {
        let pc_offset = binary_offset - self.code_offsets[inline_module_idx] as u64;
        self.lines
            .iter()
            .filter(|info| info.code_module_idx == inline_module_idx && info.address <= pc_offset)
            .max_by(|i1, i2| i1.address.cmp(&i2.address))
    }
    /// Consumes this struct and returns a `DebugData` struct representing information that should be passed to other programs
    pub fn into_debug_data(self) -> DebugData {
        let mut blocks_per_line = Vec::new();
        self.lines
            .into_iter()
            .map(|dli| dli.line)
            .for_each(|this_line| {
                if let Some(pos) = blocks_per_line
                    .iter()
                    .map(|(line, _)| line)
                    .position(|line| *line == this_line)
                {
                    blocks_per_line[pos] = (this_line, blocks_per_line[pos].1 + 1);
                } else {
                    blocks_per_line.push((this_line, 1));
                }
            });
        DebugData {
            file_map: self.file_map,
            blocks_per_line,
        }
    }
}

/// Fill in a mapper struct with debug information contained in a Wat file
/// The `text` argument should be the plaintext string that the `wat` argument was created from
pub fn read_dbg_info(wat: &Wat, text: &str, map: &mut WatLineMapper) -> parser::Result<()> {
    let mut code_module_idx = 0;
    for field in get_fields(&wat).ok_or(Error::new(
        wat.span(),
        "Input WAT file could not be parsed (may be binary or module)".to_string(),
    ))? {
        if let ComponentField::CoreModule(m) = field {
            if let CoreModuleKind::Inline { fields } = &m.kind {
                let mut section_map = HashMap::new();
                for field in fields {
                    if let ModuleField::Custom(c) = field {
                        if let Custom::Raw(c) = c {
                            let flattened_slice: Vec<u8> =
                                c.data.iter().map(|a| Vec::from(*a)).flatten().collect(); // is there a way to do this without allocating?
                            section_map.insert(c.name, flattened_slice);
                        }
                    }
                }
                let dwarf_sections = gimli::DwarfSections::load(|sec| {
                    Ok(section_map
                        .get(sec.name())
                        .map(|v| v.as_slice())
                        .unwrap_or(Default::default()))
                })?;
                let dwarf = dwarf_sections
                    .borrow(|section| gimli::EndianSlice::new(section, gimli::LittleEndian));
                let mut iter = dwarf.units();
                while let Some(header) = iter.next().unwrap() {
                    eprintln!(
                        "Unit at <.debug_info+0x{:x}>",
                        header.offset().as_debug_info_offset().unwrap().0
                    );
                    let unit = dwarf.unit(header).unwrap();
                    let unit = unit.unit_ref(&dwarf);

                    if let Some(program) = unit.line_program.clone() {
                        let comp_dir = if let Some(ref dir) = unit.comp_dir {
                            path::PathBuf::from(dir.to_string_lossy().into_owned())
                        } else {
                            path::PathBuf::new()
                        };

                        // Iterate over the line program rows.
                        let mut rows = program.rows();
                        while let Some((header, row)) = rows.next_row().unwrap() {
                            if row.end_sequence() {
                                // End of sequence indicates a possible gap in addresses.
                                eprintln!("{:x} end-sequence", row.address());
                            } else {
                                // Determine the path. Real applications should cache this for performance.
                                let mut path = path::PathBuf::new();
                                if let Some(file) = row.file(header) {
                                    path.clone_from(&comp_dir);

                                    // The directory index 0 is defined to correspond to the compilation unit directory.
                                    if file.directory_index() != 0 {
                                        if let Some(dir) = file.directory(header) {
                                            path.push(
                                                unit.attr_string(dir)
                                                    .unwrap()
                                                    .to_string_lossy()
                                                    .as_ref(),
                                            );
                                        }
                                    }

                                    path.push(
                                        unit.attr_string(file.path_name())
                                            .unwrap()
                                            .to_string_lossy()
                                            .as_ref(),
                                    );
                                }

                                // Determine line/column. DWARF line/column is never 0, so we use that
                                // but other applications may want to display this differently.
                                let line = match row.line() {
                                    Some(line) => line.get(),
                                    None => 0,
                                };
                                let column = match row.column() {
                                    gimli::ColumnType::LeftEdge => 0,
                                    gimli::ColumnType::Column(column) => column.get(),
                                };
                                let text_offset = row.address() as usize + m.span.offset();
                                //eprintln!("{:x}", text_offset);
                                //panic!();

                                eprintln!(
                                    "{:x} (%{:?}) {}:{}:{}",
                                    row.address(),
                                    str::from_utf8(&text.as_bytes()[text_offset..text_offset + 10]),
                                    path.display(),
                                    line,
                                    column
                                );

                                let path_idx =
                                    map.file_map.iter().position(|p| *p == path).unwrap_or({
                                        map.file_map.push(path);
                                        map.file_map.len() - 1
                                    });

                                let info = DebugLineInfo {
                                    address: row.address(),
                                    path_idx,
                                    line,
                                    column,
                                    code_module_idx,
                                };
                                map.add_line(info);
                            }
                        }
                    }
                }
                code_module_idx += 1;
            }
        }
    }
    Ok(())
}

/// Find the code section offsets in a binary Wasm file
pub fn find_code_offsets(input: &[u8]) -> Result<Vec<usize>, BinaryReaderError> {
    let mut code_offsets = Vec::new();
    for payload in Parser::new(0).parse_all(input) {
        if let CodeSectionStart {
            count: _, range, ..
        } = payload?
        {
            code_offsets.push(range.start);
        }
    }
    Ok(code_offsets)
}
