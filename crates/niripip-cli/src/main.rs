use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use niripip_core::{
    config_path, daemon_socket_path, runtime_kdl_path, Config, ControlPreset, DaemonRequest,
    DaemonResponse, DaemonResult, FollowMode, Placement, ResponseData, StatusSnapshot,
    TrackedWindowSnapshot, DAEMON_PROTOCOL_VERSION,
};
use niripip_ipc::{NiriBackend, RealNiriBackend};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

#[derive(Debug, Parser)]
#[command(
    name = "niripip",
    version,
    about = "Sticky Picture-in-Picture and pinned-window controller for Niri"
)]
struct Cli {
    /// Print machine-readable JSON where supported.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Pin a window (focused window by default).
    Pin {
        #[arg(long)]
        window_id: Option<u64>,
    },
    /// Stop pinning a window.
    Unpin {
        #[arg(long)]
        window_id: Option<u64>,
    },
    /// Toggle pinning for a window.
    Toggle {
        #[arg(long)]
        window_id: Option<u64>,
    },
    /// Set an exact width and height. Manual values are remembered for auto PiP.
    #[command(alias = "resize")]
    Size {
        width: u32,
        height: u32,
        #[arg(long)]
        window_id: Option<u64>,
    },
    /// Scale the current size by a percentage, for example +10 or -10.
    Scale {
        #[arg(allow_hyphen_values = true)]
        percent: i32,
        #[arg(long)]
        window_id: Option<u64>,
    },
    /// Move a tracked window to a named position.
    Position {
        placement: PlacementArg,
        #[arg(long)]
        window_id: Option<u64>,
    },
    /// Move a floating window by logical pixels without stealing focus.
    Nudge {
        #[arg(allow_hyphen_values = true)]
        dx: i32,
        #[arg(allow_hyphen_values = true)]
        dy: i32,
        #[arg(long)]
        window_id: Option<u64>,
    },
    /// Enable or disable workspace following for the selected tracked window.
    Follow {
        state: OnOff,
        #[arg(long)]
        window_id: Option<u64>,
    },
    /// Change the multi-monitor/workspace follow policy.
    FollowMode {
        mode: FollowModeArg,
        #[arg(long)]
        window_id: Option<u64>,
    },
    /// Freeze the current tracked geometry. Manual drag/resize snaps back while locked.
    Lock {
        #[arg(long)]
        window_id: Option<u64>,
    },
    /// Allow manual resizing and dragging again.
    Unlock {
        #[arg(long)]
        window_id: Option<u64>,
    },
    /// Forget remembered PiP geometry/control overrides and return to config defaults.
    Reset {
        #[arg(long)]
        window_id: Option<u64>,
    },
    /// Set PiP opacity to 10-100, or `auto` to inherit normal Niri/iNiR rules.
    Opacity { value: String },
    /// Apply a ready-made controller preset.
    Preset {
        preset: PresetArg,
        #[arg(long)]
        window_id: Option<u64>,
    },
    /// Control the active MPRIS media player through playerctl (best effort).
    Media {
        #[command(subcommand)]
        action: MediaAction,
        /// Optional playerctl player name, e.g. chromium.instance123.
        #[arg(long)]
        player: Option<String>,
    },
    /// Open the iNiR/fuzzel compact controller menu.
    Menu,
    /// List tracked PiP and manually pinned windows.
    List,
    /// Show daemon and tracked-window status.
    Status,
    /// Daemon lifecycle/status helpers.
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },
    /// Diagnose Niri IPC, config, runtime rules, daemon and systemd integration.
    Doctor,
    /// Ask the running daemon to reload config.toml.
    Reload,
    /// Enable management actions in the running daemon.
    Enable,
    /// Disable management actions in the running daemon.
    Disable,
}

#[derive(Debug, Subcommand)]
enum DaemonCommand {
    /// Check whether niripipd is reachable.
    Status,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum PlacementArg {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

impl From<PlacementArg> for Placement {
    fn from(value: PlacementArg) -> Self {
        match value {
            PlacementArg::TopLeft => Placement::TopLeft,
            PlacementArg::TopRight => Placement::TopRight,
            PlacementArg::BottomLeft => Placement::BottomLeft,
            PlacementArg::BottomRight => Placement::BottomRight,
            PlacementArg::Center => Placement::Center,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FollowModeArg {
    FollowWorkspace,
    FollowFocusedOutput,
    StayOnOutput,
}

impl From<FollowModeArg> for FollowMode {
    fn from(value: FollowModeArg) -> Self {
        match value {
            FollowModeArg::FollowWorkspace => FollowMode::FollowWorkspace,
            FollowModeArg::FollowFocusedOutput => FollowMode::FollowFocusedOutput,
            FollowModeArg::StayOnOutput => FollowMode::StayOnOutput,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OnOff {
    On,
    Off,
}

impl OnOff {
    fn enabled(self) -> bool {
        matches!(self, Self::On)
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum PresetArg {
    Tiny,
    Small,
    Medium,
    Large,
    Cinema,
    Movie,
    Study,
}

impl From<PresetArg> for ControlPreset {
    fn from(value: PresetArg) -> Self {
        match value {
            PresetArg::Tiny => ControlPreset::Tiny,
            PresetArg::Small => ControlPreset::Small,
            PresetArg::Medium => ControlPreset::Medium,
            PresetArg::Large => ControlPreset::Large,
            PresetArg::Cinema => ControlPreset::Cinema,
            PresetArg::Movie => ControlPreset::Movie,
            PresetArg::Study => ControlPreset::Study,
        }
    }
}

#[derive(Debug, Subcommand)]
enum MediaAction {
    Play,
    Pause,
    PlayPause,
    Next,
    Previous,
    Forward {
        #[arg(default_value_t = 10)]
        seconds: u32,
    },
    Back {
        #[arg(default_value_t = 10)]
        seconds: u32,
    },
    VolumeUp {
        #[arg(default_value_t = 5)]
        percent: u8,
    },
    VolumeDown {
        #[arg(default_value_t = 5)]
        percent: u8,
    },
    Status,
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Pin { window_id } => print_response(
            send_daemon(DaemonRequest::Pin { window_id }).await?,
            cli.json,
        ),
        Command::Unpin { window_id } => print_response(
            send_daemon(DaemonRequest::Unpin { window_id }).await?,
            cli.json,
        ),
        Command::Toggle { window_id } => print_response(
            send_daemon(DaemonRequest::Toggle { window_id }).await?,
            cli.json,
        ),
        Command::Size {
            width,
            height,
            window_id,
        } => print_response(
            send_daemon(DaemonRequest::Resize {
                window_id,
                width,
                height,
            })
            .await?,
            cli.json,
        ),
        Command::Scale { percent, window_id } => print_response(
            send_daemon(DaemonRequest::Scale { window_id, percent }).await?,
            cli.json,
        ),
        Command::Position {
            placement,
            window_id,
        } => print_response(
            send_daemon(DaemonRequest::SetPosition {
                window_id,
                placement: placement.into(),
            })
            .await?,
            cli.json,
        ),
        Command::Nudge { dx, dy, window_id } => print_response(
            send_daemon(DaemonRequest::Nudge { window_id, dx, dy }).await?,
            cli.json,
        ),
        Command::Follow { state, window_id } => print_response(
            send_daemon(DaemonRequest::SetFollow {
                window_id,
                enabled: state.enabled(),
            })
            .await?,
            cli.json,
        ),
        Command::FollowMode { mode, window_id } => print_response(
            send_daemon(DaemonRequest::SetFollowMode {
                window_id,
                mode: mode.into(),
            })
            .await?,
            cli.json,
        ),
        Command::Lock { window_id } => print_response(
            send_daemon(DaemonRequest::SetGeometryLock {
                window_id,
                locked: true,
            })
            .await?,
            cli.json,
        ),
        Command::Unlock { window_id } => print_response(
            send_daemon(DaemonRequest::SetGeometryLock {
                window_id,
                locked: false,
            })
            .await?,
            cli.json,
        ),
        Command::Reset { window_id } => print_response(
            send_daemon(DaemonRequest::ResetGeometry { window_id }).await?,
            cli.json,
        ),
        Command::Opacity { value } => {
            let percent = parse_opacity(&value)?;
            print_response(
                send_daemon(DaemonRequest::SetOpacity { percent }).await?,
                cli.json,
            );
        }
        Command::Preset { preset, window_id } => print_response(
            send_daemon(DaemonRequest::ApplyPreset {
                window_id,
                preset: preset.into(),
            })
            .await?,
            cli.json,
        ),
        Command::Media { action, player } => media_command(action, player.as_deref())?,
        Command::Menu => launch_menu()?,
        Command::List => print_response(send_daemon(DaemonRequest::List).await?, cli.json),
        Command::Status => print_response(send_daemon(DaemonRequest::Status).await?, cli.json),
        Command::Reload => {
            print_response(send_daemon(DaemonRequest::ReloadConfig).await?, cli.json)
        }
        Command::Enable => print_response(
            send_daemon(DaemonRequest::SetEnabled { enabled: true }).await?,
            cli.json,
        ),
        Command::Disable => print_response(
            send_daemon(DaemonRequest::SetEnabled { enabled: false }).await?,
            cli.json,
        ),
        Command::Daemon {
            command: DaemonCommand::Status,
        } => match send_daemon(DaemonRequest::Status).await {
            Ok(response) => print_response(response, cli.json),
            Err(err) => return Err(anyhow!("niripipd is not reachable: {err}")),
        },
        Command::Doctor => doctor(cli.json).await?,
    }
    Ok(())
}

fn parse_opacity(value: &str) -> Result<Option<u8>> {
    if value.eq_ignore_ascii_case("auto") || value.eq_ignore_ascii_case("inherit") {
        return Ok(None);
    }
    let value = value.trim_end_matches('%');
    let percent: u8 = value
        .parse()
        .with_context(|| format!("invalid opacity '{value}'; use 10-100 or auto"))?;
    if !(10..=100).contains(&percent) {
        bail!("opacity must be between 10 and 100 percent, or auto");
    }
    Ok(Some(percent))
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
    let response: DaemonResponse = serde_json::from_str(&line)?;
    if response.protocol_version != DAEMON_PROTOCOL_VERSION {
        return Err(anyhow!(
            "daemon protocol mismatch: CLI expects {}, daemon returned {}",
            DAEMON_PROTOCOL_VERSION,
            response.protocol_version
        ));
    }
    Ok(response)
}

fn print_response(response: DaemonResponse, json: bool) {
    if json {
        let failed = matches!(&response.result, DaemonResult::Error { .. });
        match serde_json::to_string_pretty(&response) {
            Ok(output) => println!("{output}"),
            Err(err) => {
                eprintln!("error: cannot serialize daemon response: {err}");
                std::process::exit(1);
            }
        }
        if failed {
            std::process::exit(2);
        }
        return;
    }
    match response.result {
        DaemonResult::Error { message } => {
            eprintln!("error: {message}");
            std::process::exit(2);
        }
        DaemonResult::Ok { data } => match data {
            ResponseData::Message { message } => println!("{message}"),
            ResponseData::Windows { windows } => print_windows(&windows),
            ResponseData::Status(status) => print_status(&status),
        },
    }
}

fn print_status(status: &StatusSnapshot) {
    println!("niri-pip {}\n", status.version);
    println!(
        "Daemon        {}",
        if status.daemon_running {
            "running"
        } else {
            "stopped"
        }
    );
    println!(
        "Niri IPC      {}{}",
        if status.niri_connected {
            "connected"
        } else {
            "disconnected"
        },
        status
            .niri_version
            .as_deref()
            .map(|version| format!(" ({version})"))
            .unwrap_or_default()
    );
    println!("Enabled       {}", status.enabled);
    println!("Tracked       {}", status.tracked);
    println!("Pinned        {}", status.pinned);
    println!(
        "PiP opacity   {}",
        status
            .opacity_override_percent
            .map(|value| format!("{value}%"))
            .unwrap_or_else(|| "auto/inherit".into())
    );
    if let Some(workspace) = status.focused_workspace {
        println!("Workspace     #{workspace}");
    }
    if !status.windows.is_empty() {
        println!("\nWINDOWS");
        print_windows(&status.windows);
    }
}

fn print_windows(windows: &[TrackedWindowSnapshot]) {
    if windows.is_empty() {
        println!("No tracked windows.");
        return;
    }
    for window in windows {
        let title = if window.title.is_empty() {
            "<untitled>"
        } else {
            &window.title
        };
        println!("  #{}  {}", window.id, title);
        println!(
            "       {}x{}  {:?}  {}",
            window.width, window.height, window.placement, window.mode
        );
        println!(
            "       workspace {}  follow={} ({:?})  lock={}  app-id={}{}",
            window
                .workspace_id
                .map(|value| value.to_string())
                .unwrap_or_else(|| "?".into()),
            if window.follow_enabled { "on" } else { "off" },
            window.follow_mode,
            if window.geometry_locked { "on" } else { "off" },
            if window.app_id.is_empty() {
                "<empty>"
            } else {
                &window.app_id
            },
            window
                .detector
                .as_deref()
                .map(|detector| format!("  detector={detector}"))
                .unwrap_or_default(),
        );
    }
}

fn media_command(action: MediaAction, player: Option<&str>) -> Result<()> {
    let mut command = ProcessCommand::new("playerctl");
    if let Some(player) = player {
        command.arg("--player").arg(player);
    }
    match action {
        MediaAction::Play => {
            command.arg("play");
        }
        MediaAction::Pause => {
            command.arg("pause");
        }
        MediaAction::PlayPause => {
            command.arg("play-pause");
        }
        MediaAction::Next => {
            command.arg("next");
        }
        MediaAction::Previous => {
            command.arg("previous");
        }
        MediaAction::Forward { seconds } => {
            command.arg("position").arg(format!("{seconds}+"));
        }
        MediaAction::Back { seconds } => {
            command.arg("position").arg(format!("{seconds}-"));
        }
        MediaAction::VolumeUp { percent } => {
            command
                .arg("volume")
                .arg(format!("{:.2}+", percent as f64 / 100.0));
        }
        MediaAction::VolumeDown { percent } => {
            command
                .arg("volume")
                .arg(format!("{:.2}-", percent as f64 / 100.0));
        }
        MediaAction::Status => {
            command
                .arg("metadata")
                .arg("--format")
                .arg("{{playerName}}  {{status}}  {{artist}} - {{title}}");
        }
    }
    let status = command
        .status()
        .context("cannot run playerctl; install playerctl or use iNiR's audio package")?;
    if !status.success() {
        bail!("playerctl command failed (no controllable MPRIS player may be available)");
    }
    Ok(())
}

fn launch_menu() -> Result<()> {
    let status = ProcessCommand::new("niripip-menu").status().context(
        "cannot launch niripip-menu; reinstall niri-pip so the iNiR integration is installed",
    )?;
    if !status.success() {
        bail!("niripip-menu exited with {status}");
    }
    Ok(())
}

#[derive(Debug)]
struct DoctorCheck {
    name: &'static str,
    ok: bool,
    detail: String,
}

async fn doctor(json: bool) -> Result<()> {
    let mut checks = Vec::new();

    let socket_env = std::env::var_os("NIRI_SOCKET");
    checks.push(DoctorCheck {
        name: "NIRI_SOCKET exists",
        ok: socket_env
            .as_ref()
            .is_some_and(|path| Path::new(path).exists()),
        detail: socket_env
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|| "environment variable is missing".into()),
    });

    let backend = RealNiriBackend::from_env();
    let mut niri_version = None;
    match &backend {
        Ok(backend) => match backend.version().await {
            Ok(version) => {
                niri_version = Some(version.clone());
                checks.push(DoctorCheck {
                    name: "IPC connection",
                    ok: true,
                    detail: version,
                });
            }
            Err(err) => checks.push(DoctorCheck {
                name: "IPC connection",
                ok: false,
                detail: err.to_string(),
            }),
        },
        Err(err) => checks.push(DoctorCheck {
            name: "IPC connection",
            ok: false,
            detail: err.to_string(),
        }),
    }

    let supported = niri_version.as_deref().is_some_and(version_supported);
    checks.push(DoctorCheck {
        name: "Niri version supported",
        ok: supported,
        detail: niri_version.clone().unwrap_or_else(|| {
            "unknown; niri-pip supports Niri >= 26.04; 26.04 is the verified baseline".into()
        }),
    });

    let cfg_path = config_path();
    match Config::load(&cfg_path) {
        Ok(_) => checks.push(DoctorCheck {
            name: "config valid",
            ok: true,
            detail: cfg_path.display().to_string(),
        }),
        Err(niripip_core::ConfigError::Read { source, .. })
            if source.kind() == std::io::ErrorKind::NotFound =>
        {
            checks.push(DoctorCheck {
                name: "config valid",
                ok: true,
                detail: "config missing; built-in defaults are valid".into(),
            });
        }
        Err(err) => checks.push(DoctorCheck {
            name: "config valid",
            ok: false,
            detail: err.to_string(),
        }),
    }

    if let Ok(backend) = &backend {
        match tokio::time::timeout(Duration::from_secs(2), backend.subscribe()).await {
            Ok(Ok(_receiver)) => checks.push(DoctorCheck {
                name: "event stream available",
                ok: true,
                detail: "EventStream handshake accepted".into(),
            }),
            Ok(Err(err)) => checks.push(DoctorCheck {
                name: "event stream available",
                ok: false,
                detail: err.to_string(),
            }),
            Err(_) => checks.push(DoctorCheck {
                name: "event stream available",
                ok: false,
                detail: "timed out after 2s".into(),
            }),
        }
    } else {
        checks.push(DoctorCheck {
            name: "event stream available",
            ok: false,
            detail: "Niri backend unavailable".into(),
        });
    }

    let daemon_check = match send_daemon(DaemonRequest::Status).await {
        Ok(response) => match response.result {
            DaemonResult::Ok { .. } => DoctorCheck {
                name: "daemon socket",
                ok: true,
                detail: "niripipd responded".into(),
            },
            DaemonResult::Error { message } => DoctorCheck {
                name: "daemon socket",
                ok: false,
                detail: message,
            },
        },
        Err(err) => DoctorCheck {
            name: "daemon socket",
            ok: false,
            detail: err.to_string(),
        },
    };
    checks.push(daemon_check);

    let service_active = ProcessCommand::new("systemctl")
        .args(["--user", "is-active", "--quiet", "niripip.service"])
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    checks.push(DoctorCheck {
        name: "systemd service running",
        ok: service_active,
        detail: if service_active {
            "niripip.service active".into()
        } else {
            "niripip.service is not active (or systemd --user is unavailable)".into()
        },
    });

    let runtime_path = runtime_kdl_path();
    checks.push(DoctorCheck {
        name: "runtime rule file",
        ok: runtime_path.exists(),
        detail: runtime_path.display().to_string(),
    });

    let include_locations = niri_integration_candidates();
    let include_found = include_locations.iter().find(|path| {
        std::fs::read_to_string(path)
            .map(|text| text.contains("niri-pip-runtime.kdl"))
            .unwrap_or(false)
    });
    checks.push(DoctorCheck {
        name: "runtime rule included",
        ok: include_found.is_some(),
        detail: include_found
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "run scripts/setup-niri-integration.sh or reinstall".into()),
    });

    checks.push(DoctorCheck {
        name: "floating actions supported",
        ok: supported,
        detail: "verified action schema: MoveWindowToFloating, SetWindowWidth/Height, MoveFloatingWindow, MoveWindowToWorkspace(focus=false)".into(),
    });

    let playerctl_available = ProcessCommand::new("playerctl")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    checks.push(DoctorCheck {
        name: "media controls (optional)",
        ok: true,
        detail: if playerctl_available {
            "playerctl available".into()
        } else {
            "playerctl not found; window control still works".into()
        },
    });

    let problems = checks.iter().filter(|check| !check.ok).count();
    if json {
        let value = serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "problems": problems,
            "checks": checks.iter().map(|check| serde_json::json!({
                "name": check.name,
                "ok": check.ok,
                "detail": &check.detail
            })).collect::<Vec<_>>()
        });
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("niri-pip {} doctor\n", env!("CARGO_PKG_VERSION"));
        for check in &checks {
            println!(
                "{} {:<28} {}",
                if check.ok { "✓" } else { "✗" },
                check.name,
                check.detail
            );
        }
        println!(
            "\n{} problem{} found",
            problems,
            if problems == 1 { "" } else { "s" }
        );
    }
    if problems > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn niri_integration_candidates() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    vec![
        config_home.join("niri/config.d/90-user-extra.kdl"),
        config_home.join("niri/config.kdl"),
    ]
}

fn version_supported(version: &str) -> bool {
    let token = version
        .split_whitespace()
        .find(|token| token.chars().next().is_some_and(|c| c.is_ascii_digit()));
    let Some(token) = token else {
        return false;
    };
    let mut parts = token.split('.');
    let Some(year) = parts.next().and_then(|value| value.parse::<u32>().ok()) else {
        return false;
    };
    let Some(month) = parts.next().and_then(|value| value.parse::<u32>().ok()) else {
        return false;
    };
    (year, month) >= (26, 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_current_niri_version() {
        assert!(version_supported("26.04"));
        assert!(version_supported("niri 26.04 (unknown commit)"));
        assert!(version_supported("26.05"));
    }

    #[test]
    fn rejects_old_or_unknown_version() {
        assert!(!version_supported("0.1.10"));
        assert!(!version_supported("development"));
    }

    #[test]
    fn parses_opacity_auto_and_percentage() {
        assert_eq!(parse_opacity("auto").unwrap(), None);
        assert_eq!(parse_opacity("80%").unwrap(), Some(80));
        assert!(parse_opacity("5").is_err());
    }

    #[test]
    fn clap_accepts_negative_scale_and_nudge_values() {
        assert!(Cli::try_parse_from(["niripip", "scale", "-10"]).is_ok());
        assert!(Cli::try_parse_from(["niripip", "nudge", "-20", "0"]).is_ok());
        assert!(Cli::try_parse_from(["niripip", "nudge", "0", "-50"]).is_ok());
    }
}
