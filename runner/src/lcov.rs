//! A module containing structs to output LCov formatted coverage info

use std::{fmt::Display, path::PathBuf, sync::Arc};

use wat_annotator::debug::SourceDebugInfo;

use crate::gcov::GCovFile;

type FuncDef = (u64, Option<u64>, String); // line num of func start, func end, and name
#[allow(dead_code)]
type BranchDef = (u64, bool, u64, u64); // line num, is exception, block idx, branch idx,
type Fnda = (u64, usize); // execution count, func index
type Brda = (u64, bool, u64, u64, u64); // line num, is an exception, block idx, branch idx, times taken
                                        // What lines count as instrumented? idk we should figure that out
type DA = (u64, u64, Option</*should be an md5 */ u64>); // line num, exec count, hash

/// A struct represention a source file that is part of an LCov tracefile
pub struct SourceFile {
    path: Arc<PathBuf>,
    version: Option<u64>,
    functions: Vec<FuncDef>,
    func_exces: Vec<Fnda>,
    branch_coverage: Vec<Brda>,
    code_lines: Vec<DA>,
}

impl SourceFile {
    /// Create a new `SourceFile` from a Gcovfile containing counter information and a SourceDebugInfo struct
    pub fn new(counter_log: &GCovFile, sdi: &SourceDebugInfo) -> SourceFile {
        let path = counter_log.clone_src_file();
        let version = None;
        let functions = sdi
            .functions
            .iter()
            .map(|(start, end, str, _addr)| (*start, *end, str.clone()))
            .collect::<Vec<_>>();
        let func_exces = functions
            .iter()
            .enumerate()
            .map(|(idx, (start, _, _))| {
                let counters = counter_log.get_counters_for_line(*start);
                if counters.is_none() {
                    eprintln!("Error: function line has no counters");
                }
                (counters.unwrap(), idx)
            })
            .collect::<Vec<_>>();
        eprintln!("TODO: output branch info");
        let branch_coverage = Vec::new();
        let last_line = functions
            .iter()
            .filter_map(|(_, end, _)| *end)
            .max()
            .unwrap();
        // inefficient but it should work
        let mut code_lines = Vec::new();
        for line in 0..=last_line {
            if let Some(count) = counter_log.get_counters_for_line(line) {
                let da = (line, count, None);
                code_lines.push(da);
            }
        }
        SourceFile {
            path,
            version,
            functions,
            func_exces,
            branch_coverage,
            code_lines,
        }
    }
}

impl Display for SourceFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "SF:{}", self.path.display())?;
        if let Some(v_id) = self.version {
            writeln!(f, "VER:{}", v_id)?;
        }
        for funcdef in &self.functions {
            write!(f, "FN:{},", funcdef.0)?;
            // TODO never record the end of a file (it actually isn';t part of the format)
            writeln!(f, "{}", funcdef.2)?;
        }
        for exec in &self.func_exces {
            writeln!(f, "FNDA:{},{}", exec.0, self.functions[exec.1].2)?;
        }
        writeln!(f, "FNF:{}", self.functions.len())?;
        writeln!(f, "FNH:{}", self.func_exces.len())?;
        for branch in &self.branch_coverage {
            writeln!(
                f,
                "BRDA:{},{}{},{},{}",
                branch.0,
                if branch.1 { "e" } else { "" },
                branch.2,
                branch.3,
                if branch.4 == 0 {
                    "-".to_string()
                } else {
                    branch.4.to_string()
                }
            )?;
        }
        writeln!(f, "BRF:{}", self.branch_coverage.len())?;
        writeln!(
            f,
            "BRH:{}",
            self.branch_coverage
                .iter()
                .filter(|(_, _, _, _, taken)| *taken > 0)
                .count()
        )?;
        for instrumented_line in &self.code_lines {
            write!(f, "DA:{},{}", instrumented_line.0, instrumented_line.1)?;
            if let Some(hash) = instrumented_line.2 {
                write!(f, ",{}", hash)?;
            }
            writeln!(f)?;
        }
        writeln!(
            f,
            "LH:{}",
            self.code_lines
                .iter()
                .filter(|(_, count, _)| *count > 0)
                .count()
        )?;
        writeln!(f, "LF:{}", self.code_lines.len())?;
        write!(f, "end_of_record")?;
        Ok(())
    }
}

/// A struct representing an Lcov tracefile, with file extension  ".info"
pub struct TraceFile {
    test_name: Option<String>,
    files: Vec<SourceFile>,
}

impl TraceFile {
    /// Create a new tracefile from an optional name and a list of sourcefiles
    pub fn new(name: Option<&str>, files: Vec<SourceFile>) -> TraceFile {
        TraceFile {
            test_name: name.map(String::from),
            files,
        }
    }
}

impl Display for TraceFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(tn) = &self.test_name {
            writeln!(f, "TN:{}", tn)?;
        }
        for sf in &self.files {
            // Some file paths don't actually exist
            // TODO: expand this to more systems instead of just here
            if sf.path.exists() {
                writeln!(f, "{}", sf)?;
            }
        }

        Ok(())
    }
}
