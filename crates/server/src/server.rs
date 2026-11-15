use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post};
use axum::Router;
use orbitry_core::{
    group_projects, load_config, save_config, scan, Config, ProcessManager, RepoGroup, Run,
    StartError, Subscription,
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio::sync::mpsc::UnboundedSender;

use crate::instance;
use crate::shutdown::Shutdown;
use crate::ui;

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
    let state = Arc::new(AppState {
        config_path,
        manager: Arc::clone(&manager),
    });

    let router = Router::new()
        .route("/api/projects", get(get_projects))
        .route("/api/config", get(get_config).put(put_config))
        .route("/api/run", post(post_run))
        .route("/api/stop", post(post_stop))
        .route("/api/stop-all", post(post_stop_all))
        .route("/api/runs", get(get_runs))
        .route("/api/runs/{id}/logs", get(logs_ws))
        .fallback(ui::serve_ui)
        .with_state(state);

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

async fn post_stop_all(State(state): State<Arc<AppState>>) -> Json<RunsResponse> {
    state.manager.stop_all().await;
    Json(RunsResponse {
        runs: state.manager.list().await,
    })
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

fn internal(err: String) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err)
}
