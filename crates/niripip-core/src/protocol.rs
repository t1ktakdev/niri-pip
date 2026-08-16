use crate::{FollowMode, Placement};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DAEMON_PROTOCOL_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "kebab-case")]
pub enum DaemonRequest {
    Status,
    List,
    Pin {
        window_id: Option<u64>,
    },
    Unpin {
        window_id: Option<u64>,
    },
    Toggle {
        window_id: Option<u64>,
    },
    Resize {
        window_id: Option<u64>,
        width: u32,
        height: u32,
    },
    Scale {
        window_id: Option<u64>,
        percent: i32,
    },
    SetPosition {
        window_id: Option<u64>,
        placement: Placement,
    },
    Nudge {
        window_id: Option<u64>,
        dx: i32,
        dy: i32,
    },
    SetFollow {
        window_id: Option<u64>,
        enabled: bool,
    },
    SetFollowMode {
        window_id: Option<u64>,
        mode: FollowMode,
    },
    SetGeometryLock {
        window_id: Option<u64>,
        locked: bool,
    },
    ResetGeometry {
        window_id: Option<u64>,
    },
    SetOpacity {
        percent: Option<u8>,
    },
    ApplyPreset {
        window_id: Option<u64>,
        preset: ControlPreset,
    },
    ReloadConfig,
    SetEnabled {
        enabled: bool,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ControlPreset {
    Tiny,
    Small,
    Medium,
    Large,
    Cinema,
    Movie,
    Study,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonResponse {
    pub protocol_version: u32,
    #[serde(flatten)]
    pub result: DaemonResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum DaemonResult {
    Ok { data: ResponseData },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ResponseData {
    Status(StatusSnapshot),
    Windows { windows: Vec<TrackedWindowSnapshot> },
    Message { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusSnapshot {
    pub version: String,
    pub daemon_running: bool,
    pub niri_connected: bool,
    pub niri_version: Option<String>,
    pub enabled: bool,
    pub tracked: usize,
    pub pinned: usize,
    pub focused_workspace: Option<u64>,
    pub opacity_override_percent: Option<u8>,
    pub windows: Vec<TrackedWindowSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedWindowSnapshot {
    pub id: u64,
    pub title: String,
    pub app_id: String,
    pub mode: String,
    pub detector: Option<String>,
    pub score: Option<i32>,
    pub workspace_id: Option<u64>,
    pub width: u32,
    pub height: u32,
    pub placement: Placement,
    pub follow_enabled: bool,
    pub follow_mode: FollowMode,
    pub geometry_locked: bool,
}

pub fn daemon_socket_path() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .map(|runtime| runtime.join("niri-pip/niripip.sock"))
}

pub fn runtime_kdl_path() -> PathBuf {
    if let Some(base) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(base).join("niri/niri-pip-runtime.kdl");
    }
    let home = std::env::var_os("HOME").unwrap_or_else(|| ".".into());
    PathBuf::from(home).join(".config/niri/niri-pip-runtime.kdl")
}
