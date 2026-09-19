use anyhow::{anyhow, bail, Context, Result};
use axum::{
    extract::{Json, Path as AxumPath, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post, put},
    Router,
};
use niripip_core::{
    config_path, daemon_socket_path, Config, DaemonRequest, DaemonResponse, DaemonResult,
    FollowMode, MinimizedWindowSnapshot, ResponseData, StatusSnapshot, DAEMON_PROTOCOL_VERSION,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    fs::OpenOptions,
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex as StdMutex},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, UnixStream},
    sync::watch,
};
use toml_edit::{value, DocumentMut, Item, Table};

const INDEX_HTML: &str = include_str!("../assets/index.html");
const APP_CSS: &str = include_str!("../assets/app.css");
const APP_JS: &str = include_str!("../assets/app.js");
const SETTINGS_TITLE: &str = "niri-pip — Settings";
const SETTINGS_WIDTH: &str = "1020";
const SETTINGS_HEIGHT: &str = "720";

#[derive(Clone)]
struct AppState {
    token: Arc<str>,
    last_seen: Arc<StdMutex<Instant>>,
    shutdown: watch::Sender<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UiSettings {
    auto_detect: bool,
    remember_geometry: bool,
    follow_workspace: bool,
    follow_mode: FollowMode,
    restore_layout_on_unpin: bool,
    prevent_focus_stealing: bool,
    preserve_aspect_ratio: bool,
    minimize_enabled: bool,
    minimize_restore_focus: bool,
    opacity_percent: u8,
}

#[derive(Debug, Clone, Serialize)]
struct IntegrationStatus {
    daemon_running: bool,
    daemon_version: Option<String>,
    niri_connected: bool,
    niri_version: Option<String>,
    autostart_enabled: bool,
    service_installed: bool,
    desktop_entry_installed: bool,
    inir_integrated: bool,
    minimized_count: usize,
    config_path: String,
}

#[derive(Debug, Clone, Serialize)]
struct Shortcut {
    name: &'static str,
    command: &'static str,
    suggested_bind: &'static str,
    installed: bool,
    conflict: bool,
}

#[derive(Debug, Serialize)]
struct Bootstrap {
    version: &'static str,
    language: String,
    settings: UiSettings,
    integration: IntegrationStatus,
    minimized: Vec<MinimizedWindowSnapshot>,
    shortcuts: Vec<Shortcut>,
}

#[derive(Debug, Deserialize)]
struct LanguageRequest {
    language: String,
}

#[derive(Debug, Deserialize)]
struct AutostartRequest {
    enabled: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct NiriUiWindow {
    id: u64,
    title: String,
    #[serde(default)]
    is_focused: bool,
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("niri-pip UI error: {err:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    if std::env::args().any(|arg| arg == "--help" || arg == "-h") {
        println!("niri-pip settings UI");
        println!();
        println!("Usage: niripip-ui [--no-open]");
        println!();
        println!("  --no-open   start the local settings server without opening a browser");
        return Ok(());
    }

    let no_open = std::env::args().any(|arg| arg == "--no-open");
    let token: Arc<str> = session_token()?.into();
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let state = AppState {
        token,
        last_seen: Arc::new(StdMutex::new(Instant::now())),
        shutdown: shutdown_tx,
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/app.css", get(css))
        .route("/app.js", get(js))
        .route("/api/bootstrap", get(api_bootstrap))
        .route("/api/ping", get(api_ping))
        .route("/api/language", put(api_language))
        .route("/api/settings", put(api_settings))
        .route("/api/autostart", post(api_autostart))
        .route("/api/restart-daemon", post(api_restart_daemon))
        .route("/api/open-config", post(api_open_config))
        .route("/api/reset-geometry", post(api_reset_geometry))
        .route("/api/restore-minimized", post(api_restore_minimized))
        .route("/api/restore-hidden/{id}", post(api_restore_hidden))
        .route(
            "/api/restore-all-minimized",
            post(api_restore_all_minimized),
        )
        .route("/api/shutdown", post(api_shutdown))
        .with_state(state.clone());

    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .context("binding local settings server")?;
    let addr = listener.local_addr()?;
    let url = format!("http://{addr}/");
    println!("UI_URL={url}");

    if !no_open {
        let app_mode = launch_browser(&url)?;
        if app_mode {
            tokio::spawn(arrange_settings_window());
        }
    }

    let idle_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            let elapsed = idle_state
                .last_seen
                .lock()
                .map(|seen| seen.elapsed())
                .unwrap_or(Duration::ZERO);
            if elapsed > Duration::from_secs(90) {
                let _ = idle_state.shutdown.send(true);
                break;
            }
        }
    });

    let shutdown = async move {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = shutdown_rx.changed() => {}
        }
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
        .context("serving local settings UI")?;
    Ok(())
}

async fn index(State(state): State<AppState>) -> Response {
    touch(&state);
    let html = INDEX_HTML
        .replace("__NIRIPIP_TOKEN__", state.token.as_ref())
        .replace("__NIRIPIP_VERSION__", env!("CARGO_PKG_VERSION"));
    secured(Html(html).into_response(), "text/html; charset=utf-8")
}

async fn css(State(state): State<AppState>) -> Response {
    touch(&state);
    secured(APP_CSS.into_response(), "text/css; charset=utf-8")
}

async fn js(State(state): State<AppState>) -> Response {
    touch(&state);
    secured(
        APP_JS.into_response(),
        "application/javascript; charset=utf-8",
    )
}

async fn api_bootstrap(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers) {
        return json_error(StatusCode::UNAUTHORIZED, "invalid UI session");
    }
    touch(&state);
    match build_bootstrap().await {
        Ok(data) => secured(Json(data).into_response(), "application/json"),
        Err(err) => json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    }
}

async fn api_ping(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers) {
        return json_error(StatusCode::UNAUTHORIZED, "invalid UI session");
    }
    touch(&state);
    secured(
        Json(serde_json::json!({"ok": true})).into_response(),
        "application/json",
    )
}

async fn api_language(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LanguageRequest>,
) -> Response {
    if !authorized(&state, &headers) {
        return json_error(StatusCode::UNAUTHORIZED, "invalid UI session");
    }
    touch(&state);

    match save_language(&request.language) {
        Ok(language) => secured(
            Json(serde_json::json!({"language": language})).into_response(),
            "application/json",
        ),
        Err(err) => json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    }
}

async fn api_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(settings): Json<UiSettings>,
) -> Response {
    if !authorized(&state, &headers) {
        return json_error(StatusCode::UNAUTHORIZED, "invalid UI session");
    }
    touch(&state);

    match save_settings(settings).await {
        Ok(saved) => secured(Json(saved).into_response(), "application/json"),
        Err(err) => json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    }
}

async fn api_autostart(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AutostartRequest>,
) -> Response {
    if !authorized(&state, &headers) {
        return json_error(StatusCode::UNAUTHORIZED, "invalid UI session");
    }
    touch(&state);

    let action = if request.enabled { "enable" } else { "disable" };
    match systemctl(&[action, "niripip.service"]) {
        Ok(_) => match integration_status().await {
            Ok(status) => secured(Json(status).into_response(), "application/json"),
            Err(err) => json_error(StatusCode::BAD_GATEWAY, &err.to_string()),
        },
        Err(err) => json_error(StatusCode::BAD_GATEWAY, &err.to_string()),
    }
}

async fn api_restart_daemon(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers) {
        return json_error(StatusCode::UNAUTHORIZED, "invalid UI session");
    }
    touch(&state);

    match systemctl(&["restart", "niripip.service"]) {
        Ok(_) => {
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline {
                if daemon_socket_path().is_some_and(|path| path.exists()) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            match integration_status().await {
                Ok(status) => secured(Json(status).into_response(), "application/json"),
                Err(err) => json_error(StatusCode::BAD_GATEWAY, &err.to_string()),
            }
        }
        Err(err) => json_error(StatusCode::BAD_GATEWAY, &err.to_string()),
    }
}

async fn api_open_config(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers) {
        return json_error(StatusCode::UNAUTHORIZED, "invalid UI session");
    }
    touch(&state);

    let path = config_path();
    if !path.exists() {
        if let Err(err) = write_default_config(&path) {
            return json_error(StatusCode::BAD_REQUEST, &err.to_string());
        }
    }

    match Command::new("xdg-open")
        .arg(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(_) => secured(
            Json(serde_json::json!({"ok": true})).into_response(),
            "application/json",
        ),
        Err(err) => json_error(
            StatusCode::BAD_GATEWAY,
            &format!("cannot open {}: {err}", path.display()),
        ),
    }
}

async fn api_reset_geometry(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers) {
        return json_error(StatusCode::UNAUTHORIZED, "invalid UI session");
    }
    touch(&state);

    match daemon_ok(DaemonRequest::ResetLearnedGeometry).await {
        Ok(_) => secured(
            Json(serde_json::json!({"ok": true})).into_response(),
            "application/json",
        ),
        Err(err) => json_error(StatusCode::BAD_GATEWAY, &err.to_string()),
    }
}

async fn api_restore_minimized(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers) {
        return json_error(StatusCode::UNAUTHORIZED, "invalid UI session");
    }
    touch(&state);

    match daemon_ok(DaemonRequest::RestoreMinimized { window_id: None }).await {
        Ok(_) => match build_bootstrap().await {
            Ok(data) => secured(Json(data).into_response(), "application/json"),
            Err(err) => json_error(StatusCode::BAD_GATEWAY, &err.to_string()),
        },
        Err(err) => json_error(StatusCode::BAD_GATEWAY, &err.to_string()),
    }
}

async fn api_restore_hidden(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<u64>,
    headers: HeaderMap,
) -> Response {
    if !authorized(&state, &headers) {
        return json_error(StatusCode::UNAUTHORIZED, "invalid UI session");
    }
    touch(&state);

    match daemon_ok(DaemonRequest::RestoreMinimized {
        window_id: Some(id),
    })
    .await
    {
        Ok(_) => match build_bootstrap().await {
            Ok(data) => secured(Json(data).into_response(), "application/json"),
            Err(err) => json_error(StatusCode::BAD_GATEWAY, &err.to_string()),
        },
        Err(err) => json_error(StatusCode::BAD_GATEWAY, &err.to_string()),
    }
}

async fn api_restore_all_minimized(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers) {
        return json_error(StatusCode::UNAUTHORIZED, "invalid UI session");
    }
    touch(&state);

    match daemon_ok(DaemonRequest::RestoreAllMinimized).await {
        Ok(_) => match build_bootstrap().await {
            Ok(data) => secured(Json(data).into_response(), "application/json"),
            Err(err) => json_error(StatusCode::BAD_GATEWAY, &err.to_string()),
        },
        Err(err) => json_error(StatusCode::BAD_GATEWAY, &err.to_string()),
    }
}

async fn api_shutdown(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers) {
        return json_error(StatusCode::UNAUTHORIZED, "invalid UI session");
    }
    let _ = state.shutdown.send(true);
    secured(
        Json(serde_json::json!({"ok": true})).into_response(),
        "application/json",
    )
}

async fn build_bootstrap() -> Result<Bootstrap> {
    Ok(Bootstrap {
        version: env!("CARGO_PKG_VERSION"),
        language: load_language(),
        settings: load_settings().await?,
        integration: integration_status().await?,
        minimized: minimized_windows().await,
        shortcuts: recommended_shortcuts(),
    })
}

async fn load_settings() -> Result<UiSettings> {
    let config = load_config()?;
    let opacity_percent = daemon_status()
        .await
        .and_then(|status| status.opacity_override_percent)
        .unwrap_or(100);

    Ok(UiSettings {
        auto_detect: config.general.auto_detect,
        remember_geometry: config.general.remember_geometry,
        follow_workspace: config.general.follow_workspace,
        follow_mode: config.general.follow_mode,
        restore_layout_on_unpin: config.general.restore_layout_on_unpin,
        prevent_focus_stealing: !config.pip.steal_focus,
        preserve_aspect_ratio: config.pip.preserve_aspect_ratio,
        minimize_enabled: config.minimize.enabled,
        minimize_restore_focus: config.minimize.restore_focus,
        opacity_percent,
    })
}

async fn save_settings(settings: UiSettings) -> Result<UiSettings> {
    if !(10..=100).contains(&settings.opacity_percent) {
        bail!("opacity must be between 10 and 100");
    }

    let path = config_path();
    let previous_bytes = fs::read(&path).ok();
    let previous_opacity = daemon_status()
        .await
        .and_then(|status| status.opacity_override_percent);

    write_settings_config(&path, &settings)?;

    if let Err(err) = daemon_ok(DaemonRequest::ReloadConfig).await {
        restore_file(&path, previous_bytes.as_deref())?;
        let _ = daemon_ok(DaemonRequest::ReloadConfig).await;
        return Err(err.context("daemon rejected configuration"));
    }

    if let Err(err) = daemon_ok(DaemonRequest::SetOpacity {
        percent: Some(settings.opacity_percent),
    })
    .await
    {
        restore_file(&path, previous_bytes.as_deref())?;
        let _ = daemon_ok(DaemonRequest::ReloadConfig).await;
        let _ = daemon_ok(DaemonRequest::SetOpacity {
            percent: previous_opacity,
        })
        .await;
        return Err(err.context("could not apply opacity"));
    }

    load_settings().await
}

async fn integration_status() -> Result<IntegrationStatus> {
    let daemon = daemon_status().await;
    let minimized_count = daemon.as_ref().map_or(0, |status| status.minimized);
    let service_installed = service_path_candidates().iter().any(|path| path.exists());
    let autostart_enabled = systemctl_status(&["is-enabled", "niripip.service"]);
    let daemon_running = systemctl_status(&["is-active", "niripip.service"])
        || daemon.as_ref().is_some_and(|status| status.daemon_running);

    Ok(IntegrationStatus {
        daemon_running,
        daemon_version: daemon.as_ref().map(|status| status.version.clone()),
        niri_connected: daemon.as_ref().is_some_and(|status| status.niri_connected),
        niri_version: daemon.and_then(|status| status.niri_version),
        autostart_enabled,
        service_installed,
        desktop_entry_installed: desktop_entry_path().exists(),
        inir_integrated: niri_integration_present(),
        minimized_count,
        config_path: config_path().display().to_string(),
    })
}

async fn minimized_windows() -> Vec<MinimizedWindowSnapshot> {
    match daemon_ok(DaemonRequest::ListMinimized).await {
        Ok(ResponseData::MinimizedWindows { windows }) => windows,
        _ => Vec::new(),
    }
}

async fn daemon_status() -> Option<StatusSnapshot> {
    match daemon_ok(DaemonRequest::Status).await {
        Ok(ResponseData::Status(status)) => Some(status),
        _ => None,
    }
}

async fn daemon_ok(request: DaemonRequest) -> Result<ResponseData> {
    let response = send_daemon(request).await?;
    if response.protocol_version != DAEMON_PROTOCOL_VERSION {
        bail!(
            "daemon protocol mismatch: UI expects {}, daemon returned {}",
            DAEMON_PROTOCOL_VERSION,
            response.protocol_version
        );
    }
    match response.result {
        DaemonResult::Ok { data } => Ok(data),
        DaemonResult::Error { message } => bail!("{message}"),
    }
}

async fn send_daemon(request: DaemonRequest) -> Result<DaemonResponse> {
    let path = daemon_socket_path().ok_or_else(|| anyhow!("XDG_RUNTIME_DIR is not set"))?;
    let mut stream = UnixStream::connect(&path).await.with_context(|| {
        format!(
            "cannot connect to {} (is niripip.service running?)",
            path.display()
        )
    })?;

    let mut payload = serde_json::to_vec(&request)?;
    payload.push(b'\n');
    stream.write_all(&payload).await?;
    stream.flush().await?;
    stream.shutdown().await?;

    let mut lines = BufReader::new(stream).lines();
    let line = lines
        .next_line()
        .await?
        .ok_or_else(|| anyhow!("daemon returned an empty response"))?;
    Ok(serde_json::from_str(&line)?)
}

fn language_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("niri-pip/ui-language")
}

fn load_language() -> String {
    fs::read_to_string(language_path())
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "auto" | "ru" | "en"))
        .unwrap_or_else(|| "auto".into())
}

fn save_language(language: &str) -> Result<String> {
    let language = language.trim().to_ascii_lowercase();
    if !matches!(language.as_str(), "auto" | "ru" | "en") {
        bail!("language must be auto, ru or en");
    }
    write_atomic(&language_path(), format!("{language}\n").as_bytes())?;
    Ok(language)
}

fn load_config() -> Result<Config> {
    let path = config_path();
    if path.exists() {
        Config::load(&path).map_err(Into::into)
    } else {
        Ok(Config::default())
    }
}

fn ensure_settings_table(doc: &mut DocumentMut, name: &str) {
    if doc.as_table().get(name).is_none() {
        doc.as_table_mut().insert(name, Item::Table(Table::new()));
    }
}

fn write_settings_config(path: &Path, settings: &UiSettings) -> Result<()> {
    let input = fs::read_to_string(path).unwrap_or_default();
    let mut doc = if input.trim().is_empty() {
        DocumentMut::new()
    } else {
        input
            .parse::<DocumentMut>()
            .context("parsing config.toml for settings update")?
    };

    ensure_settings_table(&mut doc, "general");
    ensure_settings_table(&mut doc, "pip");
    ensure_settings_table(&mut doc, "minimize");

    doc["general"]["auto_detect"] = value(settings.auto_detect);
    doc["general"]["remember_geometry"] = value(settings.remember_geometry);
    doc["general"]["follow_workspace"] = value(settings.follow_workspace);
    doc["general"]["follow_mode"] = value(follow_mode_name(settings.follow_mode));
    doc["general"]["restore_layout_on_unpin"] = value(settings.restore_layout_on_unpin);
    doc["pip"]["steal_focus"] = value(!settings.prevent_focus_stealing);
    doc["pip"]["preserve_aspect_ratio"] = value(settings.preserve_aspect_ratio);
    doc["minimize"]["enabled"] = value(settings.minimize_enabled);
    doc["minimize"]["restore_focus"] = value(settings.minimize_restore_focus);

    let candidate = doc.to_string();
    Config::from_toml(&candidate)?;
    write_atomic(path, candidate.as_bytes())
}

fn write_default_config(path: &Path) -> Result<()> {
    let text = r#"[general]
enabled = true
auto_detect = true
follow_workspace = true
follow_mode = "follow-workspace"
remember_geometry = true
restore_layout_on_unpin = true

[pip]
position = "bottom-right"
position_mode = "remember"
profile = "medium"
steal_focus = false
preserve_aspect_ratio = true

[minimize]
enabled = true
scratchpad_name = "niri-pip:scratchpad"
restore_focus = true
"#;
    Config::from_toml(text)?;
    write_atomic(path, text.as_bytes())
}

fn restore_file(path: &Path, previous: Option<&[u8]>) -> Result<()> {
    match previous {
        Some(bytes) => write_atomic(path, bytes),
        None => {
            if path.exists() {
                fs::remove_file(path)?;
            }
            Ok(())
        }
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("config path has no parent"))?;
    fs::create_dir_all(parent)?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;

    let tmp = path.with_extension("toml.ui.tmp");
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn follow_mode_name(mode: FollowMode) -> &'static str {
    match mode {
        FollowMode::FollowWorkspace => "follow-workspace",
        FollowMode::FollowFocusedOutput => "follow-focused-output",
        FollowMode::StayOnOutput => "stay-on-output",
    }
}

fn systemctl(args: &[&str]) -> Result<()> {
    let status = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .status()
        .with_context(|| format!("running systemctl --user {}", args.join(" ")))?;
    if !status.success() {
        bail!("systemctl --user {} failed", args.join(" "));
    }
    Ok(())
}

fn systemctl_status(args: &[&str]) -> bool {
    Command::new("systemctl")
        .arg("--user")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn service_path_candidates() -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from("/usr/lib/systemd/user/niripip.service"),
        PathBuf::from("/usr/local/lib/systemd/user/niripip.service"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        paths.push(PathBuf::from(home).join(".config/systemd/user/niripip.service"));
    }
    paths
}

fn desktop_entry_path() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("applications/niri-pip.desktop")
}

fn niri_integration_present() -> bool {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    [
        config_home.join("niri/config.d/90-user-extra.kdl"),
        config_home.join("niri/config.kdl"),
    ]
    .iter()
    .filter_map(|path| fs::read_to_string(path).ok())
    .any(|text| text.contains("niri-pip-runtime.kdl"))
}

fn niri_config_root() -> PathBuf {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    config_home.join("niri/config.kdl")
}

fn include_target(line: &str) -> Option<&str> {
    let line = line.trim();
    if line.starts_with("//") || line.starts_with("/-") || !line.starts_with("include ") {
        return None;
    }
    let start = line.find('"')? + 1;
    let end = line[start..].find('"')? + start;
    Some(&line[start..end])
}

fn collect_active_niri_config(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
    lines: &mut Vec<String>,
) {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if !visited.insert(canonical) {
        return;
    }
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    for line in text.lines() {
        lines.push(line.to_owned());
        if let Some(target) = include_target(line) {
            let included = PathBuf::from(target);
            let included = if included.is_absolute() {
                included
            } else {
                parent.join(included)
            };
            collect_active_niri_config(&included, visited, lines);
        }
    }
}

fn normalize_shortcut_bind(bind: &str) -> String {
    let mut parts: Vec<&str> = bind.split('+').filter(|part| !part.is_empty()).collect();
    let Some(key) = parts.pop() else {
        return bind.to_owned();
    };
    for part in &mut parts {
        if *part == "Mod" {
            *part = "Super";
        }
    }
    let rank = |part: &str| match part {
        "Super" => 0,
        "Ctrl" => 1,
        "Alt" => 2,
        "Shift" => 3,
        _ => 99,
    };
    parts.sort_by(|left, right| rank(left).cmp(&rank(right)).then_with(|| left.cmp(right)));
    parts.push(key);
    parts.join("+")
}

fn shortcut_status(bind: &str, commands: &[&str], lines: &[String]) -> (bool, bool) {
    let wanted = normalize_shortcut_bind(bind);
    let mut installed = false;
    let mut conflict = false;

    for raw in lines {
        let line = raw.trim();
        if line.starts_with("//") || line.starts_with("/-") {
            continue;
        }
        let Some(token) = line.split_whitespace().next() else {
            continue;
        };
        if normalize_shortcut_bind(token) != wanted {
            continue;
        }
        let ours = line.contains("niripip")
            && commands
                .iter()
                .any(|command| line.contains(&format!("\"{command}\"")));
        if ours {
            installed = true;
        } else {
            conflict = true;
        }
    }
    (installed, conflict)
}

fn recommended_shortcuts() -> Vec<Shortcut> {
    let mut lines = Vec::new();
    collect_active_niri_config(&niri_config_root(), &mut HashSet::new(), &mut lines);

    let make = |name, command, bind, accepted: &[&str]| {
        let (installed, conflict) = shortcut_status(bind, accepted, &lines);
        Shortcut {
            name,
            command,
            suggested_bind: bind,
            installed,
            conflict,
        }
    };

    vec![
        make("settings", "niripip ui", "Mod+Alt+P", &["ui"]),
        make("hide", "niripip hide", "Mod+Alt+M", &["hide", "minimize"]),
        make(
            "restore-hidden",
            "niripip restore-hidden",
            "Mod+Alt+Shift+M",
            &["restore-hidden", "restore-minimized"],
        ),
        make("toggle", "niripip toggle", "Mod+Alt+T", &["toggle"]),
        make("peek", "niripip peek", "Mod+Alt+Space", &["peek"]),
        make("restore", "niripip unpin", "Mod+Alt+U", &["unpin"]),
    ]
}

fn authorized(state: &AppState, headers: &HeaderMap) -> bool {
    headers
        .get("x-niripip-token")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == state.token.as_ref())
}

fn touch(state: &AppState) {
    if let Ok(mut seen) = state.last_seen.lock() {
        *seen = Instant::now();
    }
}

fn secured(mut response: Response, content_type: &'static str) -> Response {
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; base-uri 'none'; frame-ancestors 'none'; object-src 'none'; img-src 'self' data:; style-src 'self'; script-src 'self'; connect-src 'self'",
        ),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}

fn json_error(status: StatusCode, message: &str) -> Response {
    secured(
        (status, Json(serde_json::json!({"error": message}))).into_response(),
        "application/json",
    )
}

fn session_token() -> Result<String> {
    let mut bytes = [0_u8; 24];
    fs::File::open("/dev/urandom")
        .context("opening /dev/urandom")?
        .read_exact(&mut bytes)
        .context("reading UI session token")?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn launch_browser(url: &str) -> Result<bool> {
    let app = format!("--app={url}");

    let flatpak_chrome = Command::new("flatpak")
        .args(["info", "com.google.Chrome"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    let flatpak_started = flatpak_chrome
        && Command::new("flatpak")
            .args([
                "run",
                "com.google.Chrome",
                "--user-data-dir=/tmp/niri-pip-ui-profile",
                "--no-first-run",
                "--no-default-browser-check",
                &app,
                "--class=niri-pip-settings",
                "--window-size=1020,720",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok();
    if flatpak_started {
        return Ok(true);
    }

    for browser in [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
    ] {
        if Command::new(browser)
            .args([
                "--user-data-dir=/tmp/niri-pip-ui-profile",
                "--no-first-run",
                "--no-default-browser-check",
                &app,
                "--class=niri-pip-settings",
                "--window-size=1020,720",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok()
        {
            return Ok(true);
        }
    }

    Command::new("xdg-open")
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("no Chrome/Chromium app-mode browser or xdg-open was available")?;
    Ok(false)
}

fn settings_window_id(windows: &[NiriUiWindow]) -> Option<u64> {
    windows
        .iter()
        .find(|window| window.title == SETTINGS_TITLE && window.is_focused)
        .or_else(|| windows.iter().find(|window| window.title == SETTINGS_TITLE))
        .map(|window| window.id)
}

fn find_settings_window_id() -> Option<u64> {
    let output = Command::new("niri")
        .args(["msg", "--json", "windows"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let windows: Vec<NiriUiWindow> = serde_json::from_slice(&output.stdout).ok()?;
    settings_window_id(&windows)
}

fn run_niri_window_action(action: &str, argument: Option<&str>, id: u64) -> bool {
    let id = id.to_string();
    let mut command = Command::new("niri");
    command.args(["msg", "action", action]);
    if let Some(argument) = argument {
        command.arg(argument);
    }
    command
        .args(["--id", &id])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

async fn arrange_settings_window() {
    for _ in 0..40 {
        if let Some(id) = find_settings_window_id() {
            let floating = run_niri_window_action("move-window-to-floating", None, id);
            let width = run_niri_window_action("set-window-width", Some(SETTINGS_WIDTH), id);
            let height = run_niri_window_action("set-window-height", Some(SETTINGS_HEIGHT), id);
            if !(floating && width && height) {
                eprintln!(
                    "niri-pip UI warning: could not apply preferred settings window geometry"
                );
            }
            return;
        }
        tokio::time::sleep(Duration::from_millis(75)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_config(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("niripip-ui-{name}-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir.join("config.toml")
    }

    fn settings() -> UiSettings {
        UiSettings {
            auto_detect: true,
            remember_geometry: true,
            follow_workspace: true,
            follow_mode: FollowMode::FollowWorkspace,
            restore_layout_on_unpin: true,
            prevent_focus_stealing: true,
            preserve_aspect_ratio: true,
            minimize_enabled: true,
            minimize_restore_focus: true,
            opacity_percent: 95,
        }
    }

    #[test]
    fn shortcut_detection_normalizes_modifiers_and_detects_conflicts() {
        let lines = vec![
            "Super+Alt+M { maximize-window-to-edges; }".to_string(),
            "Mod+Shift+Alt+M repeat=false { spawn \"/usr/bin/niripip\" \"restore-hidden\"; }"
                .to_string(),
        ];

        assert_eq!(
            shortcut_status("Mod+Alt+M", &["hide", "minimize"], &lines),
            (false, true)
        );
        assert_eq!(
            shortcut_status(
                "Mod+Alt+Shift+M",
                &["restore-hidden", "restore-minimized"],
                &lines
            ),
            (true, false)
        );
    }

    #[test]
    fn include_target_ignores_comments_and_reads_optional_include() {
        assert_eq!(
            include_target(r#"include optional=true "config.d/90-user-extra.kdl""#),
            Some("config.d/90-user-extra.kdl")
        );
        assert_eq!(include_target(r#"// include "ignored.kdl""#), None);
    }

    #[test]
    fn settings_writer_preserves_unrelated_sections() {
        let path = temp_config("preserve");
        fs::write(
            &path,
            "[general]\nenabled = false\n\n[logging]\nlevel = \"debug\"\n",
        )
        .unwrap();

        let mut changed = settings();
        changed.minimize_enabled = false;
        changed.minimize_restore_focus = false;
        write_settings_config(&path, &changed).unwrap();

        let full = Config::load(&path).unwrap();
        assert!(!full.general.enabled);
        assert_eq!(full.logging.level, "debug");
        assert!(full.general.remember_geometry);
        assert!(!full.pip.steal_focus);
        assert!(!full.minimize.enabled);
        assert!(!full.minimize.restore_focus);
        assert_eq!(full.minimize.scratchpad_name, "niri-pip:scratchpad");

        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("[minimize]"));
        assert!(raw.contains("enabled = false"));
        assert!(raw.contains("restore_focus = false"));

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn settings_writer_produces_valid_config() {
        let path = temp_config("invalid");
        let original = "[general]\nenabled = true\n";
        fs::write(&path, original).unwrap();

        let mut invalid = settings();
        invalid.follow_mode = FollowMode::FollowWorkspace;
        write_settings_config(&path, &invalid).unwrap();
        assert!(Config::load(&path).is_ok());

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn settings_window_selection_prefers_focused_exact_title() {
        let windows = vec![
            NiriUiWindow {
                id: 10,
                title: SETTINGS_TITLE.into(),
                is_focused: false,
            },
            NiriUiWindow {
                id: 20,
                title: "Other window".into(),
                is_focused: true,
            },
            NiriUiWindow {
                id: 30,
                title: SETTINGS_TITLE.into(),
                is_focused: true,
            },
        ];

        assert_eq!(settings_window_id(&windows), Some(30));
    }

    #[test]
    fn settings_window_selection_falls_back_to_exact_title() {
        let windows = vec![
            NiriUiWindow {
                id: 10,
                title: SETTINGS_TITLE.into(),
                is_focused: false,
            },
            NiriUiWindow {
                id: 20,
                title: "Other window".into(),
                is_focused: true,
            },
        ];

        assert_eq!(settings_window_id(&windows), Some(10));
    }

    #[test]
    fn session_tokens_are_random_hex() {
        let first = session_token().unwrap();
        let second = session_token().unwrap();
        assert_eq!(first.len(), 48);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_ne!(first, second);
    }
}
