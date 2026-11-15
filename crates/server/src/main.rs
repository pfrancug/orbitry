use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::routing::get;
use axum::Router;
use clap::Parser;
use orbitry_core::{config_path, group_projects, load_config, save_config, scan, Config, RepoGroup};
use serde::Serialize;
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

    let state = Arc::new(AppState {
        config_path: path.clone(),
    });

    let mut router = Router::new()
        .route("/api/projects", get(get_projects))
        .route("/api/config", get(get_config).put(put_config))
        .with_state(state);

    if let Some(ui_dir) = find_ui_dist() {
        let index = ui_dir.join("index.html");
        router = router.fallback_service(
            ServeDir::new(&ui_dir).not_found_service(ServeFile::new(index)),
        );
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

    axum::serve(listener, router)
        .await
        .map_err(|err| format!("server: {err}"))
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
            candidates.push(dir.join("../ui/dist").canonicalize().unwrap_or(dir.join("../ui/dist")));
        }
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui/dist"));
    candidates.into_iter().find(|path| path.join("index.html").exists())
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