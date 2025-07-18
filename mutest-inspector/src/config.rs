use std::path::PathBuf;
use clap::ValueEnum;

/// Defines the available diffing strategies used by the renderer to generate line diffs.
#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum SysDiffType {
    Simple,
    Advanced,
}

pub struct ServerConfig {
    pub pre_cache_all: bool,
    pub sys_diff_type: SysDiffType,
    pub port: String,
    pub results_dir: PathBuf,
    pub resource_dir: PathBuf,
    pub source_dir: Option<PathBuf>,
}