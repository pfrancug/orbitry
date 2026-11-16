use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post};
use axum::Router;
use orbitry_core::{
    apply_port_overrides, attach_run_warnings, autostart_status, bridge_status, clamp_log_limit_mb,
    config_fingerprint, diff_groups, evaluate_warnings, export_cache, group_projects, load_config, paths_equal,
    empty_models_inventory, load_models_cache, load_scan_cache, models_cache_path, now_ms,
    refresh_compose, run_app_autostart, run_compose_action, compose_logs, save_config,
    save_models_cache, save_scan_cache, scan, scan_cache_path, scan_models_with_sink,
    set_autostart_enabled, static_compose_projects, AppAutostartReport, AutostartStatus,
    BridgeStatus, ComposeAction, ComposeProject, Config,
    ConfigBridgeMode, ListeningSnapshot, MetricsHub, MetricsSnapshot, ModelsInventory,
    ModelsScanState, PortOverride, ProcessManager, Project, RepoGroup, Run, RunStatus,
    SCANNER_VERSION, ScanCache, ScanDiff, ScanState, ScanStatus, StartError, Subscription, Warning,
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio::sync::mpsc::UnboundedSender;

use crate::instance;
use crate::shutdown::Shutdown;
use crate::ui;
use crate::update::{is_dev_build, UpdateService, VersionInfo};

#[derive(Clone)]
struct Inventory {
    groups: Vec<RepoGroup>,
    projects: Vec<Project>,
    scan: ScanStatus,
    warnings: Vec<Warning>,
}

#[derive(Default)]
struct NotifySeen {
    failed_runs: std::collections::HashSet<String>,
    unhealthy: std::collections::HashSet<String>,
}

struct AppState {
    config_path: PathBuf,
    cache_path: PathBuf,
    manager: Arc<ProcessManager>,
    inventory: StdMutex<Inventory>,
    scanning: AtomicBool,
    update: UpdateService,
    shutdown: Shutdown,
    port: u16,
    metrics: MetricsHub,
    startup_autostart: AtomicBool,
    app_autostart: StdMutex<AppAutostartReport>,
    autostart_warnings: StdMutex<Vec<Warning>>,
    models: Arc<StdMutex<ModelsInventory>>,
    models_scanning: AtomicBool,
    models_cache_path: PathBuf,
    compose: Arc<StdMutex<Vec<ComposeProject>>>,
    notify_seen: StdMutex<NotifySeen>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectsResponse {
    groups: Vec<RepoGroup>,
    projects: Vec<Project>,
    warnings: Vec<Warning>,
    scan: ScanStatus,
    app_autostart: AppAutostartReport,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    compose: Vec<ComposeProject>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigResponse {
    #[serde(flatten)]
    config: Config,
    default_patterns: Vec<DefaultPattern>,
    bridge_status: BridgeStatus,
    autostart: AutostartStatus,
}

#[derive(Serialize)]
struct DefaultPattern {
    kind: orbitry_core::ScriptKind,
    pattern: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RunsResponse {
    runs: Vec<Run>,
    warnings: Vec<Warning>,
    app_autostart: AppAutostartReport,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunRequest {
    part_id: String,
    script: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StopRequest {
    run_id: String,
}

pub fn run_blocking(
    config_path: PathBuf,
    port: u16,
    shutdown: Shutdown,
    update: UpdateService,
    ready: UnboundedSender<Result<u16, String>>,
) {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            let _ = ready.send(Err(format!("tokio runtime: {err}")));
            return;
        }
    };
    runtime.block_on(async move {
        if let Err(err) = serve(config_path, port, shutdown, update, ready).await {
            eprintln!("orbitry: {err}");
        }
    });
}

async fn serve(
    config_path: PathBuf,
    port: u16,
    shutdown: Shutdown,
    update: UpdateService,
    ready: UnboundedSender<Result<u16, String>>,
) -> Result<(), String> {
    let manager = ProcessManager::new();
    let config = load_config(&config_path).unwrap_or_default();
    manager
        .set_log_limit_mb(config.log_limit_mb)
        .await;
    manager.restore().await;
    let metrics = MetricsHub::start(manager.pid_index().clone());
    manager
        .set_port_overrides(config.port_overrides.clone())
        .await;
    let cache_path = scan_cache_path();
    let models_cache_path = models_cache_path();
    let inventory = if let Some(cache) = load_scan_cache(&cache_path, &config) {
        Inventory {
            groups: cache.groups,
            projects: Vec::new(),
            scan: ScanStatus::from_cache(cache.scanned_at),
            warnings: Vec::new(),
        }
    } else {
        Inventory {
            groups: Vec::new(),
            projects: Vec::new(),
            scan: ScanStatus::empty_scanning(),
            warnings: Vec::new(),
        }
    };
    let state = Arc::new(AppState {
        config_path,
        cache_path,
        manager: Arc::clone(&manager),
        inventory: StdMutex::new(inventory),
        scanning: AtomicBool::new(false),
        update: update.clone(),
        shutdown: shutdown.clone(),
        port,
        metrics,
        startup_autostart: AtomicBool::new(true),
        app_autostart: StdMutex::new(AppAutostartReport::default()),
        autostart_warnings: StdMutex::new(Vec::new()),
        models: Arc::new(StdMutex::new(load_models_cache(&models_cache_path, &config).unwrap_or_else(empty_models_inventory))),
        models_scanning: AtomicBool::new(false),
        models_cache_path,
        compose: Arc::new(StdMutex::new(Vec::new())),
        notify_seen: StdMutex::new(NotifySeen::default()),
    });

    let router = Router::new()
        .route("/api/projects", get(get_projects))
        .route("/api/scan", post(post_scan))
        .route("/api/config", get(get_config).put(put_config))
        .route("/api/run", post(post_run))
        .route("/api/install", post(post_install))
        .route("/api/stop", post(post_stop))
        .route("/api/stop-all", post(post_stop_all))
        .route("/api/runs", get(get_runs))
        .route("/api/runs/{id}/logs", get(logs_ws))
        .route("/api/pick-folder", post(post_pick_folder))
        .route("/api/open", post(post_open))
        .route("/api/version", get(get_version))
        .route("/api/update/check", post(post_update_check))
        .route("/api/update/status", get(get_update_status))
        .route("/api/update", post(post_update))
        .route("/api/tools", get(get_tools))
        .route("/api/metrics", get(get_metrics))
        .route("/api/models", get(get_models).post(post_models_scan))
        .route("/api/docker", get(get_docker).post(post_docker))
        .route("/api/docker/logs", get(get_docker_logs))
        .fallback(ui::serve_ui)
        .with_state(state.clone());

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(err) => {
            let message = if err.kind() == std::io::ErrorKind::AddrInUse {
                "already-bound".to_string()
            } else {
                format!("bind {addr}: {err}")
            };
            let _ = ready.send(Err(message));
            return Err(format!("bind {addr}: {err}"));
        }
    };

    if let Err(err) = instance::write(port) {
        let _ = ready.send(Err(err.clone()));
        return Err(err);
    }
    let _ = ready.send(Ok(port));
    println!("Orbitry is running at {}", instance::dashboard_url(port));
    start_scan(Arc::clone(&state));
    start_models_scan(Arc::clone(&state));
    spawn_compose_refresher(Arc::clone(&state));
    spawn_update_checker(Arc::clone(&state));
    spawn_live_refresh(Arc::clone(&state));



    let shutdown_manager = Arc::clone(&manager);
    let shutdown_flag = shutdown.clone();
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = shutdown_flag.wait() => {}
                _ = os_shutdown_signal() => {
                    shutdown_flag.request_quit();
                }
            }
            if shutdown_flag.stops_apps() {
                eprintln!("orbitry: shutting down, stopping child processes");
                shutdown_manager.stop_all().await;
            } else {
                eprintln!("orbitry: restarting for update, leaving managed apps running");
            }
            instance::clear();
        })
        .await
        .map_err(|err| format!("server: {err}"))
}

async fn os_shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(windows)]
    {
        let mut close = tokio::signal::windows::ctrl_close().ok();
        let mut shutdown = tokio::signal::windows::ctrl_shutdown().ok();
        tokio::select! {
            _ = ctrl_c => {}
            _ = async {
                if let Some(signal) = close.as_mut() {
                    signal.recv().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {}
            _ = async {
                if let Some(signal) = shutdown.as_mut() {
                    signal.recv().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {}
        }
    }
    #[cfg(not(windows))]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
        tokio::select! {
            _ = ctrl_c => {}
            _ = async {
                if let Some(signal) = terminate.as_mut() {
                    signal.recv().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {}
        }
    }
}

async fn get_projects(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ProjectsResponse>, (StatusCode, String)> {
    Ok(Json(snapshot_response(&state).await))
}

async fn post_scan(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ProjectsResponse>, (StatusCode, String)> {
    start_scan(Arc::clone(&state));
    Ok(Json(snapshot_response(&state).await))
}

async fn get_config(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ConfigResponse>, (StatusCode, String)> {
    let config = load_config(&state.config_path).map_err(internal)?;
    Ok(Json(config_response(config)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigBody {
    #[serde(default)]
    roots: Vec<String>,
    #[serde(default)]
    script_patterns: Vec<orbitry_core::ScriptPattern>,
    #[serde(default)]
    port_overrides: Option<Vec<PortOverride>>,
    #[serde(default)]
    config_bridge: Option<ConfigBridgeMode>,
    #[serde(default)]
    auto_check_updates: Option<bool>,
    #[serde(default)]
    log_limit_mb: Option<u32>,
    /// Applies to the OS autostart entry immediately. Omitted means leave it unchanged.
    #[serde(default)]
    start_with_system: Option<bool>,
    /// Replaces the per-app autostart list when present. Omitted keeps the saved list.
    #[serde(default)]
    autostart_apps: Option<Vec<String>>,
    #[serde(default)]
    desktop_notifications: Option<bool>,
    /// Replaces the ignored-project list when present. Omitted keeps the saved list.
    #[serde(default)]
    ignored_projects: Option<Vec<String>>,
}

async fn put_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ConfigBody>,
) -> Result<Json<ConfigResponse>, (StatusCode, String)> {
    let existing = load_config(&state.config_path).map_err(internal)?;
    let mut config = Config {
        roots: body.roots,
        script_patterns: body.script_patterns,
        port_overrides: body.port_overrides.unwrap_or_else(|| existing.port_overrides.clone()),
        config_bridge: body.config_bridge.unwrap_or(existing.config_bridge),
        auto_check_updates: body
            .auto_check_updates
            .unwrap_or(existing.auto_check_updates),
        log_limit_mb: body
            .log_limit_mb
            .map(clamp_log_limit_mb)
            .unwrap_or(existing.log_limit_mb),
        autostart_apps: body
            .autostart_apps
            .unwrap_or_else(|| existing.autostart_apps.clone()),
        desktop_notifications: body
            .desktop_notifications
            .unwrap_or(existing.desktop_notifications),
        ignored_projects: body
            .ignored_projects
            .unwrap_or_else(|| existing.ignored_projects.clone()),
    };
    config.normalize_autostart_apps();
    config.normalize_ignored_projects();
    config.roots = config
        .roots
        .into_iter()
        .map(|root| root.trim().to_string())
        .filter(|root| !root.is_empty())
        .collect();
    config
        .script_patterns
        .retain(|pattern| !pattern.pattern.trim().is_empty());
    let roots_or_patterns_changed =
        config.roots != existing.roots || config.script_patterns != existing.script_patterns;
    let bridge_changed = config.config_bridge != existing.config_bridge;
    let overrides_changed = config.port_overrides != existing.port_overrides;
    save_config(&state.config_path, &config).map_err(internal)?;
    if overrides_changed {
        state
            .manager
            .set_port_overrides(config.port_overrides.clone())
            .await;
        apply_overrides_to_inventory(&state, &config.port_overrides);
    }
    if config.log_limit_mb != existing.log_limit_mb {
        let manager = Arc::clone(&state.manager);
        let limit = config.log_limit_mb;
        tokio::spawn(async move {
            manager.set_log_limit_mb(limit).await;
        });
    }
    if config.roots != existing.roots {
        start_models_scan(Arc::clone(&state));
    }
    if roots_or_patterns_changed || bridge_changed {
        start_scan(Arc::clone(&state));
    }
    if let Some(enabled) = body.start_with_system {
        tokio::task::spawn_blocking(move || set_autostart_enabled(enabled))
            .await
            .map_err(|err| internal(format!("autostart task: {err}")))?
            .map_err(internal)?;
    }

    Ok(Json(config_response(config)))
}

async fn post_run(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RunRequest>,
) -> Result<Json<Run>, (StatusCode, String)> {
    let groups = {
        let inventory = state
            .inventory
            .lock()
            .map_err(|_| internal("inventory lock".into()))?;
        if inventory.scan.state != ScanState::Fresh {
            return Err((
                StatusCode::CONFLICT,
                "Verifying… waiting for rescan".into(),
            ));
        }
        inventory.groups.clone()
    };
    let part = groups
        .iter()
        .flat_map(|group| group.parts.iter())
        .find(|item| item.id == body.part_id || item.path == body.part_id)
        .ok_or((StatusCode::NOT_FOUND, "part not found".into()))?;
    if !part
        .scripts
        .iter()
        .any(|script| script.name == body.script)
    {
        return Err((
            StatusCode::NOT_FOUND,
            format!("script '{}' not found on this part", body.script),
        ));
    }
    match state
        .manager
        .start(
            part.id.clone(),
            part.name.clone(),
            part.path.clone(),
            body.script.clone(),
            part.scripts
                .iter()
                .find(|script| script.name == body.script)
                .and_then(|script| script.cwd.clone()),
        )
        .await
    {
        Ok(run) => Ok(Json(run)),
        Err(StartError::AlreadyRunning) => Err((
            StatusCode::CONFLICT,
            "This script is already running.".into(),
        )),
        Err(StartError::Message(err)) => Err(internal(err)),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallRequest {
    part_id: String,
}

async fn post_install(
    State(state): State<Arc<AppState>>,
    Json(body): Json<InstallRequest>,
) -> Result<Json<Run>, (StatusCode, String)> {
    let groups = {
        let inventory = state
            .inventory
            .lock()
            .map_err(|_| internal("inventory lock".into()))?;
        if inventory.scan.state != ScanState::Fresh {
            return Err((
                StatusCode::CONFLICT,
                "Verifying… waiting for rescan".into(),
            ));
        }
        inventory.groups.clone()
    };
    let part = groups
        .iter()
        .flat_map(|group| group.parts.iter())
        .find(|item| item.id == body.part_id || item.path == body.part_id)
        .ok_or((StatusCode::NOT_FOUND, "part not found".into()))?;
    match state
        .manager
        .start_install(part.id.clone(), part.name.clone(), part.path.clone())
        .await
    {
        Ok(run) => Ok(Json(run)),
        Err(StartError::AlreadyRunning) => Err((
            StatusCode::CONFLICT,
            "Dependencies are already being installed.".into(),
        )),
        Err(StartError::Message(err)) => Err(internal(err)),
    }
}

async fn post_stop(
    State(state): State<Arc<AppState>>,
    Json(body): Json<StopRequest>,
) -> Result<Json<Run>, (StatusCode, String)> {
    match state.manager.stop(&body.run_id).await {
        Ok(run) => Ok(Json(run)),
        Err(err) if err == "run not found" => Err((StatusCode::NOT_FOUND, "run not found".into())),
        Err(err) => Err(internal(err)),
    }
}

async fn post_stop_all(State(state): State<Arc<AppState>>) -> Json<RunsResponse> {
    state.manager.stop_all().await;
    let runs = state.manager.list().await;
    let (runs, warnings) = annotate_runs(&state, runs).await;
    Json(RunsResponse {
        runs,
        warnings,
        app_autostart: app_autostart_report(&state),
    })
}

async fn get_runs(State(state): State<Arc<AppState>>) -> Json<RunsResponse> {
    cached_runs_response(&state).await
}

async fn logs_ws(
    ws: WebSocketUpgrade,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.manager.subscribe(&id).await {
        Some(sub) => ws.on_upgrade(move |socket| handle_logs(socket, sub)),
        None => (StatusCode::NOT_FOUND, "run not found").into_response(),
    }
}

async fn handle_logs(mut socket: WebSocket, sub: Subscription) {
    let replay = serde_json::json!({
        "type": "replay",
        "run": sub.run,
        "lines": sub.lines,
    });
    if socket
        .send(Message::Text(replay.to_string().into()))
        .await
        .is_err()
    {
        return;
    }

    let mut rx = sub.rx;
    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Ok(event) => {
                        let Ok(payload) = serde_json::to_string(&event) else { continue };
                        if socket.send(Message::Text(payload.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(payload))) => {
                        if socket.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
        }
    }
}

async fn annotate_runs(state: &AppState, mut runs: Vec<Run>) -> (Vec<Run>, Vec<Warning>) {
    let groups = state
        .inventory
        .lock()
        .map(|inventory| inventory.groups.clone())
        .unwrap_or_default();
    let warnings = collect_warnings(&groups, &runs).await;
    attach_run_warnings(&mut runs, &warnings);
    (runs, warnings)
}

async fn collect_warnings(groups: &[RepoGroup], runs: &[Run]) -> Vec<Warning> {
    let snapshot = tokio::task::spawn_blocking(ListeningSnapshot::collect)
        .await
        .unwrap_or_default();
    evaluate_warnings(groups, runs, &snapshot.listeners)
}

#[derive(Serialize)]
struct PickFolderResponse {
    path: String,
}

#[derive(Deserialize)]
struct OpenRequest {
    kind: String,
    path: String,
}

async fn post_pick_folder() -> Result<impl IntoResponse, (StatusCode, String)> {
    let picked = tokio::task::spawn_blocking(crate::open::pick_folder)
        .await
        .map_err(|err| internal(format!("folder dialog: {err}")))?
        .map_err(internal)?;
    match picked {
        Some(path) => Ok(Json(PickFolderResponse {
            path: path.to_string_lossy().to_string(),
        })
        .into_response()),
        None => Ok(StatusCode::NO_CONTENT.into_response()),
    }
}

async fn post_open(
    Json(body): Json<OpenRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let path = PathBuf::from(body.path.trim());
    let result = tokio::task::spawn_blocking(move || match body.kind.as_str() {
        "terminal" => crate::open::open_in_terminal(&path),
        "vscode" => crate::open::open_in_vscode(&path),
        other => Err(format!("unknown open kind: {other}")),
    })
    .await
    .map_err(|err| internal(format!("open: {err}")))?;
    match result {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(err) if err.starts_with("folder not found") => Err((StatusCode::NOT_FOUND, err)),
        Err(err) if err.starts_with("unknown open kind") => Err((StatusCode::BAD_REQUEST, err)),
        Err(err) => Err((StatusCode::BAD_REQUEST, err)),
    }
}

fn start_scan(state: Arc<AppState>) {
    if state.scanning.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Ok(mut inventory) = state.inventory.lock() {
        inventory.scan.state = ScanState::Scanning;
        inventory.scan.started_at = Some(now_ms());
        inventory.scan.stale = !inventory.groups.is_empty();
        inventory.scan.diff = None;
        inventory.scan.error = None;
    }
    tokio::spawn(async move {
        let result = run_scan(&state).await;
        state.scanning.store(false, Ordering::SeqCst);
        refresh_live_state(&state).await;
        maybe_send_notifications(&state).await;
        if result.is_ok() {
            refresh_compose_state(&state).await;
            maybe_run_app_autostart(&state).await;
        }
        if let Err(err) = result {
            if let Ok(mut inventory) = state.inventory.lock() {
                inventory.scan.error = Some(err);
                inventory.scan.stale = !inventory.groups.is_empty();
                inventory.scan.state = if inventory.groups.is_empty() {
                    ScanState::Scanning
                } else {
                    ScanState::Cached
                };
            }
        }
    });
}

async fn run_scan(state: &AppState) -> Result<(), String> {
    let config = load_config(&state.config_path)?;
    state
        .manager
        .set_port_overrides(config.port_overrides.clone())
        .await;
    let cache_path = state.cache_path.clone();
    let fingerprint = config_fingerprint(&config);
    let (projects, groups) = tokio::task::spawn_blocking(move || {
        let roots = config.roots.clone();
        let projects = scan(&config);
        let groups = group_projects(projects.clone(), &roots);
        (projects, groups)
    })
    .await
    .map_err(|err| format!("scan task: {err}"))?;

    let scanned_at = now_ms();
    let cache = ScanCache {
        scanner_version: SCANNER_VERSION,
        fingerprint,
        scanned_at,
        groups: groups.clone(),
        bridge: export_cache(),
    };
    save_scan_cache(&cache_path, &cache)?;

    if let Ok(mut inventory) = state.inventory.lock() {
        let diff = if inventory.groups.is_empty() {
            ScanDiff::default()
        } else {
            diff_groups(&inventory.groups, &groups)
        };
        inventory.groups = groups;
        inventory.projects = projects;
        inventory.scan = ScanStatus {
            state: ScanState::Fresh,
            stale: false,
            scanned_at: Some(scanned_at),
            started_at: inventory.scan.started_at,
            diff: if diff.is_empty() { None } else { Some(diff) },
            error: None,
        };
    }
    Ok(())
}

async fn snapshot_response(state: &AppState) -> ProjectsResponse {
    let app_autostart = app_autostart_report(state);
    match state.inventory.lock() {
        Ok(inventory) => ProjectsResponse {
            groups: inventory.groups.clone(),
            projects: inventory.projects.clone(),
            warnings: inventory.warnings.clone(),
            scan: inventory.scan.clone(),
            app_autostart,
            compose: compose_snapshot(state),
        },
        Err(_) => ProjectsResponse {
            groups: Vec::new(),
            projects: Vec::new(),
            warnings: Vec::new(),
            scan: ScanStatus::empty_scanning(),
            app_autostart,
            compose: compose_snapshot(state),
        },
    }
}

async fn cached_runs_response(state: &AppState) -> Json<RunsResponse> {
    let mut runs = state.manager.list_cached().await;
    let warnings = state
        .inventory
        .lock()
        .map(|inventory| inventory.warnings.clone())
        .unwrap_or_default();
    attach_run_warnings(&mut runs, &warnings);
    Json(RunsResponse {
        runs,
        warnings,
        app_autostart: app_autostart_report(state),
    })
}

fn apply_overrides_to_inventory(state: &AppState, overrides: &[PortOverride]) {
    let Ok(mut inventory) = state.inventory.lock() else {
        return;
    };
    for group in &mut inventory.groups {
        for part in &mut group.parts {
            apply_port_overrides(&mut part.ports, &part.id, overrides);
        }
    }
    for project in &mut inventory.projects {
        apply_port_overrides(&mut project.ports, &project.id, overrides);
    }
}

fn spawn_live_refresh(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut first = true;
        loop {
            if !first {
                tokio::select! {
                    _ = state.shutdown.wait() => break,
                    _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {}
                }
            }
            first = false;
            refresh_live_state(&state).await;
        }
    });
}

async fn refresh_live_state(state: &AppState) {
    state.manager.refresh_live().await;
    let groups = state
        .inventory
        .lock()
        .map(|inventory| inventory.groups.clone())
        .unwrap_or_default();
    let runs = state.manager.list_cached().await;
    let mut warnings = collect_warnings(&groups, &runs).await;
    if let Ok(extra) = state.autostart_warnings.lock() {
        for warning in extra.iter() {
            let already = warnings.iter().any(|existing| {
                existing.code == warning.code
                    && existing.port == warning.port
                    && existing.part_id == warning.part_id
                    && existing.message == warning.message
            });
            if !already {
                warnings.push(warning.clone());
            }
        }
    }
    if let Ok(mut inventory) = state.inventory.lock() {
        inventory.warnings = warnings;
    }
    maybe_send_notifications(state).await;
}

fn app_autostart_report(state: &AppState) -> AppAutostartReport {
    state
        .app_autostart
        .lock()
        .map(|report| report.clone())
        .unwrap_or_default()
}

async fn maybe_run_app_autostart(state: &AppState) {
    if !state.startup_autostart.swap(false, Ordering::SeqCst) {
        return;
    }
    if !autostart_status().enabled {
        eprintln!(
            "orbitry: autostart: app autostart skipped (Orbitry is not set to start with the system)"
        );
        return;
    }
    let config = match load_config(&state.config_path) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("orbitry: autostart: {err}");
            return;
        }
    };
    if config.autostart_apps.is_empty() {
        return;
    }
    let (fresh, groups) = match state.inventory.lock() {
        Ok(inventory) => (
            inventory.scan.state == ScanState::Fresh,
            inventory.groups.clone(),
        ),
        Err(_) => {
            eprintln!("orbitry: autostart: inventory lock failed");
            return;
        }
    };
    if !fresh {
        eprintln!("orbitry: autostart: skipped (scan is not fresh)");
        return;
    }
    if let Ok(mut report) = state.app_autostart.lock() {
        *report = AppAutostartReport::running();
    }
    let shutdown = state.shutdown.clone();
    let (report, warnings) =
        run_app_autostart(&state.manager, &groups, &config, || shutdown.is_requested()).await;
    if let Ok(mut slot) = state.app_autostart.lock() {
        *slot = report;
    }
    if let Ok(mut slot) = state.autostart_warnings.lock() {
        *slot = warnings;
    }
    refresh_live_state(state).await;
}

fn config_response(config: Config) -> ConfigResponse {
    ConfigResponse {
        bridge_status: bridge_status(config.config_bridge),
        default_patterns: orbitry_core::default_kind_patterns()
            .into_iter()
            .map(|(kind, pattern)| DefaultPattern { kind, pattern })
            .collect(),
        autostart: autostart_status(),
        config,
    }
}


fn spawn_update_checker(state: Arc<AppState>) {
    if is_dev_build() {
        return;
    }
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(45)).await;
        loop {
            let auto = load_config(&state.config_path)
                .map(|config| config.auto_check_updates)
                .unwrap_or(true);
            if auto {
                if let Err(err) = state.update.check().await {
                    eprintln!("orbitry: update check: {err}");
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(6 * 60 * 60)).await;
        }
    });
}

fn version_payload(state: &AppState, auto_check: bool) -> VersionInfo {
    state.update.snapshot(auto_check)
}

async fn auto_check_flag(state: &AppState) -> bool {
    load_config(&state.config_path)
        .map(|config| config.auto_check_updates)
        .unwrap_or(true)
}

async fn get_version(
    State(state): State<Arc<AppState>>,
) -> Result<Json<VersionInfo>, (StatusCode, String)> {
    let auto_check = auto_check_flag(&state).await;
    Ok(Json(version_payload(&state, auto_check)))
}

async fn get_update_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<VersionInfo>, (StatusCode, String)> {
    get_version(State(state)).await
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolsInfo {
    vscode_installed: bool,
}

async fn get_metrics(State(state): State<Arc<AppState>>) -> Json<MetricsSnapshot> {
    Json(state.metrics.snapshot())
}

fn models_snapshot(state: &AppState) -> ModelsInventory {
    state
        .models
        .lock()
        .map(|inventory| inventory.clone())
        .unwrap_or_else(|_| empty_models_inventory())
}

async fn get_models(State(state): State<Arc<AppState>>) -> Json<ModelsInventory> {
    Json(models_snapshot(&state))
}

async fn post_models_scan(State(state): State<Arc<AppState>>) -> Json<ModelsInventory> {
    start_models_scan(state.clone());
    Json(models_snapshot(&state))
}

fn start_models_scan(state: Arc<AppState>) {
    if state.models_scanning.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Ok(mut inventory) = state.models.lock() {
        inventory.scan.state = ModelsScanState::Scanning;
        inventory.scan.started_at = Some(now_ms());
        inventory.scan.error = None;
        inventory.scan.stale = inventory.scan.scanned_at.is_some();
    }
    tokio::spawn(async move {
        let config = load_config(&state.config_path).unwrap_or_default();
        let groups = state
            .inventory
            .lock()
            .map(|inventory| inventory.groups.clone())
            .unwrap_or_default();
        let cache_path = state.models_cache_path.clone();
        let models = Arc::clone(&state.models);
        let result = tokio::task::spawn_blocking(move || {
            let inventory = scan_models_with_sink(&config, &groups, |partial| {
                if let Ok(mut slot) = models.lock() {
                    let started_at = slot.scan.started_at;
                    let previous_scanned = slot.scan.scanned_at;
                    *slot = partial.clone();
                    slot.scan.state = ModelsScanState::Scanning;
                    slot.scan.started_at = started_at.or(slot.scan.started_at);
                    slot.scan.scanned_at = previous_scanned;
                    slot.scan.stale = previous_scanned.is_some();
                }
            });
            if let Err(err) = save_models_cache(&cache_path, &inventory) {
                eprintln!("orbitry: models cache: {err}");
            }
            inventory
        })
        .await;
        match result {
            Ok(inventory) => {
                if let Ok(mut slot) = state.models.lock() {
                    *slot = inventory;
                }
            }
            Err(err) => {
                if let Ok(mut inventory) = state.models.lock() {
                    inventory.scan.state = if inventory.scan.scanned_at.is_some() {
                        ModelsScanState::Cached
                    } else {
                        ModelsScanState::Idle
                    };
                    inventory.scan.error = Some(format!("models scan: {err}"));
                }
            }
        }
        state.models_scanning.store(false, Ordering::SeqCst);
    });
}


async fn get_tools() -> Json<ToolsInfo> {
    Json(ToolsInfo {
        vscode_installed: crate::open::vscode_installed(),
    })
}

async fn post_update_check(
    State(state): State<Arc<AppState>>,
) -> Result<Json<VersionInfo>, (StatusCode, String)> {
    let auto_check = auto_check_flag(&state).await;
    if is_dev_build() {
        return Ok(Json(version_payload(&state, auto_check)));
    }
    match state.update.check().await {
        Ok(info) => {
            let mut info = info;
            info.auto_check = auto_check;
            Ok(Json(info))
        }
        Err(err) => Err((StatusCode::BAD_GATEWAY, err)),
    }
}

#[derive(Deserialize)]
struct UpdateRequest {
    #[serde(default)]
    confirm: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateResponse {
    started: bool,
    needs_confirm: bool,
    running: usize,
    #[serde(flatten)]
    version: VersionInfo,
}

async fn running_count(state: &AppState) -> usize {
    state
        .manager
        .list_cached()
        .await
        .iter()
        .filter(|run| run.status == RunStatus::Running)
        .count()
}

async fn post_update(
    State(state): State<Arc<AppState>>,
    body: Option<Json<UpdateRequest>>,
) -> Result<Json<UpdateResponse>, (StatusCode, String)> {
    let auto_check = auto_check_flag(&state).await;
    let version = version_payload(&state, auto_check);
    if is_dev_build() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Updates are disabled in development builds.".into(),
        ));
    }
    let confirm = body.map(|Json(body)| body.confirm).unwrap_or(false);
    let running = running_count(&state).await;
    if running > 0 && !confirm {
        return Ok(Json(UpdateResponse {
            started: false,
            needs_confirm: true,
            running,
            version,
        }));
    }
    let update = state.update.clone();
    let shutdown = state.shutdown.clone();
    let port = state.port;
    tokio::spawn(async move {
        if let Err(err) = update.install(shutdown, port).await {
            eprintln!("orbitry: update failed: {err}");
        }
    });
    Ok(Json(UpdateResponse {
        started: true,
        needs_confirm: false,
        running,
        version,
    }))
}

fn internal(err: String) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err)
}

fn compose_snapshot(state: &AppState) -> Vec<ComposeProject> {
    state
        .compose
        .lock()
        .map(|items| items.clone())
        .unwrap_or_default()
}

async fn refresh_compose_state(state: &AppState) {
    let groups = state
        .inventory
        .lock()
        .map(|inventory| inventory.groups.clone())
        .unwrap_or_default();
    let mut projects = static_compose_projects(&groups);
    refresh_compose(&mut projects).await;
    if let Ok(mut slot) = state.compose.lock() {
        *slot = projects;
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DockerBody {
    action: ComposeAction,
    project_id: String,
    #[serde(default)]
    service: Option<String>,
}

async fn get_docker(State(state): State<Arc<AppState>>) -> Json<Vec<ComposeProject>> {
    refresh_compose_state(&state).await;
    Json(compose_snapshot(&state))
}

async fn post_docker(
    State(state): State<Arc<AppState>>,
    Json(body): Json<DockerBody>,
) -> Result<Json<Vec<ComposeProject>>, (StatusCode, String)> {
    let project = compose_snapshot(&state)
        .into_iter()
        .find(|item| item.project_id == body.project_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "compose project not found".into()))?;
    run_compose_action(&project, body.action, body.service.as_deref())
        .await
        .map_err(internal)?;
    refresh_compose_state(&state).await;
    Ok(Json(compose_snapshot(&state)))
}

async fn get_docker_logs(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(query): axum::extract::Query<DockerLogQuery>,
) -> Result<Json<DockerLogs>, (StatusCode, String)> {
    let project = compose_snapshot(&state)
        .into_iter()
        .find(|item| item.project_id == query.project_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "compose project not found".into()))?;
    let text = compose_logs(&project, query.service.as_deref())
        .await
        .map_err(internal)?;
    Ok(Json(DockerLogs { text }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DockerLogQuery {
    project_id: String,
    #[serde(default)]
    service: Option<String>,
}

#[derive(Serialize)]
struct DockerLogs {
    text: String,
}

fn project_is_ignored(config: &Config, groups: &[RepoGroup], part_id: &str) -> bool {
    groups
        .iter()
        .find(|group| {
            group
                .parts
                .iter()
                .any(|part| paths_equal(&part.id, part_id))
        })
        .is_some_and(|group| config.ignores_project(&group.id, &group.path, &group.name))
}

async fn maybe_send_notifications(state: &AppState) {
    let config = load_config(&state.config_path).unwrap_or_default();
    if !config.desktop_notifications {
        return;
    }
    let groups = state
        .inventory
        .lock()
        .ok()
        .map(|inventory| inventory.groups.clone())
        .unwrap_or_default();
    let runs = state.manager.list_cached().await;
    let mut seen = match state.notify_seen.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    for run in &runs {
        if project_is_ignored(&config, &groups, &run.part_id) {
            continue;
        }
        if run.status == RunStatus::Failed && seen.failed_runs.insert(run.id.clone()) {
            crate::notify::show(
                "Orbitry",
                &format!("{} exited unexpectedly", run.part_name),
            );
        }
        if run.status == RunStatus::Running
            && run.health.status == orbitry_core::HealthStatus::Fail
            && seen.unhealthy.insert(run.id.clone())
        {
            crate::notify::show(
                "Orbitry",
                &format!("{} health check failed", run.part_name),
            );
        }
        if run.status != RunStatus::Failed {
            seen.failed_runs.remove(&run.id);
        }
        if run.health.status != orbitry_core::HealthStatus::Fail {
            seen.unhealthy.remove(&run.id);
        }
    }
}

fn spawn_compose_refresher(state: Arc<AppState>) {
    tokio::spawn(async move {
        // First fill from the cached inventory, then ask Docker if it is present.
        refresh_compose_state(&state).await;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            refresh_compose_state(&state).await;
        }
    });
}
