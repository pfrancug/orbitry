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
    attach_run_warnings, config_fingerprint, diff_groups, evaluate_warnings, group_projects,
    load_config, load_scan_cache, now_ms, save_config, save_scan_cache, scan, scan_cache_path,
    Config, ListeningSnapshot, PortOverride, ProcessManager, Project, RepoGroup, Run,
    SCANNER_VERSION, ScanCache, ScanDiff, ScanState, ScanStatus, StartError, Subscription,
    Warning,
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio::sync::mpsc::UnboundedSender;

use crate::instance;
use crate::shutdown::Shutdown;
use crate::ui;

#[derive(Clone)]
struct Inventory {
    groups: Vec<RepoGroup>,
    projects: Vec<Project>,
    scan: ScanStatus,
}

struct AppState {
    config_path: PathBuf,
    cache_path: PathBuf,
    manager: Arc<ProcessManager>,
    inventory: StdMutex<Inventory>,
    scanning: AtomicBool,
}

#[derive(Serialize)]
struct ProjectsResponse {
    groups: Vec<RepoGroup>,
    projects: Vec<Project>,
    warnings: Vec<Warning>,
    scan: ScanStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigResponse {
    #[serde(flatten)]
    config: Config,
    default_patterns: Vec<DefaultPattern>,
}

#[derive(Serialize)]
struct DefaultPattern {
    kind: orbitry_core::ScriptKind,
    pattern: &'static str,
}

#[derive(Serialize)]
struct RunsResponse {
    runs: Vec<Run>,
    warnings: Vec<Warning>,
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
        if let Err(err) = serve(config_path, port, shutdown, ready).await {
            eprintln!("orbitry: {err}");
        }
    });
}

async fn serve(
    config_path: PathBuf,
    port: u16,
    shutdown: Shutdown,
    ready: UnboundedSender<Result<u16, String>>,
) -> Result<(), String> {
    let manager = ProcessManager::new();
    let config = load_config(&config_path).unwrap_or_default();
    manager
        .set_port_overrides(config.port_overrides.clone())
        .await;
    let cache_path = scan_cache_path();
    let inventory = if let Some(cache) = load_scan_cache(&cache_path, &config) {
        Inventory {
            groups: cache.groups,
            projects: Vec::new(),
            scan: ScanStatus::from_cache(cache.scanned_at),
        }
    } else {
        Inventory {
            groups: Vec::new(),
            projects: Vec::new(),
            scan: ScanStatus::empty_scanning(),
        }
    };
    let state = Arc::new(AppState {
        config_path,
        cache_path,
        manager: Arc::clone(&manager),
        inventory: StdMutex::new(inventory),
        scanning: AtomicBool::new(false),
    });

    let router = Router::new()
        .route("/api/projects", get(get_projects))
        .route("/api/scan", post(post_scan))
        .route("/api/config", get(get_config).put(put_config))
        .route("/api/run", post(post_run))
        .route("/api/stop", post(post_stop))
        .route("/api/stop-all", post(post_stop_all))
        .route("/api/runs", get(get_runs))
        .route("/api/runs/{id}/logs", get(logs_ws))
        .route("/api/pick-folder", post(post_pick_folder))
        .route("/api/open", post(post_open))
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

    let shutdown_manager = Arc::clone(&manager);
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = shutdown.wait() => {}
                _ = os_shutdown_signal() => {}
            }
            eprintln!("orbitry: shutting down, stopping child processes");
            shutdown_manager.stop_all().await;
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
    Ok(Json(ConfigResponse {
        config,
        default_patterns: orbitry_core::default_kind_patterns()
            .into_iter()
            .map(|(kind, pattern)| DefaultPattern { kind, pattern })
            .collect(),
    }))
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
}

async fn put_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ConfigBody>,
) -> Result<Json<ConfigResponse>, (StatusCode, String)> {
    let existing = load_config(&state.config_path).map_err(internal)?;
    let mut config = Config {
        roots: body.roots,
        script_patterns: body.script_patterns,
        port_overrides: body.port_overrides.unwrap_or(existing.port_overrides),
    };
    config.roots = config
        .roots
        .into_iter()
        .map(|root| root.trim().to_string())
        .filter(|root| !root.is_empty())
        .collect();
    config
        .script_patterns
        .retain(|pattern| !pattern.pattern.trim().is_empty());
    save_config(&state.config_path, &config).map_err(internal)?;
    state
        .manager
        .set_port_overrides(config.port_overrides.clone())
        .await;
    start_scan(Arc::clone(&state));
    Ok(Json(ConfigResponse {
        config,
        default_patterns: orbitry_core::default_kind_patterns()
            .into_iter()
            .map(|(kind, pattern)| DefaultPattern { kind, pattern })
            .collect(),
    }))
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
            body.script,
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
    Json(RunsResponse { runs, warnings })
}

async fn get_runs(State(state): State<Arc<AppState>>) -> Json<RunsResponse> {
    let runs = state.manager.list().await;
    let (runs, warnings) = annotate_runs(&state, runs).await;
    Json(RunsResponse { runs, warnings })
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
    let (groups, projects, scan) = match state.inventory.lock() {
        Ok(inventory) => (
            inventory.groups.clone(),
            inventory.projects.clone(),
            inventory.scan.clone(),
        ),
        Err(_) => (Vec::new(), Vec::new(), ScanStatus::empty_scanning()),
    };
    let runs = state.manager.list_cached().await;
    let warnings = collect_warnings(&groups, &runs).await;
    ProjectsResponse {
        groups,
        projects,
        warnings,
        scan,
    }
}

fn internal(err: String) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err)
}
