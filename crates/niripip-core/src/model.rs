use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WindowLayout {
    #[serde(default)]
    pub pos_in_scrolling_layout: Option<(usize, usize)>,
    #[serde(default)]
    pub tile_size: (f64, f64),
    #[serde(default)]
    pub window_size: (i32, i32),
    #[serde(default)]
    pub tile_pos_in_workspace_view: Option<(f64, f64)>,
    #[serde(default)]
    pub window_offset_in_tile: (f64, f64),
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: u64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub app_id: Option<String>,
    #[serde(default)]
    pub pid: Option<i32>,
    #[serde(default)]
    pub workspace_id: Option<u64>,
    #[serde(default)]
    pub is_focused: bool,
    #[serde(default)]
    pub is_floating: bool,
    #[serde(default)]
    pub is_urgent: bool,
    #[serde(default)]
    pub layout: WindowLayout,
}

impl WindowInfo {
    pub fn title(&self) -> &str {
        self.title.as_deref().unwrap_or("")
    }

    pub fn app_id(&self) -> &str {
        self.app_id.as_deref().unwrap_or("")
    }

    pub fn logical_size(&self) -> (u32, u32) {
        let (w, h) = self.layout.window_size;
        (w.max(0) as u32, h.max(0) as u32)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub id: u64,
    #[serde(default)]
    pub idx: u8,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub is_urgent: bool,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub is_focused: bool,
    #[serde(default)]
    pub active_window_id: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OutputInfo {
    pub name: String,
    #[serde(default)]
    pub logical: Option<LogicalOutput>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct LogicalOutput {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompositorEvent {
    Connected {
        version: String,
    },
    Disconnected {
        reason: String,
    },
    WorkspacesChanged(Vec<WorkspaceInfo>),
    WorkspaceActivated {
        id: u64,
        focused: bool,
    },
    WorkspaceActiveWindowChanged {
        workspace_id: u64,
        active_window_id: Option<u64>,
    },
    WindowsChanged(Vec<WindowInfo>),
    WindowOpenedOrChanged(WindowInfo),
    WindowClosed {
        id: u64,
    },
    WindowFocusChanged {
        id: Option<u64>,
    },
    WindowLayoutsChanged(Vec<(u64, WindowLayout)>),
    OutputsChanged(HashMap<String, OutputInfo>),
    Unknown(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PositionChange {
    SetFixed(f64),
    SetProportion(f64),
    AdjustFixed(f64),
    AdjustProportion(f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SizeChange {
    SetFixed(i32),
    SetProportion(f64),
    AdjustFixed(i32),
    AdjustProportion(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompositorAction {
    FocusWindow {
        id: u64,
    },
    MoveWindowToFloating {
        id: u64,
    },
    MoveWindowToTiling {
        id: u64,
    },
    SetWindowWidth {
        id: u64,
        change: SizeChange,
    },
    SetWindowHeight {
        id: u64,
        change: SizeChange,
    },
    MoveFloatingWindow {
        id: u64,
        x: PositionChange,
        y: PositionChange,
    },
    MoveWindowToWorkspace {
        window_id: u64,
        workspace_id: u64,
        focus: bool,
    },
}
