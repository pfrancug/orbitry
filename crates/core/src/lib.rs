//! Orbitry core - scanner, process manager, port detection, and project metadata.

mod artifacts;
mod config;
mod git;
mod group;
mod health;
mod manager;
mod ports;
mod ports_resolve;
mod process;
mod scan;
mod scripts;
mod util;

pub use artifacts::BuildInfo;
pub use config::{
    canonical, config_path, default_kind_patterns, display_path, instance_path, load_config,
    save_config, Config, ScriptKind, ScriptPattern, ScriptScope,
};
pub use git::{find_git_root, GitInfo};
pub use health::{check_health, HealthInfo, HealthStatus};
pub use group::{group_projects, PartRole, ProjectPart, RepoGroup};
pub use manager::{ProcessManager, StartError, Subscription};
pub use ports::{detect_static_ports, PortInfo, PortSource};
pub use process::{detect_package_manager, LogLine, LogStream, Run, RunEvent, RunStatus};
pub use scan::{scan, Project, ProjectKind};
pub use scripts::DetectedScript;
