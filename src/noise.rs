#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NoiseLevel {
    Verbose,
    Standard,
    Quiet,
}

impl NoiseLevel {
    pub fn from_settings(verbose: bool, quiet: bool) -> NoiseLevel {
        debug_assert!(!(verbose && quiet));
        if verbose {
            NoiseLevel::Verbose
        } else if quiet {
            NoiseLevel::Quiet
        } else {
            NoiseLevel::Standard
        }
    }
    pub fn err(&self) -> bool {
        *self != NoiseLevel::Quiet
    }
    pub fn debug(&self) -> bool {
        *self == NoiseLevel::Verbose
    }
}
