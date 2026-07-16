use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Notch,
    Hud,
}

pub fn presentation_mode(safe_area_top_inset: f64) -> Mode {
    if safe_area_top_inset > 0.0 {
        Mode::Notch
    } else {
        Mode::Hud
    }
}

/// the notch cutout's horizontal bounds (plan §3.5), reported by the swift
/// shim alongside the safe-area inset. only meaningful when `width > 0.0` —
/// see [`DetectOutput::cutout`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutoutGeometry {
    pub left_x: f64,
    pub right_x: f64,
    pub width: f64,
}

impl CutoutGeometry {
    pub fn center_x(&self) -> f64 {
        (self.left_x + self.right_x) / 2.0
    }
}

#[derive(Debug, Deserialize)]
struct DetectOutput {
    #[serde(rename = "safe_area_top_inset")]
    safe_area_top_inset: f64,
    #[serde(default)]
    cutout_left_x: f64,
    #[serde(default)]
    cutout_right_x: f64,
    #[serde(default)]
    cutout_width: f64,
}

impl DetectOutput {
    /// `None` for an older shim binary's output (fields absent, defaulted to
    /// 0.0), a non-notch screen, or any other zero-width report.
    fn cutout(&self) -> Option<CutoutGeometry> {
        if self.cutout_width > 0.0 {
            Some(CutoutGeometry {
                left_x: self.cutout_left_x,
                right_x: self.cutout_right_x,
                width: self.cutout_width,
            })
        } else {
            None
        }
    }
}

pub fn detect_mode(config: &Config) -> (Mode, f64, Option<CutoutGeometry>) {
    match run_detect(&config.detect_path) {
        Ok(output) => {
            let mode = presentation_mode(output.safe_area_top_inset);
            (mode, output.safe_area_top_inset, output.cutout())
        }
        Err(e) => {
            tracing::warn!(
                "failed to detect presentation mode: {}; falling back to hud",
                e
            );
            (Mode::Hud, 0.0, None)
        }
    }
}

fn run_detect(detect_path: &Path) -> anyhow::Result<DetectOutput> {
    let output = Command::new(detect_path)
        .output()
        .map_err(|e| anyhow::anyhow!("failed to execute {:?}: {}", detect_path, e))?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "notchtap-detect exited with code {:?}",
            output.status.code()
        ));
    }

    parse_detect_output(&String::from_utf8_lossy(&output.stdout))
}

fn parse_detect_output(stdout: &str) -> anyhow::Result<DetectOutput> {
    serde_json::from_str(stdout)
        .map_err(|e| anyhow::anyhow!("failed to parse notchtap-detect output: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::path::PathBuf;

    #[test]
    fn zero_inset_is_hud_mode() {
        assert_eq!(presentation_mode(0.0), Mode::Hud);
    }

    #[test]
    fn positive_inset_is_notch_mode() {
        assert_eq!(presentation_mode(32.0), Mode::Notch);
    }

    #[test]
    fn negative_inset_is_hud_mode() {
        assert_eq!(presentation_mode(-1.0), Mode::Hud);
    }

    #[test]
    fn well_formed_stdout_parses() {
        let parsed = parse_detect_output(r#"{ "safe_area_top_inset": 32.5 }"#).unwrap();
        assert_eq!(parsed.safe_area_top_inset, 32.5);
    }

    #[test]
    fn well_formed_stdout_without_cutout_defaults_to_zero() {
        // old-shim-binary shape: no cutout fields at all
        let parsed = parse_detect_output(r#"{ "safe_area_top_inset": 32.5 }"#).unwrap();
        assert_eq!(parsed.cutout_left_x, 0.0);
        assert_eq!(parsed.cutout_right_x, 0.0);
        assert_eq!(parsed.cutout_width, 0.0);
        assert_eq!(parsed.cutout(), None);
    }

    #[test]
    fn well_formed_stdout_with_cutout_parses() {
        let parsed = parse_detect_output(
            r#"{ "safe_area_top_inset": 32.0, "cutout_left_x": 480.5, "cutout_right_x": 799.5, "cutout_width": 319.0 }"#,
        )
        .unwrap();
        assert_eq!(parsed.safe_area_top_inset, 32.0);
        assert_eq!(parsed.cutout_left_x, 480.5);
        assert_eq!(parsed.cutout_right_x, 799.5);
        assert_eq!(parsed.cutout_width, 319.0);
        assert_eq!(
            parsed.cutout(),
            Some(CutoutGeometry {
                left_x: 480.5,
                right_x: 799.5,
                width: 319.0,
            })
        );
    }

    #[test]
    fn zero_width_cutout_is_none() {
        let parsed = parse_detect_output(
            r#"{ "safe_area_top_inset": 0.0, "cutout_left_x": 0.0, "cutout_right_x": 0.0, "cutout_width": 0.0 }"#,
        )
        .unwrap();
        assert_eq!(parsed.cutout(), None);
    }

    #[test]
    fn cutout_center_x_computes_midpoint() {
        let cutout = CutoutGeometry {
            left_x: 480.5,
            right_x: 799.5,
            width: 319.0,
        };
        assert_eq!(cutout.center_x(), 640.0);
    }

    #[test]
    fn malformed_stdout_is_an_error() {
        assert!(parse_detect_output("not json at all").is_err());
        assert!(parse_detect_output(r#"{ "safe_area_top_inset": "#).is_err());
    }

    #[test]
    fn missing_binary_falls_back_to_hud() {
        let config = Config {
            detect_path: PathBuf::from("/nonexistent/notchtap-detect"),
            ..Config::default()
        };
        assert_eq!(detect_mode(&config), (Mode::Hud, 0.0, None));
    }

    #[test]
    fn nonzero_exit_falls_back_to_hud() {
        let config = Config {
            detect_path: PathBuf::from("/usr/bin/false"),
            ..Config::default()
        };
        assert_eq!(detect_mode(&config), (Mode::Hud, 0.0, None));
    }
}
