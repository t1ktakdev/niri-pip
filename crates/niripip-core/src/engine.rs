use crate::{
    corner_placement, CompositorAction, CompositorEvent, Config, ControlPreset, DetectionAction,
    DetectorEngine, DetectorError, FollowMode, OutputInfo, PersistentState, Placement,
    PlacementPlan, PositionChange, PositionMode, RememberedControls, RememberedGeometry,
    SizeChange, StatusSnapshot, TrackedWindowSnapshot, WindowInfo, WindowLayout, WorkspaceInfo,
    STATE_SCHEMA_VERSION,
};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error(transparent)]
    Detector(#[from] DetectorError),
    #[error("window #{0} does not exist")]
    WindowNotFound(u64),
    #[error("no focused window")]
    NoFocusedWindow,
    #[error("no tracked PiP/pinned window")]
    NoTrackedWindow,
    #[error("more than one tracked window matches; pass --window-id")]
    AmbiguousTrackedWindow,
    #[error("window #{0} is not pinned")]
    NotPinned(u64),
    #[error("niri-pip is disabled")]
    Disabled,
    #[error("invalid size {0}x{1}; minimum is 120x68 and each dimension must fit Niri IPC i32")]
    InvalidSize(u32, u32),
    #[error("scale must keep the window at least 120x68")]
    InvalidScale,
    #[error("opacity must be between 10 and 100 percent, or auto")]
    InvalidOpacity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackedMode {
    AutoPip,
    ManualPin,
}

#[derive(Debug)]
struct PendingWorkspaceMove {
    target: u64,
    issued_at: Instant,
}

#[derive(Debug)]
struct TrackedWindow {
    mode: TrackedMode,
    detector: Option<String>,
    score: Option<i32>,
    follow_enabled: bool,
    follow_mode: FollowMode,
    original_was_floating: bool,
    placement: Placement,
    managed_size: Option<(u32, u32)>,
    pending_workspace: Option<PendingWorkspaceMove>,
    suppress_geometry_until: Instant,
    geometry_locked: bool,
    locked_geometry: Option<RememberedGeometry>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    Action(CompositorAction),
}

#[derive(Debug)]
pub struct PresetApplication {
    pub effects: Vec<Effect>,
    pub opacity_percent: Option<u8>,
}

#[derive(Debug)]
pub struct Engine {
    config: Config,
    detector: DetectorEngine,
    windows: HashMap<u64, WindowInfo>,
    workspaces: HashMap<u64, WorkspaceInfo>,
    outputs: HashMap<String, OutputInfo>,
    tracked: HashMap<u64, TrackedWindow>,
    ignored_until_close: HashSet<u64>,
    candidate_opened_at: HashMap<u64, Instant>,
    candidate_previous_focus: HashMap<u64, Option<u64>>,
    focused_window: Option<u64>,
    previous_focused_window: Option<u64>,
    focused_workspace: Option<u64>,
    niri_connected: bool,
    niri_version: Option<String>,
    enabled: bool,
    persistent: PersistentState,
    persistent_dirty: bool,
}

impl Engine {
    pub fn new(config: Config, persistent: PersistentState) -> Result<Self, EngineError> {
        let detector = DetectorEngine::new(&config)?;
        Ok(Self {
            enabled: config.general.enabled,
            config,
            detector,
            windows: HashMap::new(),
            workspaces: HashMap::new(),
            outputs: HashMap::new(),
            tracked: HashMap::new(),
            ignored_until_close: HashSet::new(),
            candidate_opened_at: HashMap::new(),
            candidate_previous_focus: HashMap::new(),
            focused_window: None,
            previous_focused_window: None,
            focused_workspace: None,
            niri_connected: false,
            niri_version: None,
            persistent,
            persistent_dirty: false,
        })
    }

    pub fn replace_config(&mut self, config: Config) -> Result<(), EngineError> {
        let detector = DetectorEngine::new(&config)?;
        self.enabled = config.general.enabled;
        self.detector = detector;
        self.config = config;
        Ok(())
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn opacity_override_percent(&self) -> Option<u8> {
        self.persistent.pip_opacity_percent
    }

    pub fn set_opacity_override_percent(&mut self, percent: Option<u8>) -> Result<(), EngineError> {
        if let Some(percent) = percent {
            if !(10..=100).contains(&percent) {
                return Err(EngineError::InvalidOpacity);
            }
        }
        if self.persistent.pip_opacity_percent != percent {
            self.persistent.pip_opacity_percent = percent;
            self.mark_persistent_dirty();
        }
        Ok(())
    }

    pub fn handle_event(&mut self, event: CompositorEvent) -> Vec<Effect> {
        self.prune_focus_candidates();
        match event {
            CompositorEvent::Connected { version } => {
                self.niri_connected = true;
                self.niri_version = Some(version);
                Vec::new()
            }
            CompositorEvent::Disconnected { .. } => {
                self.niri_connected = false;
                Vec::new()
            }
            CompositorEvent::OutputsChanged(outputs) => {
                if self.outputs == outputs {
                    return Vec::new();
                }
                self.outputs = outputs;
                self.reconcile_unplaced_auto_pip()
            }
            CompositorEvent::WorkspacesChanged(workspaces) => {
                self.workspaces = workspaces.into_iter().map(|w| (w.id, w)).collect();
                self.focused_workspace = self
                    .workspaces
                    .values()
                    .find(|w| w.is_focused)
                    .map(|w| w.id);
                let mut effects = self.reconcile_workspace_follow();
                effects.extend(self.reconcile_unplaced_auto_pip());
                effects
            }
            CompositorEvent::WorkspaceActivated { id, focused } => {
                if let Some(target) = self.workspaces.get(&id).cloned() {
                    let output = target.output;
                    for ws in self.workspaces.values_mut() {
                        if ws.output == output {
                            ws.is_active = ws.id == id;
                        }
                        if focused {
                            ws.is_focused = ws.id == id;
                        }
                    }
                }
                if focused {
                    self.focused_workspace = Some(id);
                }
                Vec::new()
            }
            CompositorEvent::WorkspaceActiveWindowChanged {
                workspace_id,
                active_window_id,
            } => {
                if let Some(ws) = self.workspaces.get_mut(&workspace_id) {
                    ws.active_window_id = active_window_id;
                }
                Vec::new()
            }
            CompositorEvent::WindowsChanged(windows) => self.replace_windows(windows),
            CompositorEvent::WindowOpenedOrChanged(window) => self.upsert_window(window),
            CompositorEvent::WindowClosed { id } => {
                self.windows.remove(&id);
                self.tracked.remove(&id);
                self.ignored_until_close.remove(&id);
                self.candidate_opened_at.remove(&id);
                self.candidate_previous_focus.remove(&id);
                if self.focused_window == Some(id) {
                    self.focused_window = None;
                }
                if self.previous_focused_window == Some(id) {
                    self.previous_focused_window = None;
                }
                Vec::new()
            }
            CompositorEvent::WindowFocusChanged { id } => {
                let restore = id.and_then(|focused_id| self.focus_restore_effect(focused_id));
                if self.focused_window != id {
                    if self.focused_window.is_some() {
                        self.previous_focused_window = self.focused_window;
                    }
                    self.focused_window = id;
                }
                for win in self.windows.values_mut() {
                    win.is_focused = Some(win.id) == id;
                }
                restore.into_iter().collect()
            }
            CompositorEvent::WindowLayoutsChanged(changes) => {
                let mut effects = Vec::new();
                for (id, layout) in changes {
                    effects.extend(self.update_layout(id, layout));
                }
                effects
            }
            CompositorEvent::Unknown(_) => Vec::new(),
        }
    }

    fn replace_windows(&mut self, windows: Vec<WindowInfo>) -> Vec<Effect> {
        let live: HashSet<u64> = windows.iter().map(|w| w.id).collect();
        self.tracked.retain(|id, _| live.contains(id));
        self.ignored_until_close.retain(|id| live.contains(id));
        self.candidate_opened_at.retain(|id, _| live.contains(id));
        self.candidate_previous_focus
            .retain(|id, _| live.contains(id));
        self.windows = windows.into_iter().map(|w| (w.id, w)).collect();
        self.focused_window = self.windows.values().find(|w| w.is_focused).map(|w| w.id);

        let ids: Vec<u64> = self.windows.keys().copied().collect();
        let mut effects = Vec::new();
        for id in ids {
            effects.extend(self.classify_or_reconcile(id, false, true));
        }
        effects
    }

    fn upsert_window(&mut self, window: WindowInfo) -> Vec<Effect> {
        let is_new = !self.windows.contains_key(&window.id);
        let id = window.id;

        if is_new {
            let previous = if self.focused_window == Some(id) {
                self.previous_focused_window
            } else {
                self.focused_window
                    .or_else(|| self.windows.values().find(|w| w.is_focused).map(|w| w.id))
            };
            self.candidate_opened_at.insert(id, Instant::now());
            self.candidate_previous_focus.insert(id, previous);
        }
        if window.is_focused {
            if self.focused_window != Some(id) {
                if self.focused_window.is_some() {
                    self.previous_focused_window = self.focused_window;
                }
                self.focused_window = Some(id);
            }
            for existing in self.windows.values_mut() {
                existing.is_focused = false;
            }
        }

        let workspace_id = window.workspace_id;
        self.windows.insert(id, window);
        if let Some(tracked) = self.tracked.get_mut(&id) {
            if tracked
                .pending_workspace
                .as_ref()
                .is_some_and(|p| workspace_id == Some(p.target))
            {
                tracked.pending_workspace = None;
            }
        }
        self.classify_or_reconcile(id, is_new, false)
    }

    fn classify_or_reconcile(
        &mut self,
        id: u64,
        is_new: bool,
        adopt_existing_geometry: bool,
    ) -> Vec<Effect> {
        if self.tracked.contains_key(&id) {
            return self.reconcile_window(id);
        }
        if !self.enabled
            || !self.config.general.auto_detect
            || self.ignored_until_close.contains(&id)
        {
            return Vec::new();
        }
        let Some(window) = self.windows.get(&id).cloned() else {
            return Vec::new();
        };
        let Some(matched) = self.detector.detect(&window, is_new) else {
            return Vec::new();
        };
        if matched.action == DetectionAction::Ignore {
            self.ignored_until_close.insert(id);
            return Vec::new();
        }

        let controls = self.controls_for_detector(&matched.detector);
        let suppress =
            Instant::now() + Duration::from_millis(self.config.general.action_suppression_ms);
        let live_size = window.logical_size();
        let managed_size = if adopt_existing_geometry
            && !controls.geometry_locked
            && live_size.0 >= 120
            && live_size.1 >= 68
        {
            live_size
        } else {
            self.resolve_initial_pip_size(&window, &matched.detector)
        };
        let locked_geometry = if controls.geometry_locked {
            self.persistent.profiles.get(&matched.detector).copied()
        } else {
            None
        };
        self.tracked.insert(
            id,
            TrackedWindow {
                mode: TrackedMode::AutoPip,
                detector: Some(matched.detector.clone()),
                score: Some(matched.score),
                follow_enabled: controls.follow_enabled,
                follow_mode: controls.follow_mode,
                original_was_floating: window.is_floating,
                placement: controls.placement,
                managed_size: Some(managed_size),
                pending_workspace: None,
                suppress_geometry_until: suppress,
                geometry_locked: controls.geometry_locked,
                locked_geometry,
            },
        );
        if adopt_existing_geometry && !controls.geometry_locked {
            self.adopt_existing_auto(id)
        } else {
            self.initial_manage_auto(id)
        }
    }

    fn adopt_existing_auto(&mut self, id: u64) -> Vec<Effect> {
        let Some(window) = self.windows.get(&id).cloned() else {
            return Vec::new();
        };

        // A WindowsChanged snapshot is authoritative live compositor state. This path is used
        // during daemon startup/reconnect, so an already-open PiP may have been manually resized
        // or moved while niri-pip was stopped. Adopt that live geometry instead of replaying an
        // older remembered profile over it. Explicit geometry-lock is handled by the normal
        // initial-management path and remains intentionally authoritative.
        let live_size = window.logical_size();
        if live_size.0 >= 120 && live_size.1 >= 68 {
            if let Some(tracked) = self.tracked.get_mut(&id) {
                tracked.managed_size = Some(live_size);
            }
        }

        if let Some(name) = self
            .tracked
            .get(&id)
            .and_then(|tracked| tracked.detector.clone())
        {
            if let Some(live_geometry) = self.current_geometry(id) {
                if self.persistent.profiles.get(&name) != Some(&live_geometry) {
                    self.persistent.profiles.insert(name, live_geometry);
                    self.mark_persistent_dirty();
                }
            } else if live_size.0 >= 120 && live_size.1 >= 68 {
                // If workspace/output metadata has not arrived yet, at least preserve the live
                // dimensions while retaining the last known position. A later layout event will
                // replace the full profile with authoritative live coordinates.
                let changed = if let Some(remembered) = self.persistent.profiles.get_mut(&name) {
                    if (remembered.width, remembered.height) != live_size {
                        remembered.width = live_size.0;
                        remembered.height = live_size.1;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                if changed {
                    self.mark_persistent_dirty();
                }
            }
        }

        let mut effects = Vec::new();
        if !window.is_floating {
            effects.push(Effect::Action(CompositorAction::MoveWindowToFloating {
                id,
            }));
        }
        effects.extend(self.reconcile_workspace_follow_for(id));
        effects
    }

    fn controls_for_detector(&self, detector_name: &str) -> RememberedControls {
        self.persistent
            .controls
            .get(detector_name)
            .copied()
            .unwrap_or(RememberedControls {
                placement: self.config.pip.position,
                follow_enabled: self.config.general.follow_workspace,
                follow_mode: self.config.general.follow_mode,
                geometry_locked: false,
            })
    }

    fn initial_manage_auto(&mut self, id: u64) -> Vec<Effect> {
        let Some(window) = self.windows.get(&id).cloned() else {
            return Vec::new();
        };
        let detector_name = self.tracked.get(&id).and_then(|t| t.detector.clone());
        let remembered = detector_name
            .as_ref()
            .and_then(|name| self.persistent.profiles.get(name))
            .copied()
            .filter(|_| {
                self.config.general.remember_geometry
                    && self.config.pip.position_mode == PositionMode::Remember
            });
        let (width, height) = self.desired_size(id);

        let mut effects = Vec::new();
        if !window.is_floating {
            effects.push(Effect::Action(CompositorAction::MoveWindowToFloating {
                id,
            }));
        }
        effects.push(Effect::Action(CompositorAction::SetWindowWidth {
            id,
            change: SizeChange::SetFixed(width as i32),
        }));
        effects.push(Effect::Action(CompositorAction::SetWindowHeight {
            id,
            change: SizeChange::SetFixed(height as i32),
        }));

        let target_workspace = self.follow_target_for(id).or(window.workspace_id);
        if let (Some(current), Some(target)) = (window.workspace_id, target_workspace) {
            if current != target {
                effects.push(Effect::Action(CompositorAction::MoveWindowToWorkspace {
                    window_id: id,
                    workspace_id: target,
                    focus: false,
                }));
                if let Some(tracked) = self.tracked.get_mut(&id) {
                    tracked.pending_workspace = Some(PendingWorkspaceMove {
                        target,
                        issued_at: Instant::now(),
                    });
                }
            }
        }

        let placement = self.tracked.get(&id).map(|t| t.placement);
        if let Some(plan) = remembered
            .map(|g| PlacementPlan {
                x_percent: g.x_percent,
                y_percent: g.y_percent,
            })
            .or_else(|| {
                placement.and_then(|placement| {
                    self.placement_plan(target_workspace, (width, height), placement)
                })
            })
        {
            effects.push(place_effect(id, plan));
            if self
                .tracked
                .get(&id)
                .is_some_and(|tracked| tracked.geometry_locked && tracked.locked_geometry.is_none())
            {
                let locked = RememberedGeometry {
                    width,
                    height,
                    x_percent: plan.x_percent,
                    y_percent: plan.y_percent,
                };
                if let Some(tracked) = self.tracked.get_mut(&id) {
                    tracked.locked_geometry = Some(locked);
                }
            }
        }

        if window.is_focused {
            if let Some(effect) = self.focus_restore_effect(id) {
                effects.push(effect);
            }
        }
        effects
    }

    fn focus_restore_effect(&mut self, id: u64) -> Option<Effect> {
        let is_auto_pip = self
            .tracked
            .get(&id)
            .is_some_and(|tracked| tracked.mode == TrackedMode::AutoPip);
        if !is_auto_pip {
            return None;
        }

        let recent = self.candidate_opened_at.get(&id).is_some_and(|opened| {
            opened.elapsed() <= Duration::from_millis(self.config.general.focus_restore_window_ms)
        });
        let previous = self.candidate_previous_focus.get(&id).copied().flatten();
        self.candidate_opened_at.remove(&id);
        self.candidate_previous_focus.remove(&id);

        if !recent {
            return None;
        }
        previous
            .filter(|previous| *previous != id && self.windows.contains_key(previous))
            .map(|previous| Effect::Action(CompositorAction::FocusWindow { id: previous }))
    }

    fn prune_focus_candidates(&mut self) {
        let max_age = Duration::from_millis(self.config.general.focus_restore_window_ms);
        let stale: Vec<u64> = self
            .candidate_opened_at
            .iter()
            .filter_map(|(id, opened)| (opened.elapsed() > max_age).then_some(*id))
            .collect();
        for id in stale {
            self.candidate_opened_at.remove(&id);
            self.candidate_previous_focus.remove(&id);
        }
    }

    fn reconcile_window(&mut self, id: u64) -> Vec<Effect> {
        if !self.enabled {
            return Vec::new();
        }
        let Some(window) = self.windows.get(&id).cloned() else {
            return Vec::new();
        };
        let Some(tracked) = self.tracked.get(&id) else {
            return Vec::new();
        };
        let mode = tracked.mode;
        let mut effects = Vec::new();
        if !window.is_floating {
            effects.push(Effect::Action(CompositorAction::MoveWindowToFloating {
                id,
            }));
            if mode == TrackedMode::AutoPip {
                let (w, h) = self.desired_size(id);
                effects.push(Effect::Action(CompositorAction::SetWindowWidth {
                    id,
                    change: SizeChange::SetFixed(w as i32),
                }));
                effects.push(Effect::Action(CompositorAction::SetWindowHeight {
                    id,
                    change: SizeChange::SetFixed(h as i32),
                }));
                if let Some(plan) = self.placement_for_tracked(id, (w, h)) {
                    effects.push(place_effect(id, plan));
                }
            }
            if let Some(tracked) = self.tracked.get_mut(&id) {
                tracked.suppress_geometry_until = Instant::now()
                    + Duration::from_millis(self.config.general.action_suppression_ms);
            }
        }
        effects
    }

    fn reconcile_unplaced_auto_pip(&mut self) -> Vec<Effect> {
        if !self.enabled {
            return Vec::new();
        }
        let ids: Vec<u64> = self
            .tracked
            .iter()
            .filter_map(|(id, t)| (t.mode == TrackedMode::AutoPip).then_some(*id))
            .collect();
        let mut effects = Vec::new();
        for id in ids {
            let (w, h) = self.desired_size(id);
            if let Some(plan) = self.placement_for_tracked(id, (w, h)) {
                effects.push(place_effect(id, plan));
            }
        }
        effects
    }

    pub fn reconcile_workspace_follow(&mut self) -> Vec<Effect> {
        if !self.enabled {
            return Vec::new();
        }
        let Some(target) = self.focused_workspace else {
            return Vec::new();
        };
        let ids: Vec<u64> = self.tracked.keys().copied().collect();
        let mut effects = Vec::new();
        for id in ids {
            effects.extend(self.reconcile_workspace_follow_for_target(id, target));
        }
        effects
    }

    fn reconcile_workspace_follow_for_target(&mut self, id: u64, target: u64) -> Vec<Effect> {
        let Some(window) = self.windows.get(&id).cloned() else {
            return Vec::new();
        };
        let Some(tracked) = self.tracked.get(&id) else {
            return Vec::new();
        };
        if !tracked.follow_enabled
            || !self.follow_allowed(tracked.follow_mode, window.workspace_id, target)
            || window.workspace_id == Some(target)
        {
            return Vec::new();
        }
        if tracked.pending_workspace.as_ref().is_some_and(|pending| {
            pending.target == target && pending.issued_at.elapsed() < Duration::from_secs(1)
        }) {
            return Vec::new();
        }

        let mode = tracked.mode;
        let geometry_locked = tracked.geometry_locked;
        let mut effects = vec![Effect::Action(CompositorAction::MoveWindowToWorkspace {
            window_id: id,
            workspace_id: target,
            focus: false,
        })];
        if let Some(t) = self.tracked.get_mut(&id) {
            t.pending_workspace = Some(PendingWorkspaceMove {
                target,
                issued_at: Instant::now(),
            });
            t.suppress_geometry_until =
                Instant::now() + Duration::from_millis(self.config.general.action_suppression_ms);
        }
        if mode == TrackedMode::AutoPip || geometry_locked {
            let (w, h) = self.desired_size(id);
            if let Some(plan) = self.placement_for_tracked(id, (w, h)) {
                effects.push(place_effect(id, plan));
            }
        }
        effects
    }

    fn follow_allowed(&self, mode: FollowMode, current: Option<u64>, target: u64) -> bool {
        match mode {
            FollowMode::FollowWorkspace | FollowMode::FollowFocusedOutput => true,
            FollowMode::StayOnOutput => {
                let current_output = current
                    .and_then(|id| self.workspaces.get(&id))
                    .and_then(|w| w.output.as_deref());
                let target_output = self
                    .workspaces
                    .get(&target)
                    .and_then(|w| w.output.as_deref());
                current_output.is_some() && current_output == target_output
            }
        }
    }

    fn follow_target_for(&self, id: u64) -> Option<u64> {
        let tracked = self.tracked.get(&id)?;
        if !tracked.follow_enabled {
            return self.windows.get(&id).and_then(|w| w.workspace_id);
        }
        let target = self.focused_workspace?;
        if self.follow_allowed(
            tracked.follow_mode,
            self.windows.get(&id).and_then(|w| w.workspace_id),
            target,
        ) {
            Some(target)
        } else {
            self.windows.get(&id).and_then(|w| w.workspace_id)
        }
    }

    fn desired_size(&self, id: u64) -> (u32, u32) {
        self.tracked
            .get(&id)
            .and_then(|t| t.managed_size)
            .unwrap_or_else(|| self.config.pip.resolved_size())
    }

    fn resolve_initial_pip_size(&self, window: &WindowInfo, detector_name: &str) -> (u32, u32) {
        if self.config.general.remember_geometry
            && self.config.pip.position_mode == PositionMode::Remember
        {
            if let Some(g) = self.persistent.profiles.get(detector_name) {
                return (g.width, g.height);
            }
        }

        let (base_w, base_h) = self.config.pip.resolved_size();
        if !self.config.pip.preserve_aspect_ratio {
            return (base_w, base_h);
        }
        let (current_w, current_h) = window.logical_size();
        if current_w == 0 || current_h == 0 {
            return (base_w, base_h);
        }
        let ratio = current_w as f64 / current_h as f64;
        if !(0.4..=3.5).contains(&ratio) {
            return (base_w, base_h);
        }

        let area = (base_w as f64 * base_h as f64).max(1.0);
        let width = (area * ratio).sqrt().round().max(120.0) as u32;
        let height = (area / ratio).sqrt().round().max(68.0) as u32;
        (width, height)
    }

    fn placement_for_tracked(&self, id: u64, size: (u32, u32)) -> Option<PlacementPlan> {
        let tracked = self.tracked.get(&id)?;
        if let Some(locked) = tracked.locked_geometry {
            return Some(PlacementPlan {
                x_percent: locked.x_percent,
                y_percent: locked.y_percent,
            });
        }
        if self.config.general.remember_geometry
            && self.config.pip.position_mode == PositionMode::Remember
        {
            if let Some(name) = tracked.detector.as_ref() {
                if let Some(g) = self.persistent.profiles.get(name) {
                    return Some(PlacementPlan {
                        x_percent: g.x_percent,
                        y_percent: g.y_percent,
                    });
                }
            }
        }
        self.placement_plan(self.follow_target_for(id), size, tracked.placement)
    }

    fn placement_plan(
        &self,
        workspace_id: Option<u64>,
        size: (u32, u32),
        placement: Placement,
    ) -> Option<PlacementPlan> {
        let workspace_id = workspace_id?;
        let output_name = self.workspaces.get(&workspace_id)?.output.as_ref()?;
        let output = self.outputs.get(output_name)?.logical?;
        Some(corner_placement(
            output,
            size,
            placement,
            self.config.pip.gap,
            self.config.margins,
        ))
    }

    fn current_geometry(&self, id: u64) -> Option<RememberedGeometry> {
        let window = self.windows.get(&id)?;
        let workspace_id = window.workspace_id?;
        let output_name = self.workspaces.get(&workspace_id)?.output.as_ref()?;
        let logical = self.outputs.get(output_name)?.logical?;
        let (x, y) = window.layout.tile_pos_in_workspace_view?;
        let (width, height) = window.logical_size();
        if width == 0 || height == 0 || logical.width == 0 || logical.height == 0 {
            return None;
        }
        Some(RememberedGeometry {
            width,
            height,
            x_percent: (x / logical.width as f64 * 100.0).clamp(0.0, 100.0),
            y_percent: (y / logical.height as f64 * 100.0).clamp(0.0, 100.0),
        })
    }

    fn geometry_from_plan(&self, id: u64, size: (u32, u32)) -> Option<RememberedGeometry> {
        let tracked = self.tracked.get(&id)?;
        let plan = self.placement_plan(self.follow_target_for(id), size, tracked.placement)?;
        Some(RememberedGeometry {
            width: size.0,
            height: size.1,
            x_percent: plan.x_percent,
            y_percent: plan.y_percent,
        })
    }

    fn update_layout(&mut self, id: u64, layout: WindowLayout) -> Vec<Effect> {
        if let Some(window) = self.windows.get_mut(&id) {
            window.layout = layout.clone();
        }

        let Some((mode, suppress_geometry_until, detector_name, geometry_locked, locked)) =
            self.tracked.get(&id).map(|tracked| {
                (
                    tracked.mode,
                    tracked.suppress_geometry_until,
                    tracked.detector.clone(),
                    tracked.geometry_locked,
                    tracked.locked_geometry,
                )
            })
        else {
            return Vec::new();
        };

        if Instant::now() <= suppress_geometry_until {
            return Vec::new();
        }

        if geometry_locked {
            let Some(locked) = locked else {
                return Vec::new();
            };
            let current_size = self
                .windows
                .get(&id)
                .map(WindowInfo::logical_size)
                .unwrap_or((0, 0));
            let current_geometry = self.current_geometry(id);
            let mut effects = Vec::new();
            if current_size.0.abs_diff(locked.width) > 1 {
                effects.push(Effect::Action(CompositorAction::SetWindowWidth {
                    id,
                    change: SizeChange::SetFixed(locked.width as i32),
                }));
            }
            if current_size.1.abs_diff(locked.height) > 1 {
                effects.push(Effect::Action(CompositorAction::SetWindowHeight {
                    id,
                    change: SizeChange::SetFixed(locked.height as i32),
                }));
            }
            if current_geometry.is_some_and(|current| {
                (current.x_percent - locked.x_percent).abs() > 0.25
                    || (current.y_percent - locked.y_percent).abs() > 0.25
            }) {
                effects.push(place_effect(
                    id,
                    PlacementPlan {
                        x_percent: locked.x_percent,
                        y_percent: locked.y_percent,
                    },
                ));
            }
            if !effects.is_empty() {
                if let Some(tracked) = self.tracked.get_mut(&id) {
                    tracked.suppress_geometry_until = Instant::now()
                        + Duration::from_millis(self.config.general.action_suppression_ms);
                }
            }
            return effects;
        }

        if mode != TrackedMode::AutoPip
            || !self.config.general.remember_geometry
            || self.config.pip.position_mode != PositionMode::Remember
        {
            return Vec::new();
        }
        let Some(name) = detector_name else {
            return Vec::new();
        };
        let Some(remembered) = self.current_geometry(id) else {
            return Vec::new();
        };
        if self.persistent.profiles.get(&name) != Some(&remembered) {
            self.persistent.profiles.insert(name, remembered);
            self.mark_persistent_dirty();
        }
        if let Some(tracked) = self.tracked.get_mut(&id) {
            tracked.managed_size = Some((remembered.width, remembered.height));
        }
        Vec::new()
    }

    pub fn focused_window_id(&self) -> Option<u64> {
        self.focused_window
            .or_else(|| self.windows.values().find(|w| w.is_focused).map(|w| w.id))
    }

    fn resolve_window_id(&self, requested: Option<u64>) -> Result<u64, EngineError> {
        requested
            .or_else(|| self.focused_window_id())
            .ok_or(EngineError::NoFocusedWindow)
    }

    fn resolve_tracked_window_id(&self, requested: Option<u64>) -> Result<u64, EngineError> {
        if let Some(id) = requested {
            return self
                .tracked
                .contains_key(&id)
                .then_some(id)
                .ok_or(EngineError::NotPinned(id));
        }
        if let Some(id) = self
            .focused_window_id()
            .filter(|id| self.tracked.contains_key(id))
        {
            return Ok(id);
        }

        let auto: Vec<u64> = self
            .tracked
            .iter()
            .filter_map(|(id, tracked)| (tracked.mode == TrackedMode::AutoPip).then_some(*id))
            .collect();
        if auto.len() == 1 {
            return Ok(auto[0]);
        }
        if self.tracked.len() == 1 {
            return self
                .tracked
                .keys()
                .next()
                .copied()
                .ok_or(EngineError::NoTrackedWindow);
        }
        if self.tracked.is_empty() {
            Err(EngineError::NoTrackedWindow)
        } else {
            Err(EngineError::AmbiguousTrackedWindow)
        }
    }

    pub fn pin(&mut self, requested: Option<u64>) -> Result<Vec<Effect>, EngineError> {
        if !self.enabled {
            return Err(EngineError::Disabled);
        }
        let id = self.resolve_window_id(requested)?;
        let window = self
            .windows
            .get(&id)
            .cloned()
            .ok_or(EngineError::WindowNotFound(id))?;
        self.ignored_until_close.remove(&id);
        if self.tracked.contains_key(&id) {
            return Ok(Vec::new());
        }
        self.tracked.insert(
            id,
            TrackedWindow {
                mode: TrackedMode::ManualPin,
                detector: None,
                score: None,
                follow_enabled: self.config.general.follow_workspace,
                follow_mode: self.config.general.follow_mode,
                original_was_floating: window.is_floating,
                placement: self.config.pip.position,
                managed_size: None,
                pending_workspace: None,
                suppress_geometry_until: Instant::now()
                    + Duration::from_millis(self.config.general.action_suppression_ms),
                geometry_locked: false,
                locked_geometry: None,
            },
        );
        let mut effects = Vec::new();
        if !window.is_floating {
            effects.push(Effect::Action(CompositorAction::MoveWindowToFloating {
                id,
            }));
        }
        effects.extend(self.reconcile_workspace_follow_for(id));
        Ok(effects)
    }

    fn reconcile_workspace_follow_for(&mut self, id: u64) -> Vec<Effect> {
        let Some(target) = self.focused_workspace else {
            return Vec::new();
        };
        self.reconcile_workspace_follow_for_target(id, target)
    }

    pub fn unpin(&mut self, requested: Option<u64>) -> Result<Vec<Effect>, EngineError> {
        let id = match requested {
            Some(id) => id,
            None => self
                .focused_window_id()
                .filter(|id| self.tracked.contains_key(id))
                .or_else(|| {
                    let mut ids = self.tracked.keys().copied();
                    let first = ids.next()?;
                    ids.next().is_none().then_some(first)
                })
                .ok_or(EngineError::NoTrackedWindow)?,
        };
        let tracked = self.tracked.remove(&id).ok_or(EngineError::NotPinned(id))?;
        if tracked.mode == TrackedMode::AutoPip {
            self.ignored_until_close.insert(id);
        }
        let mut effects = Vec::new();
        if tracked.mode == TrackedMode::ManualPin
            && self.config.general.restore_layout_on_unpin
            && !tracked.original_was_floating
            && self.windows.contains_key(&id)
        {
            effects.push(Effect::Action(CompositorAction::MoveWindowToTiling { id }));
        }
        Ok(effects)
    }

    pub fn toggle(&mut self, requested: Option<u64>) -> Result<Vec<Effect>, EngineError> {
        let id = self.resolve_window_id(requested)?;
        if self.tracked.contains_key(&id) {
            self.unpin(Some(id))
        } else {
            self.pin(Some(id))
        }
    }

    pub fn resize(
        &mut self,
        requested: Option<u64>,
        width: u32,
        height: u32,
    ) -> Result<Vec<Effect>, EngineError> {
        if width < 120 || height < 68 || width > i32::MAX as u32 || height > i32::MAX as u32 {
            return Err(EngineError::InvalidSize(width, height));
        }
        let id = self.resolve_tracked_window_id(requested)?;
        if !self.windows.contains_key(&id) {
            return Err(EngineError::WindowNotFound(id));
        }
        let size = (width, height);
        // Optimistically mirror an explicit controller resize. Niri will send the authoritative
        // WindowLayoutsChanged event afterwards, but this keeps chained operations such as
        // `preset` (resize + position) consistent before that event arrives.
        if let Some(window) = self.windows.get_mut(&id) {
            window.layout.window_size = (width as i32, height as i32);
        }
        if let Some(tracked) = self.tracked.get_mut(&id) {
            tracked.managed_size = Some(size);
            tracked.suppress_geometry_until =
                Instant::now() + Duration::from_millis(self.config.general.action_suppression_ms);
            if tracked.geometry_locked {
                if let Some(locked) = tracked.locked_geometry.as_mut() {
                    locked.width = width;
                    locked.height = height;
                }
            }
        }
        self.persist_auto_size(id, size);
        Ok(vec![
            Effect::Action(CompositorAction::SetWindowWidth {
                id,
                change: SizeChange::SetFixed(width as i32),
            }),
            Effect::Action(CompositorAction::SetWindowHeight {
                id,
                change: SizeChange::SetFixed(height as i32),
            }),
        ])
    }

    pub fn scale(
        &mut self,
        requested: Option<u64>,
        percent: i32,
    ) -> Result<Vec<Effect>, EngineError> {
        let id = self.resolve_tracked_window_id(requested)?;
        let window = self
            .windows
            .get(&id)
            .ok_or(EngineError::WindowNotFound(id))?;
        let (current_w, current_h) = window.logical_size();
        let (base_w, base_h) = if current_w == 0 || current_h == 0 {
            self.desired_size(id)
        } else {
            (current_w, current_h)
        };
        let factor = 1.0 + percent as f64 / 100.0;
        if factor <= 0.0 {
            return Err(EngineError::InvalidScale);
        }
        let width = (base_w as f64 * factor).round() as u32;
        let height = (base_h as f64 * factor).round() as u32;
        if width < 120 || height < 68 {
            return Err(EngineError::InvalidScale);
        }
        self.resize(Some(id), width, height)
    }

    pub fn set_position(
        &mut self,
        requested: Option<u64>,
        placement: Placement,
    ) -> Result<Vec<Effect>, EngineError> {
        let id = self.resolve_tracked_window_id(requested)?;
        let size = self
            .windows
            .get(&id)
            .map(WindowInfo::logical_size)
            .filter(|(w, h)| *w > 0 && *h > 0)
            .unwrap_or_else(|| self.desired_size(id));
        let target = self.follow_target_for(id);
        let Some(plan) = self.placement_plan(target, size, placement) else {
            return Ok(Vec::new());
        };
        let detector = self.tracked.get(&id).and_then(|t| t.detector.clone());
        if let Some(tracked) = self.tracked.get_mut(&id) {
            tracked.placement = placement;
            tracked.suppress_geometry_until =
                Instant::now() + Duration::from_millis(self.config.general.action_suppression_ms);
            if tracked.geometry_locked {
                tracked.locked_geometry = Some(RememberedGeometry {
                    width: size.0,
                    height: size.1,
                    x_percent: plan.x_percent,
                    y_percent: plan.y_percent,
                });
            }
        }
        if let Some(name) = detector {
            let controls = self.controls_for_detector(&name);
            self.persistent.controls.insert(
                name.clone(),
                RememberedControls {
                    placement,
                    ..controls
                },
            );
            let mut geometry =
                self.persistent
                    .profiles
                    .get(&name)
                    .copied()
                    .unwrap_or(RememberedGeometry {
                        width: size.0,
                        height: size.1,
                        x_percent: plan.x_percent,
                        y_percent: plan.y_percent,
                    });
            geometry.width = size.0;
            geometry.height = size.1;
            geometry.x_percent = plan.x_percent;
            geometry.y_percent = plan.y_percent;
            self.persistent.profiles.insert(name, geometry);
            self.mark_persistent_dirty();
        }
        Ok(vec![place_effect(id, plan)])
    }

    pub fn nudge(
        &mut self,
        requested: Option<u64>,
        dx: i32,
        dy: i32,
    ) -> Result<Vec<Effect>, EngineError> {
        let id = self.resolve_tracked_window_id(requested)?;
        if dx == 0 && dy == 0 {
            return Ok(Vec::new());
        }

        let mut updated_geometry = self.current_geometry(id);
        if let Some(geometry) = updated_geometry.as_mut() {
            if let Some(window) = self.windows.get(&id) {
                if let Some(workspace_id) = window.workspace_id {
                    if let Some(output_name) = self
                        .workspaces
                        .get(&workspace_id)
                        .and_then(|workspace| workspace.output.as_ref())
                    {
                        if let Some(logical) = self.outputs.get(output_name).and_then(|o| o.logical)
                        {
                            if logical.width > 0 && logical.height > 0 {
                                geometry.x_percent = (geometry.x_percent
                                    + dx as f64 / logical.width as f64 * 100.0)
                                    .clamp(0.0, 100.0);
                                geometry.y_percent = (geometry.y_percent
                                    + dy as f64 / logical.height as f64 * 100.0)
                                    .clamp(0.0, 100.0);
                            }
                        }
                    }
                }
            }
        }

        let detector = self.tracked.get(&id).and_then(|t| t.detector.clone());
        if let Some(tracked) = self.tracked.get_mut(&id) {
            tracked.suppress_geometry_until =
                Instant::now() + Duration::from_millis(self.config.general.action_suppression_ms);
            if tracked.geometry_locked {
                if let Some(geometry) = updated_geometry {
                    tracked.locked_geometry = Some(geometry);
                }
            }
        }
        if let (Some(name), Some(geometry)) = (detector, updated_geometry) {
            self.persistent.profiles.insert(name, geometry);
            self.mark_persistent_dirty();
        }

        Ok(vec![Effect::Action(CompositorAction::MoveFloatingWindow {
            id,
            x: PositionChange::AdjustFixed(dx as f64),
            y: PositionChange::AdjustFixed(dy as f64),
        })])
    }

    pub fn set_follow(
        &mut self,
        requested: Option<u64>,
        enabled: bool,
    ) -> Result<Vec<Effect>, EngineError> {
        let id = self.resolve_tracked_window_id(requested)?;
        let detector = self.tracked.get(&id).and_then(|t| t.detector.clone());
        if let Some(tracked) = self.tracked.get_mut(&id) {
            tracked.follow_enabled = enabled;
            tracked.pending_workspace = None;
        }
        if let Some(name) = detector {
            let mut controls = self.controls_for_detector(&name);
            controls.follow_enabled = enabled;
            self.persistent.controls.insert(name, controls);
            self.mark_persistent_dirty();
        }
        if enabled {
            Ok(self.reconcile_workspace_follow_for(id))
        } else {
            Ok(Vec::new())
        }
    }

    pub fn set_follow_mode(
        &mut self,
        requested: Option<u64>,
        mode: FollowMode,
    ) -> Result<Vec<Effect>, EngineError> {
        let id = self.resolve_tracked_window_id(requested)?;
        let detector = self.tracked.get(&id).and_then(|t| t.detector.clone());
        let follow_enabled = if let Some(tracked) = self.tracked.get_mut(&id) {
            tracked.follow_mode = mode;
            tracked.pending_workspace = None;
            tracked.follow_enabled
        } else {
            false
        };
        if let Some(name) = detector {
            let mut controls = self.controls_for_detector(&name);
            controls.follow_mode = mode;
            self.persistent.controls.insert(name, controls);
            self.mark_persistent_dirty();
        }
        if follow_enabled {
            Ok(self.reconcile_workspace_follow_for(id))
        } else {
            Ok(Vec::new())
        }
    }

    pub fn set_geometry_lock(
        &mut self,
        requested: Option<u64>,
        locked: bool,
    ) -> Result<Vec<Effect>, EngineError> {
        let id = self.resolve_tracked_window_id(requested)?;
        let detector = self.tracked.get(&id).and_then(|t| t.detector.clone());
        let geometry = if locked {
            let current_size = self
                .windows
                .get(&id)
                .map(WindowInfo::logical_size)
                .filter(|(width, height)| *width > 0 && *height > 0)
                .unwrap_or_else(|| self.desired_size(id));
            self.current_geometry(id)
                .or_else(|| self.geometry_from_plan(id, current_size))
        } else {
            None
        };
        if let Some(tracked) = self.tracked.get_mut(&id) {
            tracked.geometry_locked = locked;
            tracked.locked_geometry = geometry;
        }
        if let Some(name) = detector {
            let mut controls = self.controls_for_detector(&name);
            controls.geometry_locked = locked;
            self.persistent.controls.insert(name.clone(), controls);
            if let Some(geometry) = geometry {
                self.persistent.profiles.insert(name, geometry);
            }
            self.mark_persistent_dirty();
        }
        Ok(Vec::new())
    }

    pub fn reset_geometry(&mut self, requested: Option<u64>) -> Result<Vec<Effect>, EngineError> {
        let id = self.resolve_tracked_window_id(requested)?;
        let mode = self
            .tracked
            .get(&id)
            .map(|tracked| tracked.mode)
            .ok_or(EngineError::NotPinned(id))?;
        let detector = self.tracked.get(&id).and_then(|t| t.detector.clone());
        if let Some(name) = detector.as_ref() {
            self.persistent.profiles.remove(name);
            self.persistent.controls.remove(name);
            self.mark_persistent_dirty();
        }
        if let Some(tracked) = self.tracked.get_mut(&id) {
            tracked.placement = self.config.pip.position;
            tracked.follow_enabled = self.config.general.follow_workspace;
            tracked.follow_mode = self.config.general.follow_mode;
            tracked.geometry_locked = false;
            tracked.locked_geometry = None;
            tracked.pending_workspace = None;
        }
        let mut effects = Vec::new();
        if mode == TrackedMode::AutoPip {
            let window = self
                .windows
                .get(&id)
                .cloned()
                .ok_or(EngineError::WindowNotFound(id))?;
            let detector_name = detector.as_deref().unwrap_or("generic-pip-title");
            let size = self.resolve_initial_pip_size(&window, detector_name);
            if let Some(tracked) = self.tracked.get_mut(&id) {
                tracked.managed_size = Some(size);
                tracked.suppress_geometry_until = Instant::now()
                    + Duration::from_millis(self.config.general.action_suppression_ms);
            }
            effects.extend(self.resize(Some(id), size.0, size.1)?);
            effects.extend(self.set_position(Some(id), self.config.pip.position)?);
        }
        effects.extend(self.reconcile_workspace_follow_for(id));
        Ok(effects)
    }

    pub fn apply_preset(
        &mut self,
        requested: Option<u64>,
        preset: ControlPreset,
    ) -> Result<PresetApplication, EngineError> {
        let id = self.resolve_tracked_window_id(requested)?;
        let target_is_auto_pip = self
            .tracked
            .get(&id)
            .is_some_and(|tracked| tracked.mode == TrackedMode::AutoPip);
        let (width, height, placement, preset_opacity_percent) = match preset {
            ControlPreset::Tiny => (320, 180, Placement::BottomRight, Some(100)),
            ControlPreset::Small => (384, 216, Placement::BottomRight, Some(100)),
            ControlPreset::Medium => (480, 270, Placement::BottomRight, Some(100)),
            ControlPreset::Large => (640, 360, Placement::BottomRight, Some(100)),
            ControlPreset::Cinema => (960, 540, Placement::BottomRight, Some(100)),
            ControlPreset::Movie => (1120, 630, Placement::BottomRight, Some(100)),
            ControlPreset::Study => (560, 315, Placement::TopRight, Some(95)),
        };
        let mut effects = self.resize(Some(id), width, height)?;
        effects.extend(self.set_position(Some(id), placement)?);
        effects.extend(self.set_follow(Some(id), true)?);

        // Opacity is a title-based PiP runtime rule, not a per-window Niri IPC action.
        // Applying a geometry preset to a manually pinned Kitty/editor must therefore not
        // unexpectedly change the opacity policy of an unrelated browser PiP window.
        let opacity_percent = if target_is_auto_pip {
            self.set_opacity_override_percent(preset_opacity_percent)?;
            preset_opacity_percent
        } else {
            self.opacity_override_percent()
        };
        Ok(PresetApplication {
            effects,
            opacity_percent,
        })
    }

    fn persist_auto_size(&mut self, id: u64, size: (u32, u32)) {
        let detector = self.tracked.get(&id).and_then(|t| t.detector.clone());
        let Some(name) = detector else {
            return;
        };
        let mut geometry = self
            .persistent
            .profiles
            .get(&name)
            .copied()
            .or_else(|| self.current_geometry(id))
            .or_else(|| self.geometry_from_plan(id, size))
            .unwrap_or(RememberedGeometry {
                width: size.0,
                height: size.1,
                x_percent: 70.0,
                y_percent: 70.0,
            });
        geometry.width = size.0;
        geometry.height = size.1;
        self.persistent.profiles.insert(name, geometry);
        self.mark_persistent_dirty();
    }

    fn mark_persistent_dirty(&mut self) {
        self.persistent.schema_version = STATE_SCHEMA_VERSION;
        self.persistent_dirty = true;
    }

    pub fn take_persistent_if_dirty(&mut self) -> Option<PersistentState> {
        if self.persistent_dirty {
            self.persistent_dirty = false;
            Some(self.persistent.clone())
        } else {
            None
        }
    }

    pub fn status_snapshot(&self) -> StatusSnapshot {
        let mut windows = self.tracked_snapshots();
        windows.sort_by_key(|w| w.id);
        StatusSnapshot {
            version: env!("CARGO_PKG_VERSION").to_string(),
            daemon_running: true,
            niri_connected: self.niri_connected,
            niri_version: self.niri_version.clone(),
            enabled: self.enabled,
            tracked: self.tracked.len(),
            pinned: self.tracked.len(),
            focused_workspace: self.focused_workspace,
            opacity_override_percent: self.persistent.pip_opacity_percent,
            windows,
        }
    }

    pub fn tracked_snapshots(&self) -> Vec<TrackedWindowSnapshot> {
        self.tracked
            .iter()
            .filter_map(|(id, tracked)| {
                let window = self.windows.get(id)?;
                let (width, height) = window.logical_size();
                Some(TrackedWindowSnapshot {
                    id: *id,
                    title: window.title().to_owned(),
                    app_id: window.app_id().to_owned(),
                    mode: match tracked.mode {
                        TrackedMode::AutoPip => "auto-pip",
                        TrackedMode::ManualPin => "manual-pin",
                    }
                    .into(),
                    detector: tracked.detector.clone(),
                    score: tracked.score,
                    workspace_id: window.workspace_id,
                    width,
                    height,
                    placement: tracked.placement,
                    follow_enabled: tracked.follow_enabled,
                    follow_mode: tracked.follow_mode,
                    geometry_locked: tracked.geometry_locked,
                })
            })
            .collect()
    }
}

fn place_effect(id: u64, plan: PlacementPlan) -> Effect {
    Effect::Action(CompositorAction::MoveFloatingWindow {
        id,
        x: PositionChange::SetProportion(plan.x_percent),
        y: PositionChange::SetProportion(plan.y_percent),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LogicalOutput;

    fn engine() -> Engine {
        let mut engine = Engine::new(Config::default(), PersistentState::default()).unwrap();
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
                ..Default::default()
            },
            WorkspaceInfo {
                id: 2,
                idx: 2,
                output: Some("eDP-1".into()),
                ..Default::default()
            },
        ]));
        engine
    }

    fn pip(id: u64, workspace: u64) -> WindowInfo {
        WindowInfo {
            id,
            title: Some("Picture in picture".into()),
            app_id: Some("".into()),
            workspace_id: Some(workspace),
            layout: WindowLayout {
                window_size: (500, 281),
                tile_size: (500.0, 281.0),
                tile_pos_in_workspace_view: Some((1200.0, 700.0)),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn auto_pip_emits_float_resize_and_place() {
        let mut engine = engine();
        let effects = engine.handle_event(CompositorEvent::WindowOpenedOrChanged(pip(42, 1)));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            Effect::Action(CompositorAction::MoveWindowToFloating { id: 42 })
        )));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            Effect::Action(CompositorAction::SetWindowWidth { id: 42, .. })
        )));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            Effect::Action(CompositorAction::MoveFloatingWindow { id: 42, .. })
        )));
    }

    #[test]
    fn workspace_follow_uses_focus_false_and_deduplicates() {
        let mut engine = engine();
        engine.handle_event(CompositorEvent::WindowOpenedOrChanged(pip(42, 1)));
        engine.handle_event(CompositorEvent::WorkspaceActivated {
            id: 2,
            focused: true,
        });
        let first = engine.reconcile_workspace_follow();
        assert!(first.iter().any(|effect| matches!(
            effect,
            Effect::Action(CompositorAction::MoveWindowToWorkspace {
                window_id: 42,
                workspace_id: 2,
                focus: false
            })
        )));
        let duplicate = engine.reconcile_workspace_follow();
        assert!(!duplicate.iter().any(|effect| matches!(
            effect,
            Effect::Action(CompositorAction::MoveWindowToWorkspace { .. })
        )));
    }

    #[test]
    fn follow_can_be_disabled_per_window() {
        let mut engine = engine();
        engine.handle_event(CompositorEvent::WindowOpenedOrChanged(pip(42, 1)));
        engine.set_follow(Some(42), false).unwrap();
        engine.handle_event(CompositorEvent::WorkspaceActivated {
            id: 2,
            focused: true,
        });
        assert!(engine.reconcile_workspace_follow().is_empty());
    }

    #[test]
    fn startup_snapshot_adopts_live_manual_geometry_over_stale_profile() {
        let mut engine = engine();
        engine.persistent.profiles.insert(
            "chromium-empty-app-id".into(),
            RememberedGeometry {
                width: 994,
                height: 600,
                x_percent: 80.0,
                y_percent: 70.0,
            },
        );

        let mut live = pip(42, 1);
        live.is_floating = true;
        live.layout.window_size = (782, 419);
        live.layout.tile_size = (782.0, 419.0);
        live.layout.tile_pos_in_workspace_view = Some((1010.0, 520.0));

        let effects = engine.handle_event(CompositorEvent::WindowsChanged(vec![live]));

        assert!(!effects.iter().any(|effect| matches!(
            effect,
            Effect::Action(CompositorAction::SetWindowWidth { id: 42, .. })
                | Effect::Action(CompositorAction::SetWindowHeight { id: 42, .. })
        )));
        assert!(!effects.iter().any(|effect| matches!(
            effect,
            Effect::Action(CompositorAction::MoveFloatingWindow { id: 42, .. })
        )));
        assert_eq!(engine.desired_size(42), (782, 419));
        let remembered = engine.persistent.profiles["chromium-empty-app-id"];
        assert_eq!((remembered.width, remembered.height), (782, 419));
    }

    #[test]
    fn startup_snapshot_still_enforces_explicit_geometry_lock() {
        let mut engine = engine();
        engine.persistent.profiles.insert(
            "chromium-empty-app-id".into(),
            RememberedGeometry {
                width: 994,
                height: 600,
                x_percent: 80.0,
                y_percent: 70.0,
            },
        );
        engine.persistent.controls.insert(
            "chromium-empty-app-id".into(),
            RememberedControls {
                geometry_locked: true,
                ..Default::default()
            },
        );

        let mut live = pip(42, 1);
        live.is_floating = true;
        live.layout.window_size = (782, 419);
        live.layout.tile_size = (782.0, 419.0);

        let effects = engine.handle_event(CompositorEvent::WindowsChanged(vec![live]));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            Effect::Action(CompositorAction::SetWindowWidth {
                id: 42,
                change: SizeChange::SetFixed(994)
            })
        )));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            Effect::Action(CompositorAction::SetWindowHeight {
                id: 42,
                change: SizeChange::SetFixed(600)
            })
        )));
    }

    #[test]
    fn manual_resize_is_persisted_for_auto_pip() {
        let mut engine = engine();
        engine.handle_event(CompositorEvent::WindowOpenedOrChanged(pip(42, 1)));
        let effects = engine.resize(Some(42), 1131, 636).unwrap();
        assert!(effects.iter().any(|effect| matches!(
            effect,
            Effect::Action(CompositorAction::SetWindowWidth {
                id: 42,
                change: SizeChange::SetFixed(1131)
            })
        )));
        assert_eq!(
            engine.persistent.profiles["chromium-empty-app-id"].width,
            1131
        );
        assert_eq!(
            engine.persistent.profiles["chromium-empty-app-id"].height,
            636
        );
    }

    #[test]
    fn relative_scale_uses_the_current_free_form_size() {
        let mut engine = engine();
        engine.handle_event(CompositorEvent::WindowOpenedOrChanged(pip(42, 1)));
        engine.resize(Some(42), 1131, 636).unwrap();
        let effects = engine.scale(Some(42), 10).unwrap();
        assert!(effects.iter().any(|effect| matches!(
            effect,
            Effect::Action(CompositorAction::SetWindowWidth {
                id: 42,
                change: SizeChange::SetFixed(1244)
            })
        )));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            Effect::Action(CompositorAction::SetWindowHeight {
                id: 42,
                change: SizeChange::SetFixed(700)
            })
        )));
    }

    #[test]
    fn position_preset_updates_remembered_coordinates_without_losing_size() {
        let mut engine = engine();
        engine.handle_event(CompositorEvent::WindowOpenedOrChanged(pip(42, 1)));
        engine.resize(Some(42), 1131, 636).unwrap();
        let effects = engine.set_position(Some(42), Placement::TopLeft).unwrap();
        assert!(effects.iter().any(|effect| matches!(
            effect,
            Effect::Action(CompositorAction::MoveFloatingWindow { id: 42, .. })
        )));
        let remembered = engine.persistent.profiles["chromium-empty-app-id"];
        assert_eq!((remembered.width, remembered.height), (1131, 636));
    }

    #[test]
    fn geometry_lock_reapplies_changed_size() {
        let mut engine = engine();
        engine.handle_event(CompositorEvent::WindowOpenedOrChanged(pip(42, 1)));
        if let Some(tracked) = engine.tracked.get_mut(&42) {
            tracked.suppress_geometry_until = Instant::now() - Duration::from_secs(1);
        }
        engine.set_geometry_lock(Some(42), true).unwrap();
        if let Some(tracked) = engine.tracked.get_mut(&42) {
            tracked.suppress_geometry_until = Instant::now() - Duration::from_secs(1);
        }
        let effects = engine.handle_event(CompositorEvent::WindowLayoutsChanged(vec![(
            42,
            WindowLayout {
                window_size: (800, 450),
                tile_size: (800.0, 450.0),
                tile_pos_in_workspace_view: Some((900.0, 500.0)),
                ..Default::default()
            },
        )]));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            Effect::Action(CompositorAction::SetWindowWidth { id: 42, .. })
        )));
    }

    #[test]
    fn manual_pin_does_not_resize_kitty_and_unpin_restores_tiling() {
        let mut engine = engine();
        let kitty = WindowInfo {
            id: 7,
            title: Some("shell".into()),
            app_id: Some("kitty".into()),
            workspace_id: Some(1),
            is_focused: true,
            layout: WindowLayout {
                window_size: (900, 700),
                tile_size: (900.0, 700.0),
                ..Default::default()
            },
            ..Default::default()
        };
        engine.handle_event(CompositorEvent::WindowOpenedOrChanged(kitty));
        let pin = engine.pin(None).unwrap();
        assert!(pin.iter().any(|effect| matches!(
            effect,
            Effect::Action(CompositorAction::MoveWindowToFloating { id: 7 })
        )));
        assert!(!pin.iter().any(|effect| matches!(
            effect,
            Effect::Action(CompositorAction::SetWindowWidth { .. })
        )));
        let unpin = engine.unpin(Some(7)).unwrap();
        assert!(unpin.iter().any(|effect| matches!(
            effect,
            Effect::Action(CompositorAction::MoveWindowToTiling { id: 7 })
        )));
    }

    #[test]
    fn close_removes_stale_tracked_window() {
        let mut engine = engine();
        engine.handle_event(CompositorEvent::WindowOpenedOrChanged(pip(42, 1)));
        assert_eq!(engine.status_snapshot().tracked, 1);
        engine.handle_event(CompositorEvent::WindowClosed { id: 42 });
        assert_eq!(engine.status_snapshot().tracked, 0);
    }

    #[test]
    fn late_metadata_can_classify_pip() {
        let mut engine = engine();
        let mut pending = pip(42, 1);
        pending.title = None;
        assert!(engine
            .handle_event(CompositorEvent::WindowOpenedOrChanged(pending.clone()))
            .is_empty());
        pending.title = Some("Picture in picture".into());
        let effects = engine.handle_event(CompositorEvent::WindowOpenedOrChanged(pending));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            Effect::Action(CompositorAction::MoveWindowToFloating { id: 42 })
        )));
    }

    #[test]
    fn restores_previous_focus_when_new_pip_stole_focus() {
        let mut engine = engine();
        engine.handle_event(CompositorEvent::WindowOpenedOrChanged(WindowInfo {
            id: 7,
            title: Some("editor".into()),
            app_id: Some("code".into()),
            workspace_id: Some(1),
            is_focused: true,
            ..Default::default()
        }));
        let mut video = pip(42, 1);
        video.is_focused = true;
        let effects = engine.handle_event(CompositorEvent::WindowOpenedOrChanged(video));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            Effect::Action(CompositorAction::FocusWindow { id: 7 })
        )));
    }

    #[test]
    fn opacity_auto_and_fixed_are_persisted() {
        let mut engine = engine();
        engine.set_opacity_override_percent(Some(80)).unwrap();
        assert_eq!(engine.opacity_override_percent(), Some(80));
        engine.set_opacity_override_percent(None).unwrap();
        assert_eq!(engine.opacity_override_percent(), None);
    }

    #[test]
    fn manual_pin_preset_does_not_change_browser_pip_opacity_policy() {
        let mut engine = engine();
        engine.set_opacity_override_percent(Some(80)).unwrap();
        engine.handle_event(CompositorEvent::WindowOpenedOrChanged(WindowInfo {
            id: 7,
            title: Some("shell".into()),
            app_id: Some("kitty".into()),
            workspace_id: Some(1),
            is_focused: true,
            layout: WindowLayout {
                window_size: (900, 700),
                tile_size: (900.0, 700.0),
                tile_pos_in_workspace_view: Some((200.0, 100.0)),
                ..Default::default()
            },
            ..Default::default()
        }));
        engine.pin(Some(7)).unwrap();
        let application = engine.apply_preset(Some(7), ControlPreset::Small).unwrap();
        assert_eq!(application.opacity_percent, Some(80));
        assert_eq!(engine.opacity_override_percent(), Some(80));
    }

    #[test]
    fn transient_disconnect_preserves_tracking_until_authoritative_snapshot() {
        let mut engine = engine();
        engine.handle_event(CompositorEvent::WindowOpenedOrChanged(pip(42, 1)));
        engine.handle_event(CompositorEvent::Disconnected {
            reason: "test".into(),
        });
        assert_eq!(engine.status_snapshot().tracked, 1);
        assert!(!engine.status_snapshot().niri_connected);
        engine.handle_event(CompositorEvent::WindowsChanged(Vec::new()));
        assert!(engine.tracked_snapshots().is_empty());
    }
}
