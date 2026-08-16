use crate::{Config, DetectionAction, DetectorConfig, WindowInfo};
use regex::Regex;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DetectorError {
    #[error("detector '{name}' contains an invalid regex: {source}")]
    Regex { name: String, source: regex::Error },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectionMatch {
    pub detector: String,
    pub score: i32,
    pub action: DetectionAction,
}

#[derive(Debug)]
struct CompiledDetector {
    config: DetectorConfig,
    title: Option<Regex>,
    app_id: Option<Regex>,
    exclude_title: Option<Regex>,
    exclude_app_id: Option<Regex>,
}

#[derive(Debug)]
pub struct DetectorEngine {
    detectors: Vec<CompiledDetector>,
    threshold: i32,
}

impl DetectorEngine {
    pub fn new(config: &Config) -> Result<Self, DetectorError> {
        let mut detectors = Vec::with_capacity(config.detectors.len());
        for detector in &config.detectors {
            if !builtin_detector_enabled(config, &detector.name) {
                continue;
            }
            let compile = |pattern: &Option<String>| -> Result<Option<Regex>, DetectorError> {
                pattern
                    .as_ref()
                    .map(|p| {
                        Regex::new(p).map_err(|source| DetectorError::Regex {
                            name: detector.name.clone(),
                            source,
                        })
                    })
                    .transpose()
            };
            detectors.push(CompiledDetector {
                config: detector.clone(),
                title: compile(&detector.title_regex)?,
                app_id: compile(&detector.app_id_regex)?,
                exclude_title: compile(&detector.exclude_title_regex)?,
                exclude_app_id: compile(&detector.exclude_app_id_regex)?,
            });
        }
        Ok(Self {
            detectors,
            threshold: config.general.detection_threshold,
        })
    }

    pub fn detect(&self, window: &WindowInfo, is_new: bool) -> Option<DetectionMatch> {
        self.detectors
            .iter()
            .filter_map(|d| d.score(window, is_new))
            .filter(|m| m.score >= self.threshold)
            .max_by_key(|m| m.score)
    }
}

impl CompiledDetector {
    fn score(&self, window: &WindowInfo, is_new: bool) -> Option<DetectionMatch> {
        if !self.config.enabled {
            return None;
        }

        let title = window.title();
        let app_id = window.app_id();

        if self
            .exclude_title
            .as_ref()
            .is_some_and(|r| r.is_match(title))
            || self
                .exclude_app_id
                .as_ref()
                .is_some_and(|r| r.is_match(app_id))
        {
            return None;
        }
        if self.title.as_ref().is_some_and(|r| !r.is_match(title)) {
            return None;
        }
        if self.app_id.as_ref().is_some_and(|r| !r.is_match(app_id)) {
            return None;
        }
        if self
            .config
            .floating
            .is_some_and(|expected| expected != window.is_floating)
        {
            return None;
        }

        let (w, h) = window.logical_size();
        if self.config.min_width.is_some_and(|v| w < v)
            || self.config.max_width.is_some_and(|v| w > v)
            || self.config.min_height.is_some_and(|v| h < v)
            || self.config.max_height.is_some_and(|v| h > v)
        {
            return None;
        }

        let mut score = self.config.score;
        if w > 0 && h > 0 && w <= 1280 && h <= 900 {
            score += self.config.compact_bonus;
        }
        if h > 0 {
            let ratio = w as f64 / h as f64;
            if (ratio - (16.0 / 9.0)).abs() <= 0.20 {
                score += self.config.aspect_16_9_bonus;
            }
        }
        if app_id.is_empty() {
            score += self.config.empty_app_id_bonus;
        }
        if window.pid.is_some() {
            score += self.config.pid_present_bonus;
        }
        if is_new {
            score += self.config.new_window_bonus;
        }

        Some(DetectionMatch {
            detector: self.config.name.clone(),
            score,
            action: self.config.action,
        })
    }
}

fn builtin_detector_enabled(config: &Config, name: &str) -> bool {
    match name {
        "firefox-pip" => config.browsers.firefox,
        "chromium-empty-app-id" => {
            config.browsers.chromium
                || config.browsers.brave
                || config.browsers.vivaldi
                || config.browsers.edge
        }
        "chromium-family-pip" => {
            config.browsers.chromium
                || config.browsers.brave
                || config.browsers.vivaldi
                || config.browsers.edge
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WindowLayout;

    fn window(app_id: Option<&str>, title: &str, size: (i32, i32)) -> WindowInfo {
        WindowInfo {
            id: 42,
            app_id: app_id.map(str::to_owned),
            title: Some(title.to_owned()),
            layout: WindowLayout {
                window_size: size,
                tile_size: (size.0 as f64, size.1 as f64),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn detects_empty_app_id_chromium_pip() {
        let d = DetectorEngine::new(&Config::default()).unwrap();
        let m = d
            .detect(&window(Some(""), "Picture in picture", (480, 270)), true)
            .unwrap();
        assert_eq!(m.detector, "chromium-empty-app-id");
        assert!(m.score >= 155);
    }

    #[test]
    fn detects_missing_app_id_chromium_pip() {
        let d = DetectorEngine::new(&Config::default()).unwrap();
        let m = d
            .detect(&window(None, "Picture in picture", (480, 270)), true)
            .unwrap();
        assert_eq!(m.detector, "chromium-empty-app-id");
    }

    #[test]
    fn identified_pip_is_not_rejected_only_for_being_large() {
        let d = DetectorEngine::new(&Config::default()).unwrap();
        let m = d
            .detect(&window(Some(""), "Picture in picture", (2400, 1350)), true)
            .unwrap();
        assert_eq!(m.detector, "chromium-empty-app-id");
    }

    #[test]
    fn detects_firefox_pip() {
        let d = DetectorEngine::new(&Config::default()).unwrap();
        let m = d
            .detect(
                &window(Some("firefox"), "Picture-in-Picture", (480, 270)),
                true,
            )
            .unwrap();
        assert_eq!(m.detector, "firefox-pip");
    }

    #[test]
    fn rejects_normal_browser_window() {
        let d = DetectorEngine::new(&Config::default()).unwrap();
        assert!(d
            .detect(
                &window(
                    Some("google-chrome"),
                    "YouTube - Google Chrome",
                    (1440, 900),
                ),
                true,
            )
            .is_none());
    }

    #[test]
    fn rejects_oversized_generic_title_false_positive() {
        let d = DetectorEngine::new(&Config::default()).unwrap();
        assert!(d
            .detect(
                &window(Some("unknown"), "Picture in picture", (1920, 1080)),
                true,
            )
            .is_none());
    }
}
