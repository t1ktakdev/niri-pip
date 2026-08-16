use niripip_core::{CompositorAction, CompositorEvent};
use niripip_ipc::{mock::MockNiriBackend, NiriBackend};

#[tokio::test]
async fn mock_backend_replays_events_and_records_actions() {
    let backend =
        MockNiriBackend::default().with_events(vec![CompositorEvent::WindowClosed { id: 9 }]);
    let mut events = backend.subscribe().await.expect("mock subscribe");
    assert_eq!(
        events
            .recv()
            .await
            .expect("mock event")
            .expect("successful mock event"),
        CompositorEvent::WindowClosed { id: 9 }
    );

    backend
        .execute(CompositorAction::MoveWindowToFloating { id: 9 })
        .await
        .expect("mock action");
    assert_eq!(
        backend.actions(),
        vec![CompositorAction::MoveWindowToFloating { id: 9 }]
    );
}
