//! Orbitry core - scanner, process manager, port detection, and project metadata.

mod app_autostart;
mod artifacts;
mod autostart;
mod bridge;
mod config;
mod docker;
mod git;
mod group;
mod health;
mod logpump;
mod logs;
mod metrics;
mod models;
mod manager;
mod ports;
mod ports_resolve;
mod process;
mod scan;
mod scan_cache;
mod scripts;
mod util;
mod warnings;

pub use app_autostart::{
    run_app_autostart, AppAutostartEvent, AppAutostartOutcome, AppAutostartReport, AppAutostartState,
};
pub use artifacts::BuildInfo;
pub use autostart::{
    autostart_status, refresh_autostart_path, set_autostart_enabled, AutostartStatus,
};
pub use bridge::{bridge_status, export_cache, BridgeCacheEntry, BridgeStatus};
pub use docker::{
    compose_file_in, compose_logs, parse_compose_services, refresh_compose, run_compose_action,
    static_compose_projects, ComposeAction, ComposeProject, ComposeService,
};
pub use config::{
    canonical, config_path, default_kind_patterns, display_path, instance_path, load_config,
    paths_equal, save_config, Config, ConfigBridgeMode, PortOverride, ScriptKind, ScriptPattern, ScriptScope,
};
pub use git::{find_git_root, GitInfo};
pub use health::{check_health, HealthInfo, HealthStatus};
pub use group::{group_projects, PartRole, ProjectPart, RepoGroup};
pub use logpump::run_log_pump;
pub use logs::{clamp_log_limit_mb, DEFAULT_LOG_LIMIT_MB, MAX_LOG_LIMIT_MB, MIN_LOG_LIMIT_MB};
pub use manager::{ProcessManager, StartError, Subscription};
pub use metrics::{MetricsHub, MetricsSnapshot};
pub use models::{
    empty_models_inventory, load_models_cache, models_cache_path, models_fingerprint, save_models_cache,
    scan_models, scan_models_with_sink, CacheSource, CachedModel, ModelUsage, ModelsInventory, ModelsScanState, ModelsScanStatus,
    MODELS_SCANNER_VERSION, OllamaInventory, OllamaModel, ProjectWeights, SourceTotal, WeightFile,
};
pub use ports::{
    apply_port_overrides, detect_expected_ports, detect_static_ports, listening_ports_for,
    Listener, ListeningSnapshot, PortInfo, PortSource,
};
pub use process::{detect_package_manager, LogLine, LogStream, Run, RunEvent, RunStatus};
pub use scan::{scan, Project, ProjectKind};
pub use scan_cache::{
    config_fingerprint, diff_groups, load_scan_cache, now_ms, save_scan_cache, scan_cache_path,
    ScanCache, ScanChange, ScanChangeKind, ScanDiff, ScanState, ScanStatus, SCANNER_VERSION,
};
pub use scripts::{DetectedScript, ScriptSource};
pub use warnings::{
    attach_run_warnings, evaluate_warnings, Warning, WarningSeverity,
};
