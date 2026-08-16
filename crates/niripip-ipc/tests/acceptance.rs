use niripip_core::{
    CompositorAction, CompositorEvent, Config, Effect, Engine, LogicalOutput, OutputInfo,
    PersistentState, WindowInfo, WindowLayout, WorkspaceInfo,
};
use niripip_ipc::{mock::MockNiriBackend, NiriBackend};
use std::collections::HashMap;

fn window(
    id: u64,
    title: &str,
    app_id: &str,
    workspace_id: u64,
    size: (i32, i32),
    focused: bool,
) -> WindowInfo {
    WindowInfo {
        id,
        title: Some(title.into()),
        app_id: Some(app_id.into()),
        workspace_id: Some(workspace_id),
        is_focused: focused,
        is_floating: false,
        layout: WindowLayout {
            tile_size: (size.0 as f64, size.1 as f64),
            window_size: size,
            tile_pos_in_workspace_view: Some((1100.0, 650.0)),
            ..WindowLayout::default()
        },
        ..WindowInfo::default()
    }
}

fn workspace(id: u64, idx: u8, focused: bool) -> WorkspaceInfo {
    WorkspaceInfo {
        id,
        idx,
        output: Some("DP-1".into()),
        is_active: focused,
        is_focused: focused,
        ..WorkspaceInfo::default()
    }
}

fn outputs() -> HashMap<String, OutputInfo> {
    HashMap::from([(
        "DP-1".into(),
        OutputInfo {
            name: "DP-1".into(),
            logical: Some(LogicalOutput {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                scale: 1.0,
            }),
        },
    )])
}

async fn apply(backend: &MockNiriBackend, effects: Vec<Effect>) {
    for effect in effects {
        let Effect::Action(action) = effect;
        backend.execute(action).await.expect("mock action");
    }
}

#[tokio::test]
async fn mvp_chromium_pip_follow_close_then_generic_pin_unpin() {
    let backend = MockNiriBackend::default().with_outputs(outputs());
    let mut engine =
        Engine::new(Config::default(), PersistentState::default()).expect("valid default engine");

    engine.handle_event(CompositorEvent::Connected {
        version: "26.04".into(),
    });
    engine.handle_event(CompositorEvent::OutputsChanged(outputs()));
    engine.handle_event(CompositorEvent::WorkspacesChanged(vec![
        workspace(1, 1, true),
        workspace(2, 2, false),
    ]));

    // The acceptance case: Chromium/XWayland PiP can expose an empty app-id.
    let effects = engine.handle_event(CompositorEvent::WindowOpenedOrChanged(window(
        42,
        "Picture in picture",
        "",
        1,
        (480, 270),
        false,
    )));
    assert!(effects.iter().any(|effect| {
        matches!(
            effect,
            Effect::Action(CompositorAction::MoveWindowToFloating { id: 42 })
        )
    }));
    assert!(effects.iter().any(|effect| {
        matches!(
            effect,
            Effect::Action(CompositorAction::SetWindowWidth { id: 42, .. })
        )
    }));
    assert!(!effects.iter().any(|effect| {
        matches!(
            effect,
            Effect::Action(CompositorAction::FocusWindow { id: 42 })
        )
    }));
    apply(&backend, effects).await;

    engine.handle_event(CompositorEvent::WorkspaceActivated {
        id: 2,
        focused: true,
    });
    let follow = engine.reconcile_workspace_follow();
    assert!(follow.iter().any(|effect| {
        matches!(
            effect,
            Effect::Action(CompositorAction::MoveWindowToWorkspace {
                window_id: 42,
                workspace_id: 2,
                focus: false,
            })
        )
    }));
    apply(&backend, follow).await;

    engine.handle_event(CompositorEvent::WindowClosed { id: 42 });
    assert!(engine.tracked_snapshots().is_empty());

    // Generic pinning keeps the arbitrary app's size and only changes floating/sticky state.
    engine.handle_event(CompositorEvent::WindowOpenedOrChanged(window(
        7,
        "shell",
        "kitty",
        2,
        (900, 700),
        true,
    )));
    let pin = engine.pin(None).expect("pin focused Kitty");
    assert!(pin.iter().any(|effect| {
        matches!(
            effect,
            Effect::Action(CompositorAction::MoveWindowToFloating { id: 7 })
        )
    }));
    assert!(!pin.iter().any(|effect| {
        matches!(
            effect,
            Effect::Action(
                CompositorAction::SetWindowWidth { id: 7, .. }
                    | CompositorAction::SetWindowHeight { id: 7, .. }
            )
        )
    }));
    apply(&backend, pin).await;

    let unpin = engine.unpin(Some(7)).expect("unpin Kitty");
    assert!(unpin.iter().any(|effect| {
        matches!(
            effect,
            Effect::Action(CompositorAction::MoveWindowToTiling { id: 7 })
        )
    }));
    apply(&backend, unpin).await;

    let actions = backend.actions();
    assert!(actions.iter().any(|action| {
        matches!(
            action,
            CompositorAction::MoveWindowToWorkspace {
                window_id: 42,
                workspace_id: 2,
                focus: false,
            }
        )
    }));
}

#[tokio::test]
async fn controller_preserves_free_resize_and_follow_is_runtime_switchable() {
    let backend = MockNiriBackend::default().with_outputs(outputs());
    let mut engine =
        Engine::new(Config::default(), PersistentState::default()).expect("valid default engine");
    engine.handle_event(CompositorEvent::Connected {
        version: "26.04".into(),
    });
    engine.handle_event(CompositorEvent::OutputsChanged(outputs()));
    engine.handle_event(CompositorEvent::WorkspacesChanged(vec![
        workspace(1, 1, true),
        workspace(2, 2, false),
    ]));
    apply(
        &backend,
        engine.handle_event(CompositorEvent::WindowOpenedOrChanged(window(
            42,
            "Picture in picture",
            "",
            1,
            (500, 281),
            false,
        ))),
    )
    .await;

    // Exact arbitrary dimensions are a first-class controller operation, not a preset-only UI.
    let resize = engine.resize(Some(42), 1131, 636).expect("free resize");
    assert!(resize.iter().any(|effect| matches!(
        effect,
        Effect::Action(CompositorAction::SetWindowWidth {
            id: 42,
            change: niripip_core::SizeChange::SetFixed(1131)
        })
    )));
    assert!(resize.iter().any(|effect| matches!(
        effect,
        Effect::Action(CompositorAction::SetWindowHeight {
            id: 42,
            change: niripip_core::SizeChange::SetFixed(636)
        })
    )));
    apply(&backend, resize).await;

    engine.set_follow(Some(42), false).expect("disable follow");
    engine.handle_event(CompositorEvent::WorkspaceActivated {
        id: 2,
        focused: true,
    });
    assert!(engine.reconcile_workspace_follow().is_empty());

    let follow = engine.set_follow(Some(42), true).expect("enable follow");
    assert!(follow.iter().any(|effect| matches!(
        effect,
        Effect::Action(CompositorAction::MoveWindowToWorkspace {
            window_id: 42,
            workspace_id: 2,
            focus: false
        })
    )));
    apply(&backend, follow).await;

    engine
        .set_geometry_lock(Some(42), true)
        .expect("lock current PiP geometry");
    let snapshot = engine.status_snapshot();
    let pip = snapshot
        .windows
        .iter()
        .find(|window| window.id == 42)
        .unwrap();
    assert!(pip.follow_enabled);
    assert!(pip.geometry_locked);
    assert_eq!(snapshot.opacity_override_percent, Some(100));

    engine
        .set_geometry_lock(Some(42), false)
        .expect("unlock current PiP geometry");
    assert!(!engine.status_snapshot().windows[0].geometry_locked);
}
