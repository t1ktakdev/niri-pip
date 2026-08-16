use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot read config {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid TOML in {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("invalid configuration: {0}")]
    Validation(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub pip: PipConfig,
    pub margins: Margins,
    pub browsers: BrowserConfig,
    pub detectors: Vec<DetectorConfig>,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub enabled: bool,
    pub auto_detect: bool,
    pub follow_workspace: bool,
    pub follow_mode: FollowMode,
    pub remember_geometry: bool,
    pub detection_threshold: i32,
    pub restore_layout_on_unpin: bool,
    pub action_suppression_ms: u64,
    pub workspace_debounce_ms: u64,
    pub focus_restore_window_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PipConfig {
    pub position: Placement,
    pub position_mode: PositionMode,
    pub width: u32,
    pub height: u32,
    pub gap: u32,
    pub steal_focus: bool,
    pub preserve_aspect_ratio: bool,
    pub profile: SizeProfile,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Placement {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PositionMode {
    Fixed,
    Remember,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FollowMode {
    FollowWorkspace,
    FollowFocusedOutput,
    StayOnOutput,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SizeProfile {
    Tiny,
    Small,
    Medium,
    Large,
    Cinema,
    Custom,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct Margins {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BrowserConfig {
    pub chromium: bool,
    pub firefox: bool,
    pub brave: bool,
    pub vivaldi: bool,
    pub edge: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DetectorConfig {
    pub name: String,
    pub enabled: bool,
    pub action: DetectionAction,
    pub title_regex: Option<String>,
    pub app_id_regex: Option<String>,
    pub exclude_title_regex: Option<String>,
    pub exclude_app_id_regex: Option<String>,
    pub min_width: Option<u32>,
    pub max_width: Option<u32>,
    pub min_height: Option<u32>,
    pub max_height: Option<u32>,
    pub floating: Option<bool>,
    pub score: i32,
    pub compact_bonus: i32,
    pub aspect_16_9_bonus: i32,
    pub empty_app_id_bonus: i32,
    pub pid_present_bonus: i32,
    pub new_window_bonus: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DetectionAction {
    Pip,
    Ignore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    pub level: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_detect: true,
            follow_workspace: true,
            follow_mode: FollowMode::FollowWorkspace,
            remember_geometry: true,
            detection_threshold: 100,
            restore_layout_on_unpin: true,
            action_suppression_ms: 650,
            workspace_debounce_ms: 75,
            focus_restore_window_ms: 500,
        }
    }
}

impl Default for PipConfig {
    fn default() -> Self {
        Self {
            position: Placement::BottomRight,
            position_mode: PositionMode::Remember,
            width: 480,
            height: 270,
            gap: 18,
            steal_focus: false,
            preserve_aspect_ratio: true,
            profile: SizeProfile::Medium,
        }
    }
}

impl PipConfig {
    pub fn resolved_size(&self) -> (u32, u32) {
        match self.profile {
            SizeProfile::Tiny => (320, 180),
            SizeProfile::Small => (384, 216),
            SizeProfile::Medium => (480, 270),
            SizeProfile::Large => (640, 360),
            SizeProfile::Cinema => (960, 540),
            SizeProfile::Custom => (self.width, self.height),
        }
    }
}

impl Default for Margins {
    fn default() -> Self {
        Self {
            top: 18,
            right: 18,
            bottom: 18,
            left: 18,
        }
    }
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            chromium: true,
            firefox: true,
            brave: true,
            vivaldi: true,
            edge: true,
        }
    }
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            name: "unnamed".to_string(),
            enabled: true,
            action: DetectionAction::Pip,
            title_regex: None,
            app_id_regex: None,
            exclude_title_regex: None,
            exclude_app_id_regex: None,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            floating: None,
            score: 100,
            compact_bonus: 0,
            aspect_16_9_bonus: 0,
            empty_app_id_bonus: 0,
            pid_present_bonus: 0,
            new_window_bonus: 0,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            pip: PipConfig::default(),
            margins: Margins::default(),
            browsers: BrowserConfig::default(),
            detectors: default_detectors(),
            logging: LoggingConfig::default(),
        }
    }
}

pub fn default_detectors() -> Vec<DetectorConfig> {
    vec![
        DetectorConfig {
            name: "chromium-empty-app-id".into(),
            title_regex: Some(r"(?i)^picture(?:[ -]?in[ -]?)picture$".into()),
            app_id_regex: Some(r"^$".into()),
            score: 155,
            compact_bonus: 10,
            aspect_16_9_bonus: 10,
            empty_app_id_bonus: 20,
            new_window_bonus: 5,
            ..DetectorConfig::default()
        },
        DetectorConfig {
            name: "firefox-pip".into(),
            title_regex: Some(r"(?i)^picture-in-picture$".into()),
            app_id_regex: Some(r"(?i)^(firefox|org\.mozilla\.firefox)$".into()),
            score: 150,
            compact_bonus: 10,
            aspect_16_9_bonus: 10,
            new_window_bonus: 5,
            ..DetectorConfig::default()
        },
        DetectorConfig {
            name: "chromium-family-pip".into(),
            title_regex: Some(r"(?i)^picture(?:[ -]?in[ -]?)picture$".into()),
            app_id_regex: Some(r"(?i)(chrome|chromium|brave|vivaldi|edge)".into()),
            score: 145,
            compact_bonus: 10,
            aspect_16_9_bonus: 10,
            new_window_bonus: 5,
            ..DetectorConfig::default()
        },
        DetectorConfig {
            name: "generic-pip-title".into(),
            title_regex: Some(r"(?i)^picture(?:[ -]?in[ -]?)picture$".into()),
            max_width: Some(1280),
            max_height: Some(900),
            score: 105,
            compact_bonus: 5,
            aspect_16_9_bonus: 5,
            ..DetectorConfig::default()
        },
    ]
}

impl Config {
    pub fn from_toml(input: &str) -> Result<Self, ConfigError> {
        let cfg: Self = toml::from_str(input).map_err(|source| ConfigError::Parse {
            path: PathBuf::from("<memory>"),
            source,
        })?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let input = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let cfg: Self = toml::from_str(&input).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.general.detection_threshold < 0 {
            return Err(ConfigError::Validation(
                "general.detection_threshold must be >= 0".into(),
            ));
        }
        let (w, h) = self.pip.resolved_size();
        if w < 120 || h < 68 {
            return Err(ConfigError::Validation(
                "PiP size is implausibly small (minimum 120x68)".into(),
            ));
        }
        if w > i32::MAX as u32 || h > i32::MAX as u32 {
            return Err(ConfigError::Validation(
                "PiP width/height must fit Niri IPC signed 32-bit size values".into(),
            ));
        }
        if self.pip.steal_focus {
            return Err(ConfigError::Validation(
                "pip.steal_focus=true is intentionally unsupported; use false".into(),
            ));
        }
        if !matches!(
            self.logging.level.as_str(),
            "error" | "warn" | "info" | "debug" | "trace"
        ) {
            return Err(ConfigError::Validation(format!(
                "logging.level must be one of error, warn, info, debug, trace (got '{}')",
                self.logging.level
            )));
        }

        let mut names = HashSet::new();
        for detector in &self.detectors {
            if detector.name.trim().is_empty() {
                return Err(ConfigError::Validation(
                    "detector name must not be empty".into(),
                ));
            }
            if !names.insert(detector.name.as_str()) {
                return Err(ConfigError::Validation(format!(
                    "detector names must be unique (duplicate '{}')",
                    detector.name
                )));
            }
            if detector.title_regex.is_none()
                && detector.app_id_regex.is_none()
                && detector.min_width.is_none()
                && detector.max_width.is_none()
                && detector.min_height.is_none()
                && detector.max_height.is_none()
                && detector.floating.is_none()
            {
                return Err(ConfigError::Validation(format!(
                    "detector '{}' has no matching constraints",
                    detector.name
                )));
            }
            if detector
                .min_width
                .zip(detector.max_width)
                .is_some_and(|(min, max)| min > max)
            {
                return Err(ConfigError::Validation(format!(
                    "detector '{}': min_width must be <= max_width",
                    detector.name
                )));
            }
            if detector
                .min_height
                .zip(detector.max_height)
                .is_some_and(|(min, max)| min > max)
            {
                return Err(ConfigError::Validation(format!(
                    "detector '{}': min_height must be <= max_height",
                    detector.name
                )));
            }
            for (kind, pattern) in [
                ("title_regex", &detector.title_regex),
                ("app_id_regex", &detector.app_id_regex),
                ("exclude_title_regex", &detector.exclude_title_regex),
                ("exclude_app_id_regex", &detector.exclude_app_id_regex),
            ] {
                if let Some(pattern) = pattern {
                    regex::Regex::new(pattern).map_err(|err| {
                        ConfigError::Validation(format!(
                            "detector '{}': invalid {kind}: {err}",
                            detector.name
                        ))
                    })?;
                }
            }
        }
        Ok(())
    }
}

pub fn config_path() -> PathBuf {
    if let Some(base) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(base).join("niri-pip/config.toml");
    }
    let home = std::env::var_os("HOME").unwrap_or_else(|| ".".into());
    PathBuf::from(home).join(".config/niri-pip/config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_config_with_defaults() {
        let cfg = Config::from_toml("[general]\nenabled = true\n").unwrap();
        assert_eq!(cfg.pip.resolved_size(), (480, 270));
        assert!(cfg.general.auto_detect);
        assert!(!cfg.detectors.is_empty());
    }

    #[test]
    fn rejects_invalid_regex() {
        let input = r#"
[[detectors]]
name = "bad"
title_regex = "("
score = 100
"#;
        assert!(Config::from_toml(input).is_err());
    }

    #[test]
    fn rejects_duplicate_detector_names() {
        let input = r#"
[[detectors]]
name = "same"
title_regex = "one"

[[detectors]]
name = "same"
title_regex = "two"
"#;
        assert!(Config::from_toml(input).is_err());
    }

    #[test]
    fn rejects_inverted_size_range() {
        let input = r#"
[[detectors]]
name = "bad-range"
title_regex = "pip"
min_width = 900
max_width = 400
"#;
        assert!(Config::from_toml(input).is_err());
    }
}
