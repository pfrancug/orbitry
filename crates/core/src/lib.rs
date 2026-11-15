//! Orbitry core — scanner, process manager, port detection, and project metadata.

mod artifacts;
mod config;
mod git;
mod scan;
mod scripts;

pub use artifacts::BuildInfo;
pub use config::{
    canonical, config_path, default_kind_patterns, display_path, load_config, save_config,
    Config, ScriptKind, ScriptPattern, ScriptScope,
};
pub use git::GitInfo;
pub use scan::{scan, Project, ProjectKind};
pub use scripts::DetectedScript;