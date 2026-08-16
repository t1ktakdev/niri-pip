use crate::NiriBackend;
use async_trait::async_trait;
use niripip_core::{
    CompositorAction, CompositorEvent, LogicalOutput, OutputInfo, PositionChange, SizeChange,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc;
use tracing::{debug, trace};

#[derive(Debug, Error, Clone)]
pub enum NiriIpcError {
    #[error("NIRI_SOCKET is not set; start niri-pip inside a Niri session")]
    MissingSocket,
    #[error("cannot connect to Niri IPC socket {path}: {message}")]
    Connect { path: PathBuf, message: String },
    #[error("Niri IPC I/O error: {0}")]
    Io(String),
    #[error("invalid Niri IPC JSON: {0}")]
    Json(String),
    #[error("Niri IPC returned an error: {0}")]
    Remote(String),
    #[error("unexpected Niri IPC response: {0}")]
    Unexpected(String),
    #[error("Niri event stream ended")]
    StreamEnded,
}

impl From<std::io::Error> for NiriIpcError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

impl From<serde_json::Error> for NiriIpcError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct RealNiriBackend {
    socket_path: PathBuf,
}

impl RealNiriBackend {
    pub fn from_env() -> Result<Self, NiriIpcError> {
        let path = env::var_os("NIRI_SOCKET").ok_or(NiriIpcError::MissingSocket)?;
        Ok(Self {
            socket_path: PathBuf::from(path),
        })
    }

    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
        }
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    async fn connect(&self) -> Result<UnixStream, NiriIpcError> {
        UnixStream::connect(&self.socket_path)
            .await
            .map_err(|err| NiriIpcError::Connect {
                path: self.socket_path.clone(),
                message: err.to_string(),
            })
    }

    async fn request(&self, request: Value) -> Result<Value, NiriIpcError> {
        let mut stream = self.connect().await?;
        let mut payload = serde_json::to_vec(&request)?;
        payload.push(b'\n');
        stream.write_all(&payload).await?;
        stream.flush().await?;
        stream.shutdown().await?;

        let mut lines = BufReader::new(stream).lines();
        let line = lines
            .next_line()
            .await?
            .ok_or_else(|| NiriIpcError::Unexpected("empty response".into()))?;
        parse_reply(&line)
    }
}

#[async_trait]
impl NiriBackend for RealNiriBackend {
    async fn version(&self) -> Result<String, NiriIpcError> {
        match self.request(json!("Version")).await? {
            Value::Object(mut obj) => match obj.remove("Version") {
                Some(Value::String(version)) => Ok(version),
                other => Err(NiriIpcError::Unexpected(format!(
                    "Version payload: {other:?}"
                ))),
            },
            other => Err(NiriIpcError::Unexpected(format!(
                "Version response: {other}"
            ))),
        }
    }

    async fn outputs(&self) -> Result<HashMap<String, OutputInfo>, NiriIpcError> {
        let value = self.request(json!("Outputs")).await?;
        let raw = value
            .get("Outputs")
            .cloned()
            .ok_or_else(|| NiriIpcError::Unexpected(format!("Outputs response: {value}")))?;
        let map: HashMap<String, WireOutput> = serde_json::from_value(raw)?;
        Ok(map
            .into_iter()
            .map(|(name, output)| {
                let logical = output.logical.map(|logical| LogicalOutput {
                    x: logical.x,
                    y: logical.y,
                    width: logical.width,
                    height: logical.height,
                    scale: logical.scale,
                });
                (name.clone(), OutputInfo { name, logical })
            })
            .collect())
    }

    async fn execute(&self, action: CompositorAction) -> Result<(), NiriIpcError> {
        let request = action_request(action);
        trace!(request = %request, "sending Niri action");
        let response = self.request(request).await?;
        match response {
            Value::String(ref handled) if handled == "Handled" => Ok(()),
            other => Err(NiriIpcError::Unexpected(format!(
                "action response: {other}"
            ))),
        }
    }

    async fn subscribe(
        &self,
    ) -> Result<mpsc::Receiver<Result<CompositorEvent, NiriIpcError>>, NiriIpcError> {
        let stream = self.connect().await?;
        let (read_half, mut write_half) = stream.into_split();
        write_half.write_all(b"\"EventStream\"\n").await?;
        write_half.flush().await?;
        write_half.shutdown().await?;

        let mut lines = BufReader::new(read_half).lines();
        let ack = lines.next_line().await?.ok_or_else(|| {
            NiriIpcError::Unexpected("event stream returned no acknowledgement".into())
        })?;
        let ack_value = parse_reply(&ack)?;
        if ack_value != Value::String("Handled".into()) {
            return Err(NiriIpcError::Unexpected(format!(
                "event stream acknowledgement: {ack_value}"
            )));
        }
        debug!(socket = %self.socket_path.display(), "subscribed to Niri event stream");

        let (tx, rx) = mpsc::channel(256);
        tokio::spawn(async move {
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        let parsed = parse_event_line(&line);
                        if tx.send(parsed).await.is_err() {
                            break;
                        }
                    }
                    Ok(None) => {
                        let _ = tx.send(Err(NiriIpcError::StreamEnded)).await;
                        break;
                    }
                    Err(err) => {
                        let _ = tx.send(Err(NiriIpcError::Io(err.to_string()))).await;
                        break;
                    }
                }
            }
        });
        Ok(rx)
    }
}

fn parse_reply(line: &str) -> Result<Value, NiriIpcError> {
    let value: Value = serde_json::from_str(line)?;
    let obj = value
        .as_object()
        .ok_or_else(|| NiriIpcError::Unexpected(line.to_owned()))?;
    if let Some(ok) = obj.get("Ok") {
        return Ok(ok.clone());
    }
    if let Some(err) = obj.get("Err") {
        return Err(NiriIpcError::Remote(
            err.as_str().unwrap_or("unknown Niri error").to_owned(),
        ));
    }
    Err(NiriIpcError::Unexpected(line.to_owned()))
}

pub fn action_request(action: CompositorAction) -> Value {
    let body = match action {
        CompositorAction::FocusWindow { id } => json!({"FocusWindow": {"id": id}}),
        CompositorAction::MoveWindowToFloating { id } => {
            json!({"MoveWindowToFloating": {"id": id}})
        }
        CompositorAction::MoveWindowToTiling { id } => {
            json!({"MoveWindowToTiling": {"id": id}})
        }
        CompositorAction::SetWindowWidth { id, change } => {
            json!({"SetWindowWidth": {"id": id, "change": size_change(change)}})
        }
        CompositorAction::SetWindowHeight { id, change } => {
            json!({"SetWindowHeight": {"id": id, "change": size_change(change)}})
        }
        CompositorAction::MoveFloatingWindow { id, x, y } => json!({
            "MoveFloatingWindow": {
                "id": id,
                "x": position_change(x),
                "y": position_change(y)
            }
        }),
        CompositorAction::MoveWindowToWorkspace {
            window_id,
            workspace_id,
            focus,
        } => json!({
            "MoveWindowToWorkspace": {
                "window_id": window_id,
                "reference": {"Id": workspace_id},
                "focus": focus
            }
        }),
    };
    json!({"Action": body})
}

fn size_change(change: SizeChange) -> Value {
    match change {
        SizeChange::SetFixed(v) => json!({"SetFixed": v}),
        SizeChange::SetProportion(v) => json!({"SetProportion": v}),
        SizeChange::AdjustFixed(v) => json!({"AdjustFixed": v}),
        SizeChange::AdjustProportion(v) => json!({"AdjustProportion": v}),
    }
}

fn position_change(change: PositionChange) -> Value {
    match change {
        PositionChange::SetFixed(v) => json!({"SetFixed": v}),
        PositionChange::SetProportion(v) => json!({"SetProportion": v}),
        PositionChange::AdjustFixed(v) => json!({"AdjustFixed": v}),
        PositionChange::AdjustProportion(v) => json!({"AdjustProportion": v}),
    }
}

pub fn parse_event_line(line: &str) -> Result<CompositorEvent, NiriIpcError> {
    let value: Value = serde_json::from_str(line)?;
    let obj = value
        .as_object()
        .ok_or_else(|| NiriIpcError::Unexpected(format!("event is not an object: {line}")))?;
    if obj.len() != 1 {
        return Err(NiriIpcError::Unexpected(format!(
            "event must contain one top-level variant: {line}"
        )));
    }
    let Some((name, payload)) = obj.iter().next() else {
        return Err(NiriIpcError::Unexpected("empty event object".into()));
    };

    match name.as_str() {
        "WorkspacesChanged" => Ok(CompositorEvent::WorkspacesChanged(deserialize_field(
            payload,
            "workspaces",
        )?)),
        "WorkspaceActivated" => Ok(CompositorEvent::WorkspaceActivated {
            id: field_u64(payload, "id")?,
            focused: field_bool(payload, "focused")?,
        }),
        "WorkspaceActiveWindowChanged" => Ok(CompositorEvent::WorkspaceActiveWindowChanged {
            workspace_id: field_u64(payload, "workspace_id")?,
            active_window_id: optional_u64(payload, "active_window_id")?,
        }),
        "WindowsChanged" => Ok(CompositorEvent::WindowsChanged(deserialize_field(
            payload, "windows",
        )?)),
        "WindowOpenedOrChanged" => Ok(CompositorEvent::WindowOpenedOrChanged(deserialize_field(
            payload, "window",
        )?)),
        "WindowClosed" => Ok(CompositorEvent::WindowClosed {
            id: field_u64(payload, "id")?,
        }),
        "WindowFocusChanged" => Ok(CompositorEvent::WindowFocusChanged {
            id: optional_u64(payload, "id")?,
        }),
        "WindowLayoutsChanged" => Ok(CompositorEvent::WindowLayoutsChanged(deserialize_field(
            payload, "changes",
        )?)),
        other => Ok(CompositorEvent::Unknown(other.to_owned())),
    }
}

fn deserialize_field<T: for<'de> Deserialize<'de>>(
    value: &Value,
    field: &str,
) -> Result<T, NiriIpcError> {
    let field_value = value
        .get(field)
        .cloned()
        .ok_or_else(|| NiriIpcError::Unexpected(format!("missing event field '{field}'")))?;
    Ok(serde_json::from_value(field_value)?)
}

fn field_u64(value: &Value, field: &str) -> Result<u64, NiriIpcError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| NiriIpcError::Unexpected(format!("missing/invalid u64 field '{field}'")))
}

fn field_bool(value: &Value, field: &str) -> Result<bool, NiriIpcError> {
    value
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| NiriIpcError::Unexpected(format!("missing/invalid bool field '{field}'")))
}

fn optional_u64(value: &Value, field: &str) -> Result<Option<u64>, NiriIpcError> {
    match value.get(field) {
        Some(Value::Null) | None => Ok(None),
        Some(value) => value.as_u64().map(Some).ok_or_else(|| {
            NiriIpcError::Unexpected(format!("invalid optional u64 field '{field}'"))
        }),
    }
}

#[derive(Debug, Deserialize)]
struct WireOutput {
    #[serde(default)]
    logical: Option<WireLogicalOutput>,
}

#[derive(Debug, Deserialize)]
struct WireLogicalOutput {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_empty_app_id_window_event() {
        let line = r#"{"WindowOpenedOrChanged":{"window":{"id":42,"title":"Picture in picture","app_id":"","pid":123,"workspace_id":3,"is_focused":false,"is_floating":false,"is_urgent":false,"layout":{"pos_in_scrolling_layout":null,"tile_size":[480.0,270.0],"window_size":[480,270],"tile_pos_in_workspace_view":null,"window_offset_in_tile":[0.0,0.0]},"focus_timestamp":null}}}"#;
        match parse_event_line(line).expect("valid window event") {
            CompositorEvent::WindowOpenedOrChanged(window) => {
                assert_eq!(window.id, 42);
                assert_eq!(window.app_id.as_deref(), Some(""));
                assert_eq!(window.title.as_deref(), Some("Picture in picture"));
            }
            other => panic!("wrong event: {other:?}"),
        }
    }

    #[test]
    fn unknown_event_is_forward_compatible() {
        let event = parse_event_line(r#"{"FutureNiriEvent":{"thing":1}}"#)
            .expect("unknown events are valid input");
        assert_eq!(event, CompositorEvent::Unknown("FutureNiriEvent".into()));
    }

    #[test]
    fn workspace_move_json_is_exact_and_focus_false() {
        let value = action_request(CompositorAction::MoveWindowToWorkspace {
            window_id: 42,
            workspace_id: 7,
            focus: false,
        });
        assert_eq!(
            value,
            json!({"Action":{"MoveWindowToWorkspace":{
                "window_id":42,
                "reference":{"Id":7},
                "focus":false
            }}})
        );
    }

    #[test]
    fn resize_json_uses_set_fixed() {
        let value = action_request(CompositorAction::SetWindowWidth {
            id: 42,
            change: SizeChange::SetFixed(480),
        });
        assert_eq!(
            value,
            json!({"Action":{"SetWindowWidth":{"id":42,"change":{"SetFixed":480}}}})
        );
    }
}
