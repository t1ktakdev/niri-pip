use anyhow::{anyhow, Context, Result};
use niripip_core::{
    config_path, daemon_socket_path, runtime_kdl_path, state_path, CompositorEvent, Config,
    DaemonRequest, DaemonResponse, DaemonResult, Effect, Engine, PersistentState, ResponseData,
    DAEMON_PROTOCOL_VERSION,
};
use niripip_ipc::{NiriBackend, RealNiriBackend};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tokio::time::MissedTickBehavior;
use tracing::{debug, info, trace, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    if unsafe { libc::geteuid() } == 0 {
        return Err(anyhow!("niripipd must not run as root"));
    }

    let cfg_path = config_path();
    let config = load_config_or_default(&cfg_path)?;
    init_logging(&config.logging.level);

    let persistent_path = state_path();
    let mut persistent = PersistentState::load(&persistent_path)
        .with_context(|| format!("loading {}", persistent_path.display()))?;
    let niri_session = current_niri_session_key()?;
    let cleared_minimized = normalize_persistent_niri_session(&mut persistent, &niri_session);
    if cleared_minimized > 0 {
        info!(
            cleared = cleared_minimized,
            "discarded minimized window IDs from a previous Niri session"
        );
    }
    // Persist the normalized schema/session immediately so migrations and stale-ID cleanup are
    // durable even if the daemon stops before the first geometry/controller/minimize event.
    persistent
        .save_atomic(&persistent_path)
        .with_context(|| format!("normalizing {}", persistent_path.display()))?;
    let engine = Arc::new(Mutex::new(Engine::new(config, persistent)?));

    let runtime_path = runtime_kdl_path();
    let initial_opacity = engine.lock().await.opacity_override_percent();
    write_runtime_kdl(&runtime_path, initial_opacity)
        .with_context(|| format!("writing runtime Niri rules to {}", runtime_path.display()))?;

    let real_backend = RealNiriBackend::from_env().context(
        "Niri IPC is unavailable. If this is a systemd service, start Niri with niri-session/--session so NIRI_SOCKET is imported into the user manager",
    )?;
    let niri_socket_path = real_backend.socket_path().to_path_buf();
    let backend: Arc<dyn NiriBackend> = Arc::new(real_backend);

    let socket_path = daemon_socket_path().ok_or_else(|| anyhow!("XDG_RUNTIME_DIR is not set"))?;
    let listener = bind_daemon_socket(&socket_path)?;
    info!(
        version = env!("CARGO_PKG_VERSION"),
        socket = %socket_path.display(),
        "niripipd started"
    );

    let mut cli_task = tokio::spawn(run_cli_server(
        listener,
        engine.clone(),
        backend.clone(),
        cfg_path.clone(),
        persistent_path.clone(),
        runtime_path.clone(),
    ));
    let mut niri_task = tokio::spawn(run_niri_supervisor(
        engine.clone(),
        backend.clone(),
        persistent_path.clone(),
        niri_socket_path,
    ));

    let outcome: Result<()> = tokio::select! {
        result = &mut cli_task => match result {
            Ok(inner) => inner,
            Err(err) => Err(err).context("CLI server task panicked"),
        },
        result = &mut niri_task => match result {
            Ok(inner) => inner,
            Err(err) => Err(err).context("Niri supervisor task panicked"),
        },
        result = shutdown_signal() => {
            if result.is_ok() {
                info!("shutdown signal received");
            }
            result
        },
    };

    cli_task.abort();
    niri_task.abort();

    if let Err(err) = persist_if_dirty(&engine, &persistent_path).await {
        warn!(error = %err, "failed to persist runtime state during shutdown");
    }
    let _ = fs::remove_file(&socket_path);
    outcome
}

#[cfg(unix)]
async fn shutdown_signal() -> Result<()> {
    use tokio::signal::unix::{signal, SignalKind};

    let mut terminate = signal(SignalKind::terminate()).context("installing SIGTERM handler")?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => result.context("installing SIGINT handler")?,
        _ = terminate.recv() => {}
    }
    Ok(())
}

fn init_logging(level: &str) {
    let default = format!("niripip_daemon={level},niripip_ipc={level},niripip_core={level}");
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .compact()
        .init();
}

fn current_niri_session_key() -> Result<String> {
    let socket = std::env::var_os("NIRI_SOCKET")
        .ok_or_else(|| anyhow!("NIRI_SOCKET is not set; start niri-pip inside a Niri session"))?;
    let boot_id =
        fs::read_to_string("/proc/sys/kernel/random/boot_id").context("reading Linux boot ID")?;
    Ok(format!(
        "{}|{}",
        boot_id.trim(),
        PathBuf::from(socket).display()
    ))
}

fn normalize_persistent_niri_session(state: &mut PersistentState, current: &str) -> usize {
    if state.niri_session.as_deref() == Some(current) {
        return 0;
    }

    let cleared = state.minimized.len();
    state.minimized.clear();
    state.niri_session = Some(current.to_owned());
    cleared
}

fn load_config_or_default(path: &Path) -> Result<Config> {
    match Config::load(path) {
        Ok(cfg) => Ok(cfg),
        Err(niripip_core::ConfigError::Read { source, .. })
            if source.kind() == std::io::ErrorKind::NotFound =>
        {
            Ok(Config::default())
        }
        Err(err) => Err(err.into()),
    }
}

fn bind_daemon_socket(path: &Path) -> Result<UnixListener> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("invalid daemon socket path"))?;
    fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;

    if path.exists() {
        if std::os::unix::net::UnixStream::connect(path).is_ok() {
            return Err(anyhow!(
                "another niripipd instance is already listening at {}",
                path.display()
            ));
        }
        fs::remove_file(path)
            .with_context(|| format!("removing stale socket {}", path.display()))?;
    }
    let listener =
        UnixListener::bind(path).with_context(|| format!("binding {}", path.display()))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

async fn run_cli_server(
    listener: UnixListener,
    engine: Arc<Mutex<Engine>>,
    backend: Arc<dyn NiriBackend>,
    cfg_path: PathBuf,
    persistent_path: PathBuf,
    runtime_path: PathBuf,
) -> Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        let engine = engine.clone();
        let backend = backend.clone();
        let cfg_path = cfg_path.clone();
        let persistent_path = persistent_path.clone();
        let runtime_path = runtime_path.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_cli_connection(
                stream,
                engine,
                backend,
                cfg_path,
                persistent_path,
                runtime_path,
            )
            .await
            {
                warn!(error = %err, "CLI request failed");
            }
        });
    }
}

async fn handle_cli_connection(
    stream: UnixStream,
    engine: Arc<Mutex<Engine>>,
    backend: Arc<dyn NiriBackend>,
    cfg_path: PathBuf,
    persistent_path: PathBuf,
    runtime_path: PathBuf,
) -> Result<()> {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();
    let count = reader.read_line(&mut line).await?;
    if count == 0 {
        return Ok(());
    }
    if line.len() > 64 * 1024 {
        return Err(anyhow!("daemon request exceeds 64 KiB"));
    }

    let request: DaemonRequest = serde_json::from_str(line.trim_end())?;
    let response = process_daemon_request(
        request,
        &engine,
        backend.as_ref(),
        &cfg_path,
        &persistent_path,
        &runtime_path,
    )
    .await;
    let mut encoded = serde_json::to_vec(&response)?;
    encoded.push(b'\n');
    write_half.write_all(&encoded).await?;
    write_half.shutdown().await?;
    Ok(())
}

async fn process_daemon_request(
    request: DaemonRequest,
    engine: &Arc<Mutex<Engine>>,
    backend: &dyn NiriBackend,
    cfg_path: &Path,
    persistent_path: &Path,
    runtime_path: &Path,
) -> DaemonResponse {
    let result: Result<ResponseData> = async {
        match request {
            DaemonRequest::Status => {
                let snapshot = engine.lock().await.status_snapshot();
                Ok(ResponseData::Status(snapshot))
            }
            DaemonRequest::List => {
                let windows = engine.lock().await.tracked_snapshots();
                Ok(ResponseData::Windows { windows })
            }
            DaemonRequest::ListMinimized => {
                let windows = engine.lock().await.minimized_snapshots();
                Ok(ResponseData::MinimizedWindows { windows })
            }
            DaemonRequest::Minimize { window_id } => {
                let application = engine.lock().await.minimize(window_id)?;
                if let Err(err) = execute_effects(backend, application.effects.clone()).await {
                    let compensation = engine.lock().await.compensate_minimize(&application);
                    execute_effects_best_effort(backend, compensation).await;
                    return Err(err);
                }
                if !engine.lock().await.commit_minimize(&application) {
                    let compensation = engine.lock().await.compensate_minimize(&application);
                    execute_effects_best_effort(backend, compensation).await;
                    return Err(anyhow!(
                        "window #{} closed before minimize state could be committed",
                        application.window_id
                    ));
                }
                Ok(ResponseData::Message {
                    message: "window minimized to niri-pip scratchpad".into(),
                })
            }
            DaemonRequest::RestoreMinimized { window_id } => {
                let application = engine.lock().await.restore_minimized(window_id)?;
                if let Err(err) = execute_effects(backend, application.effects.clone()).await {
                    let compensation = engine.lock().await.compensate_restore(&application);
                    execute_effects_best_effort(backend, compensation).await;
                    return Err(err);
                }
                engine.lock().await.commit_restore(&application);
                Ok(ResponseData::Message {
                    message: "minimized window restored".into(),
                })
            }
            DaemonRequest::RestoreAllMinimized => {
                let application = engine.lock().await.restore_all_minimized()?;
                if let Err(err) = execute_effects(backend, application.effects.clone()).await {
                    let compensation = engine.lock().await.compensate_restore(&application);
                    execute_effects_best_effort(backend, compensation).await;
                    return Err(err);
                }
                engine.lock().await.commit_restore(&application);
                Ok(ResponseData::Message {
                    message: "all minimized windows restored".into(),
                })
            }
            DaemonRequest::Pin { window_id } => {
                let effects = engine.lock().await.pin(window_id)?;
                execute_effects(backend, effects).await?;
                Ok(ResponseData::Message {
                    message: "window pinned".into(),
                })
            }
            DaemonRequest::Overlay { window_id, profile } => {
                let effects = engine.lock().await.overlay(window_id, profile.as_deref())?;
                execute_effects(backend, effects).await?;
                Ok(ResponseData::Message {
                    message: "window converted to overlay".into(),
                })
            }
            DaemonRequest::Unpin { window_id } => {
                let effects = engine.lock().await.unpin(window_id)?;
                execute_effects(backend, effects).await?;
                Ok(ResponseData::Message {
                    message: "window unpinned".into(),
                })
            }
            DaemonRequest::Toggle { window_id } => {
                let effects = engine.lock().await.toggle(window_id)?;
                execute_effects(backend, effects).await?;
                Ok(ResponseData::Message {
                    message: "window pin state toggled".into(),
                })
            }
            DaemonRequest::SetPeek { window_id, enabled } => {
                let effects = engine.lock().await.set_peek(window_id, enabled)?;
                execute_effects(backend, effects).await?;
                Ok(ResponseData::Message {
                    message: match enabled {
                        Some(true) => "peek enabled".into(),
                        Some(false) => "peek disabled".into(),
                        None => "peek toggled".into(),
                    },
                })
            }
            DaemonRequest::Resize {
                window_id,
                width,
                height,
            } => {
                let effects = engine.lock().await.resize(window_id, width, height)?;
                execute_effects(backend, effects).await?;
                Ok(ResponseData::Message {
                    message: format!("window resized to {width}x{height}"),
                })
            }
            DaemonRequest::Scale { window_id, percent } => {
                let effects = engine.lock().await.scale(window_id, percent)?;
                execute_effects(backend, effects).await?;
                Ok(ResponseData::Message {
                    message: format!("window scaled by {percent:+}%"),
                })
            }
            DaemonRequest::SetPosition {
                window_id,
                placement,
            } => {
                let effects = engine.lock().await.set_position(window_id, placement)?;
                execute_effects(backend, effects).await?;
                Ok(ResponseData::Message {
                    message: format!("window position set to {placement:?}"),
                })
            }
            DaemonRequest::Nudge { window_id, dx, dy } => {
                let effects = engine.lock().await.nudge(window_id, dx, dy)?;
                execute_effects(backend, effects).await?;
                Ok(ResponseData::Message {
                    message: format!("window moved by x={dx}, y={dy}"),
                })
            }
            DaemonRequest::SetFollow { window_id, enabled } => {
                let effects = engine.lock().await.set_follow(window_id, enabled)?;
                execute_effects(backend, effects).await?;
                Ok(ResponseData::Message {
                    message: format!(
                        "workspace follow {}",
                        if enabled { "enabled" } else { "disabled" }
                    ),
                })
            }
            DaemonRequest::SetFollowMode { window_id, mode } => {
                let effects = engine.lock().await.set_follow_mode(window_id, mode)?;
                execute_effects(backend, effects).await?;
                Ok(ResponseData::Message {
                    message: format!("follow mode set to {mode:?}"),
                })
            }
            DaemonRequest::SetGeometryLock { window_id, locked } => {
                let effects = engine.lock().await.set_geometry_lock(window_id, locked)?;
                execute_effects(backend, effects).await?;
                Ok(ResponseData::Message {
                    message: if locked {
                        "geometry locked".into()
                    } else {
                        "geometry unlocked".into()
                    },
                })
            }
            DaemonRequest::ResetGeometry { window_id } => {
                let effects = engine.lock().await.reset_geometry(window_id)?;
                execute_effects(backend, effects).await?;
                Ok(ResponseData::Message {
                    message: "window controls reset to configured defaults".into(),
                })
            }
            DaemonRequest::ResetLearnedGeometry => {
                engine.lock().await.reset_learned_geometry();
                Ok(ResponseData::Message {
                    message: "remembered PiP geometry reset".into(),
                })
            }
            DaemonRequest::SetOpacity { percent } => {
                let old = engine.lock().await.opacity_override_percent();
                engine.lock().await.set_opacity_override_percent(percent)?;
                if let Err(err) = write_runtime_kdl(runtime_path, percent) {
                    let _ = engine.lock().await.set_opacity_override_percent(old);
                    return Err(err);
                }
                Ok(ResponseData::Message {
                    message: match percent {
                        Some(percent) => format!("PiP opacity set to {percent}%"),
                        None => "PiP opacity set to auto/inherit".into(),
                    },
                })
            }
            DaemonRequest::ApplyPreset { window_id, preset } => {
                let old_opacity = engine.lock().await.opacity_override_percent();
                let application = engine.lock().await.apply_preset(window_id, preset)?;
                execute_effects(backend, application.effects).await?;
                if let Err(err) = write_runtime_kdl(runtime_path, application.opacity_percent) {
                    let _ = engine
                        .lock()
                        .await
                        .set_opacity_override_percent(old_opacity);
                    return Err(err);
                }
                Ok(ResponseData::Message {
                    message: format!("preset {preset:?} applied"),
                })
            }
            DaemonRequest::ReloadConfig => {
                let cfg = load_config_or_default(cfg_path)?;
                engine.lock().await.replace_config(cfg)?;
                Ok(ResponseData::Message {
                    message: "configuration reloaded".into(),
                })
            }
            DaemonRequest::SetEnabled { enabled } => {
                engine.lock().await.set_enabled(enabled);
                Ok(ResponseData::Message {
                    message: format!("niri-pip {}", if enabled { "enabled" } else { "disabled" }),
                })
            }
        }
    }
    .await;

    if let Err(err) = persist_if_dirty(engine, persistent_path).await {
        warn!(error = %err, "failed to persist runtime state after CLI request");
    }

    DaemonResponse {
        protocol_version: DAEMON_PROTOCOL_VERSION,
        result: match result {
            Ok(data) => DaemonResult::Ok { data },
            Err(err) => DaemonResult::Error {
                message: err.to_string(),
            },
        },
    }
}

fn write_runtime_kdl(path: &Path, opacity_percent: Option<u8>) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("invalid runtime KDL path"))?;
    fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;

    let body = match opacity_percent {
        Some(percent) => {
            let opacity = percent as f64 / 100.0;
            format!(
                "// Generated by niri-pip. Do not edit while niripipd is running.\n\
                 // The include is installed separately and can be removed without touching iNiR core files.\n\n\
                 window-rule {{\n\
                     match title=r#\"(?i)^picture(?:[ -]?in[ -]?)picture$\"#\n\
                     opacity {opacity:.2}\n\
                 }}\n"
            )
        }
        None => "// Generated by niri-pip. PiP opacity mode: auto/inherit.\n".to_string(),
    };

    let tmp = path.with_extension("kdl.tmp");
    let mut file = fs::File::create(&tmp)
        .with_context(|| format!("creating temporary runtime config {}", tmp.display()))?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    file.write_all(body.as_bytes())?;
    file.sync_all()?;
    fs::rename(&tmp, path)
        .with_context(|| format!("replacing runtime config {}", path.display()))?;
    Ok(())
}

async fn run_niri_supervisor(
    engine: Arc<Mutex<Engine>>,
    backend: Arc<dyn NiriBackend>,
    persistent_path: PathBuf,
    niri_socket_path: PathBuf,
) -> Result<()> {
    let follow_generation = Arc::new(AtomicU64::new(0));
    let mut backoff = Duration::from_millis(250);

    loop {
        match run_niri_connection(
            engine.clone(),
            backend.clone(),
            persistent_path.clone(),
            follow_generation.clone(),
        )
        .await
        {
            Ok(()) => warn!("Niri event stream ended; reconnecting"),
            Err(err) => warn!(error = %err, "Niri IPC disconnected; reconnecting"),
        }
        engine
            .lock()
            .await
            .handle_event(CompositorEvent::Disconnected {
                reason: "IPC disconnected".into(),
            });

        if !niri_socket_owner_is_present(&niri_socket_path) {
            return Err(anyhow!(
                "Niri IPC socket {} is gone or its PID-scoped owner exited; exiting so the service can restart with a fresh NIRI_SOCKET",
                niri_socket_path.display()
            ));
        }

        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(15));
        if backend.version().await.is_ok() {
            backoff = Duration::from_millis(250);
        }
    }
}

async fn run_niri_connection(
    engine: Arc<Mutex<Engine>>,
    backend: Arc<dyn NiriBackend>,
    persistent_path: PathBuf,
    follow_generation: Arc<AtomicU64>,
) -> Result<()> {
    let version = backend.version().await?;
    let outputs = backend.outputs().await?;
    let mut events = backend.subscribe().await?;

    {
        let mut state = engine.lock().await;
        state.handle_event(CompositorEvent::Connected {
            version: version.clone(),
        });
        let effects = state.handle_event(CompositorEvent::OutputsChanged(outputs));
        drop(state);
        execute_effects_best_effort(backend.as_ref(), effects).await;
    }
    info!(niri_version = %version, "connected to Niri IPC");
    let mut last_output_refresh = Instant::now() - Duration::from_secs(2);

    let mut persist_tick = tokio::time::interval(Duration::from_secs(1));
    persist_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    persist_tick.tick().await;

    loop {
        tokio::select! {
            maybe_event = events.recv() => {
                let Some(event) = maybe_event else {
                    if let Err(err) = persist_if_dirty(&engine, &persistent_path).await {
                        warn!(error = %err, "failed to persist runtime state at stream end");
                    }
                    return Err(anyhow!("Niri event stream channel closed"));
                };
                let event = match event {
                    Ok(event) => event,
                    Err(err) => {
                        if let Err(persist_err) = persist_if_dirty(&engine, &persistent_path).await {
                            warn!(error = %persist_err, "failed to persist runtime state after IPC error");
                        }
                        return Err(err.into());
                    }
                };

                let is_workspace_activation = matches!(
                    &event,
                    CompositorEvent::WorkspaceActivated { focused: true, .. }
                );
                let output_refresh_hint = matches!(
                    &event,
                    CompositorEvent::WorkspacesChanged(_)
                        | CompositorEvent::WindowLayoutsChanged(_)
                );
                let refresh_outputs = output_refresh_hint
                    && last_output_refresh.elapsed() >= Duration::from_secs(2);
                let effects = engine.lock().await.handle_event(event);
                execute_effects_best_effort(backend.as_ref(), effects).await;

                if refresh_outputs {
                    last_output_refresh = Instant::now();
                    match backend.outputs().await {
                        Ok(outputs) => {
                            let effects = engine
                                .lock()
                                .await
                                .handle_event(CompositorEvent::OutputsChanged(outputs));
                            execute_effects_best_effort(backend.as_ref(), effects).await;
                        }
                        Err(err) => {
                            debug!(
                                error = %err,
                                "output refresh after workspace topology change failed"
                            );
                        }
                    }
                }

                if is_workspace_activation {
                    let generation = follow_generation.fetch_add(1, Ordering::SeqCst) + 1;
                    let engine = engine.clone();
                    let backend = backend.clone();
                    let generation_counter = follow_generation.clone();
                    let debounce_ms = engine.lock().await.config().general.workspace_debounce_ms;
                    tokio::spawn(async move {
                        tokio::time::sleep(Duration::from_millis(debounce_ms)).await;
                        if generation_counter.load(Ordering::SeqCst) != generation {
                            trace!("discarding stale workspace-follow debounce");
                            return;
                        }
                        let effects = engine.lock().await.reconcile_workspace_follow();
                        execute_effects_best_effort(backend.as_ref(), effects).await;
                    });
                }
            }
            _ = persist_tick.tick() => {
                if let Err(err) = persist_if_dirty(&engine, &persistent_path).await {
                    warn!(error = %err, "failed to persist runtime state");
                }
            }
        }
    }
}

fn niri_socket_pid(path: &Path) -> Option<u32> {
    let file_name = path.file_name()?.to_str()?;
    let stem = file_name.strip_suffix(".sock")?;
    let (_, pid) = stem.rsplit_once('.')?;
    pid.parse().ok()
}

fn niri_socket_owner_is_present(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    let Some(pid) = niri_socket_pid(path) else {
        return true;
    };
    Path::new("/proc").join(pid.to_string()).exists()
}

async fn execute_effects(backend: &dyn NiriBackend, effects: Vec<Effect>) -> Result<()> {
    for effect in effects {
        let Effect::Action(action) = effect;
        backend.execute(action).await?;
    }
    Ok(())
}

async fn execute_effects_best_effort(backend: &dyn NiriBackend, effects: Vec<Effect>) {
    for effect in effects {
        let Effect::Action(action) = effect;
        if let Err(err) = backend.execute(action).await {
            warn!(error = %err, "Niri reconciliation action failed");
        }
    }
}

async fn persist_if_dirty(engine: &Arc<Mutex<Engine>>, path: &Path) -> Result<()> {
    let state = engine.lock().await.take_persistent_if_dirty();
    if let Some(state) = state {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || state.save_atomic(&path))
            .await
            .context("state writer panicked")??;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use niripip_core::{
        CompositorAction, LogicalOutput, MinimizedWindowState, OutputInfo, WindowInfo,
        WindowLayout, WorkspaceInfo,
    };
    use niripip_ipc::mock::MockNiriBackend;
    use std::collections::HashMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn minimized_ids_are_scoped_to_the_niri_session() {
        let mut state = PersistentState {
            niri_session: Some("old-session".into()),
            ..PersistentState::default()
        };
        state.minimized.push(MinimizedWindowState {
            window_id: 42,
            title: "old window".into(),
            app_id: "org.example.App".into(),
            origin_workspace_id: Some(1),
            was_floating: false,
            size: Some((800, 600)),
            geometry: None,
        });

        assert_eq!(
            normalize_persistent_niri_session(&mut state, "new-session"),
            1
        );
        assert!(state.minimized.is_empty());
        assert_eq!(state.niri_session.as_deref(), Some("new-session"));

        state.minimized.push(MinimizedWindowState {
            window_id: 43,
            title: "same-session window".into(),
            app_id: "org.example.App".into(),
            origin_workspace_id: Some(1),
            was_floating: true,
            size: Some((640, 420)),
            geometry: None,
        });
        assert_eq!(
            normalize_persistent_niri_session(&mut state, "new-session"),
            0
        );
        assert_eq!(state.minimized.len(), 1);
    }

    #[test]
    fn parses_pid_scoped_niri_socket_name() {
        assert_eq!(
            niri_socket_pid(Path::new("/run/user/1000/niri.wayland-1.4242.sock")),
            Some(4242)
        );
        assert_eq!(niri_socket_pid(Path::new("/run/user/1000/niri.sock")), None);
    }

    #[test]
    fn runtime_kdl_supports_fixed_and_auto_opacity() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("niripip-runtime-{nonce}"));
        let path = dir.join("runtime.kdl");
        write_runtime_kdl(&path, Some(80)).expect("write fixed opacity");
        let fixed = fs::read_to_string(&path).expect("read fixed opacity");
        assert!(fixed.contains("opacity 0.80"));

        write_runtime_kdl(&path, None).expect("write auto opacity");
        let auto = fs::read_to_string(&path).expect("read auto opacity");
        assert!(!auto.contains("window-rule"));
        let _ = fs::remove_dir_all(dir);
    }

    fn transaction_test_engine() -> Engine {
        let mut engine = Engine::new(Config::default(), PersistentState::default())
            .expect("create transaction test engine");
        engine.handle_event(CompositorEvent::OutputsChanged(HashMap::from([(
            "eDP-1".into(),
            OutputInfo {
                name: "eDP-1".into(),
                logical: Some(LogicalOutput {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                    scale: 1.0,
                }),
            },
        )])));
        engine.handle_event(CompositorEvent::WorkspacesChanged(vec![
            WorkspaceInfo {
                id: 1,
                idx: 1,
                output: Some("eDP-1".into()),
                is_active: true,
                is_focused: true,
                active_window_id: Some(50),
                ..Default::default()
            },
            WorkspaceInfo {
                id: 2,
                idx: 2,
                output: Some("eDP-1".into()),
                ..Default::default()
            },
        ]));
        engine.handle_event(CompositorEvent::WindowOpenedOrChanged(WindowInfo {
            id: 50,
            title: Some("transaction-test".into()),
            app_id: Some("Alacritty".into()),
            workspace_id: Some(1),
            is_focused: true,
            is_floating: false,
            layout: WindowLayout {
                window_size: (900, 700),
                tile_size: (900.0, 700.0),
                ..Default::default()
            },
            ..Default::default()
        }));
        engine
    }

    fn transaction_test_paths(name: &str) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("niripip-{name}-{nonce}"));
        (
            dir.clone(),
            dir.join("config.toml"),
            dir.join("state.json"),
            dir.join("runtime.kdl"),
        )
    }

    #[tokio::test]
    async fn failed_minimize_does_not_commit_state_and_compensates() {
        let engine = Arc::new(Mutex::new(transaction_test_engine()));
        let backend = MockNiriBackend::default().fail_on_action(2);
        let (dir, cfg, state, runtime) = transaction_test_paths("failed-minimize");

        let response = process_daemon_request(
            DaemonRequest::Minimize {
                window_id: Some(50),
            },
            &engine,
            &backend,
            &cfg,
            &state,
            &runtime,
        )
        .await;

        assert!(matches!(response.result, DaemonResult::Error { .. }));
        assert!(engine.lock().await.minimized_snapshots().is_empty());

        let actions = backend.actions();
        assert!(matches!(
            actions.first(),
            Some(CompositorAction::SetWorkspaceName {
                workspace_id: 2,
                ..
            })
        ));
        assert!(actions.iter().any(|action| matches!(
            action,
            CompositorAction::MoveWindowToWorkspace {
                window_id: 50,
                workspace_id: 1,
                focus: false
            }
        )));
        assert!(actions.iter().any(|action| matches!(
            action,
            CompositorAction::UnsetWorkspaceName { name }
                if name == "niri-pip:scratchpad"
        )));
        let _ = fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn failed_restore_keeps_state_and_compensates_back_to_scratchpad() {
        let mut prepared = transaction_test_engine();
        let minimize = prepared.minimize(Some(50)).expect("prepare minimize");
        prepared.commit_minimize(&minimize);
        prepared.handle_event(CompositorEvent::WorkspacesChanged(vec![
            WorkspaceInfo {
                id: 1,
                idx: 1,
                output: Some("eDP-1".into()),
                is_active: true,
                is_focused: true,
                ..Default::default()
            },
            WorkspaceInfo {
                id: 2,
                idx: 2,
                name: Some("niri-pip:scratchpad".into()),
                output: Some("eDP-1".into()),
                active_window_id: Some(50),
                ..Default::default()
            },
        ]));
        prepared.handle_event(CompositorEvent::WindowOpenedOrChanged(WindowInfo {
            id: 50,
            title: Some("transaction-test".into()),
            app_id: Some("Alacritty".into()),
            workspace_id: Some(2),
            is_focused: false,
            is_floating: false,
            layout: WindowLayout {
                window_size: (900, 700),
                tile_size: (900.0, 700.0),
                ..Default::default()
            },
            ..Default::default()
        }));
        let _ = prepared.take_persistent_if_dirty();

        let engine = Arc::new(Mutex::new(prepared));
        let backend = MockNiriBackend::default().fail_on_action(2);
        let (dir, cfg, state, runtime) = transaction_test_paths("failed-restore");

        let response = process_daemon_request(
            DaemonRequest::RestoreMinimized {
                window_id: Some(50),
            },
            &engine,
            &backend,
            &cfg,
            &state,
            &runtime,
        )
        .await;

        assert!(matches!(response.result, DaemonResult::Error { .. }));
        assert_eq!(engine.lock().await.minimized_snapshots().len(), 1);

        let actions = backend.actions();
        assert!(matches!(
            actions.first(),
            Some(CompositorAction::MoveWindowToWorkspace {
                window_id: 50,
                workspace_id: 1,
                focus: false
            })
        ));
        assert!(actions.iter().any(|action| matches!(
            action,
            CompositorAction::SetWorkspaceName {
                workspace_id: 2,
                name
            } if name == "niri-pip:scratchpad"
        )));
        assert!(actions.iter().any(|action| matches!(
            action,
            CompositorAction::MoveWindowToWorkspaceName {
                window_id: 50,
                workspace_name,
                focus: false
            } if workspace_name == "niri-pip:scratchpad"
        )));
        let _ = fs::remove_dir_all(dir);
    }
}
