use crate::{FollowMode, Placement};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const STATE_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PersistentState {
    pub schema_version: u32,
    pub profiles: HashMap<String, RememberedGeometry>,
    pub controls: HashMap<String, RememberedControls>,
    /// `Some(100)` forces PiP fully opaque. `None` inherits normal Niri/iNiR rules.
    pub pip_opacity_percent: Option<u8>,
}

impl Default for PersistentState {
    fn default() -> Self {
        Self {
            schema_version: STATE_SCHEMA_VERSION,
            profiles: HashMap::new(),
            controls: HashMap::new(),
            pip_opacity_percent: Some(100),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct RememberedGeometry {
    pub width: u32,
    pub height: u32,
    pub x_percent: f64,
    pub y_percent: f64,
}

impl Default for RememberedGeometry {
    fn default() -> Self {
        Self {
            width: 480,
            height: 270,
            x_percent: 70.0,
            y_percent: 70.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct RememberedControls {
    pub placement: Placement,
    pub follow_enabled: bool,
    pub follow_mode: FollowMode,
    pub geometry_locked: bool,
}

impl Default for RememberedControls {
    fn default() -> Self {
        Self {
            placement: Placement::BottomRight,
            follow_enabled: true,
            follow_mode: FollowMode::FollowWorkspace,
            geometry_locked: false,
        }
    }
}

#[derive(Debug, Error)]
pub enum StateError {
    #[error("cannot read state {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid state JSON in {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("cannot write state {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot serialize runtime state: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("unsupported state schema version {found}; this build supports versions 1 and 2")]
    UnsupportedSchema { found: u32 },
}

impl PersistentState {
    pub fn load(path: &Path) -> Result<Self, StateError> {
        let mut state: Self = match fs::read_to_string(path) {
            Ok(data) => serde_json::from_str(&data).map_err(|source| StateError::Parse {
                path: path.to_path_buf(),
                source,
            })?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(source) => {
                return Err(StateError::Read {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };

        match state.schema_version {
            1 => {
                // v0.1 stored only geometry. Preserve every learned size/position and add the
                // controller defaults instead of throwing the user's real PiP geometry away.
                state.schema_version = STATE_SCHEMA_VERSION;
                state.controls = HashMap::new();
                if state.pip_opacity_percent.is_none() {
                    state.pip_opacity_percent = Some(100);
                }
            }
            STATE_SCHEMA_VERSION => {}
            found => return Err(StateError::UnsupportedSchema { found }),
        }

        Ok(state)
    }

    pub fn save_atomic(&self, path: &Path) -> Result<(), StateError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| StateError::Write {
                path: parent.to_path_buf(),
                source,
            })?;
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).map_err(|source| {
                StateError::Write {
                    path: parent.to_path_buf(),
                    source,
                }
            })?;
        }
        let tmp = path.with_extension("json.tmp");
        let data = serde_json::to_vec_pretty(self)?;
        let mut file = fs::File::create(&tmp).map_err(|source| StateError::Write {
            path: tmp.clone(),
            source,
        })?;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|source| StateError::Write {
                path: tmp.clone(),
                source,
            })?;
        file.write_all(&data).map_err(|source| StateError::Write {
            path: tmp.clone(),
            source,
        })?;
        file.sync_all().map_err(|source| StateError::Write {
            path: tmp.clone(),
            source,
        })?;
        fs::rename(&tmp, path).map_err(|source| StateError::Write {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(())
    }
}

pub fn state_path() -> PathBuf {
    if let Some(base) = std::env::var_os("XDG_STATE_HOME") {
        return PathBuf::from(base).join("niri-pip/state.json");
    }
    let home = std::env::var_os("HOME").unwrap_or_else(|| ".".into());
    PathBuf::from(home).join(".local/state/niri-pip/state.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_state_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("niri-pip-{name}-{nonce}.json"))
    }

    #[test]
    fn rejects_unknown_state_schema() {
        let path = temp_state_path("unsupported-schema");
        fs::write(&path, r#"{"schema_version":3,"profiles":{},"controls":{}}"#)
            .expect("write test state");

        let err = PersistentState::load(&path).expect_err("schema version 3 must be rejected");
        assert!(matches!(err, StateError::UnsupportedSchema { found: 3 }));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn migrates_v1_geometry_without_losing_it() {
        let path = temp_state_path("v1-migration");
        fs::write(
            &path,
            r#"{"schema_version":1,"profiles":{"chromium-empty-app-id":{"width":1131,"height":636,"x_percent":60.0,"y_percent":30.0}}}"#,
        )
        .expect("write v1 state");

        let state = PersistentState::load(&path).expect("v1 should migrate");
        assert_eq!(state.schema_version, STATE_SCHEMA_VERSION);
        assert_eq!(
            state.profiles["chromium-empty-app-id"].width, 1131,
            "manual PiP size must survive migration"
        );
        assert_eq!(state.pip_opacity_percent, Some(100));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn missing_state_uses_current_schema() {
        let path = temp_state_path("missing");
        let state = PersistentState::load(&path).expect("missing state should use defaults");
        assert_eq!(state.schema_version, STATE_SCHEMA_VERSION);
        assert!(state.profiles.is_empty());
        assert_eq!(state.pip_opacity_percent, Some(100));
    }
}
