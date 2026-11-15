use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post};
use axum::Router;
use clap::Parser;
use orbitry_core::{
    config_path, group_projects, load_config, save_config, scan, Config, ProcessManager, RepoGroup,
    Run, StartError, Subscription,
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tower_http::services::{ServeDir, ServeFile};

#[derive(Parser, Debug)]
#[command(
    name = "orbitry",
    version,
    about = "A local command center for every app in orbit."
)]
struct Args {
    /// Project root to scan (repeatable). Added to the persisted user config.
    #[arg(long)]
    root: Vec<PathBuf>,

    /// HTTP port for the dashboard and API.
    #[arg(long, default_value_t = 4280)]
    port: u16,

    /// Do not open the browser after start.
    #[arg(long)]
    no_open: bool,
}

#[derive(Clone)]
struct AppState {
    config_path: PathBuf,
    manager: Arc<ProcessManager>,
}

#[derive(Serialize)]
struct ProjectsResponse {
    groups: Vec<RepoGroup>,
    projects: Vec<orbitry_core::Project>,
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

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("orbitry: {err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let args = Args::parse();
    let path = config_path();
    let mut config = load_config(&path)?;
    for root in &args.root {
        let displayed = orbitry_core::display_path(&orbitry_core::canonical(root));
        config = config.with_root(displayed);
    }
    if !args.root.is_empty() {
        save_config(&path, &config)?;
    }

    let manager = ProcessManager::new();
    let state = Arc::new(AppState {
        config_path: path.clone(),
        manager: Arc::clone(&manager),
    });

    let mut router = Router::new()
        .route("/api/projects", get(get_projects))
        .route("/api/config", get(get_config).put(put_config))
        .route("/api/run", post(post_run))
        .route("/api/stop", post(post_stop))
        .route("/api/runs", get(get_runs))
        .route("/api/runs/{id}/logs", get(logs_ws))
        .with_state(state);

    if let Some(ui_dir) = find_ui_dist() {
        let index = ui_dir.join("index.html");
        router = router
            .fallback_service(ServeDir::new(&ui_dir).not_found_service(ServeFile::new(index)));
    } else {
        router = router.fallback(fallback_placeholder);
    }

    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|err| format!("bind {addr}: {err}"))?;
    let url = format!("http://127.0.0.1:{}", args.port);
    println!("Orbitry is running at {url}");
    if !args.no_open {
        open_browser(&url);
    }

    let shutdown_manager = Arc::clone(&manager);
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            eprintln!("orbitry: shutting down, stopping child processes");
            shutdown_manager.stop_all().await;
        })
        .await
        .map_err(|err| format!("server: {err}"))
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(windows)]
    {
        let mut close = tokio::signal::windows::ctrl_close()
            .ok()
            .or_else(|| tokio::signal::windows::ctrl_close().ok());
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
    let config = load_config(&state.config_path).map_err(internal)?;
    let projects = scan(&config);
    let groups = group_projects(projects.clone(), &config.roots);
    Ok(Json(ProjectsResponse { groups, projects }))
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

async fn put_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Config>,
) -> Result<Json<ConfigResponse>, (StatusCode, String)> {
    let mut config = body;
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
    let config = load_config(&state.config_path).map_err(internal)?;
    let projects = scan(&config);
    let project = projects
        .iter()
        .find(|item| item.id == body.part_id || item.path == body.part_id)
        .ok_or((StatusCode::NOT_FOUND, "part not found".into()))?;
    if !project
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
            project.id.clone(),
            project.name.clone(),
            project.path.clone(),
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

async fn get_runs(State(state): State<Arc<AppState>>) -> Json<RunsResponse> {
    Json(RunsResponse {
        runs: state.manager.list().await,
    })
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

async fn fallback_placeholder() -> impl IntoResponse {
    (
        StatusCode::OK,
        [("content-type", "text/html; charset=utf-8")],
        "<!doctype html><meta charset=utf-8><title>Orbitry</title><body style=\"font-family:system-ui;background:#09090d;color:#f4f4f7;padding:48px\"><h1>Orbitry</h1><p>The API is running. Build the UI with <code>npm run build</code> in <code>ui/</code> and restart.</p></body>",
    )
}

fn internal(err: String) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err)
}

fn find_ui_dist() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("ui").join("dist"));
        candidates.push(cwd.join("dist"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("ui").join("dist"));
            candidates.push(
                dir.join("../ui/dist")
                    .canonicalize()
                    .unwrap_or(dir.join("../ui/dist")),
            );
        }
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui/dist"));
    candidates
        .into_iter()
        .find(|path| path.join("index.html").exists())
}

fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}
