use crate::{LogicalOutput, Margins, Placement};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacementPlan {
    pub x_percent: f64,
    pub y_percent: f64,
}

pub fn corner_placement(
    output: LogicalOutput,
    window_size: (u32, u32),
    placement: Placement,
    gap: u32,
    margins: Margins,
) -> PlacementPlan {
    let width = output.width.max(1) as f64;
    let height = output.height.max(1) as f64;
    let window_w = window_size.0.min(output.width) as f64;
    let window_h = window_size.1.min(output.height) as f64;

    let left = (margins.left + gap) as f64;
    let right = (margins.right + gap) as f64;
    let top = (margins.top + gap) as f64;
    let bottom = (margins.bottom + gap) as f64;

    let right_x = (width - window_w - right).max(left);
    let bottom_y = (height - window_h - bottom).max(top);
    let center_x = ((width - window_w) / 2.0).max(0.0);
    let center_y = ((height - window_h) / 2.0).max(0.0);

    let (x, y) = match placement {
        Placement::TopLeft => (left, top),
        Placement::TopRight => (right_x, top),
        Placement::BottomLeft => (left, bottom_y),
        Placement::BottomRight => (right_x, bottom_y),
        Placement::Center => (center_x, center_y),
    };

    PlacementPlan {
        x_percent: (x / width * 100.0).clamp(0.0, 100.0),
        y_percent: (y / height * 100.0).clamp(0.0, 100.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output() -> LogicalOutput {
        LogicalOutput {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            scale: 1.0,
        }
    }

    #[test]
    fn bottom_right_is_inside_safe_area() {
        let p = corner_placement(
            output(),
            (480, 270),
            Placement::BottomRight,
            18,
            Margins::default(),
        );
        assert!(p.x_percent > 70.0 && p.x_percent < 80.0);
        assert!(p.y_percent > 65.0 && p.y_percent < 80.0);
    }

    #[test]
    fn top_left_respects_margins_and_gap() {
        let p = corner_placement(
            output(),
            (480, 270),
            Placement::TopLeft,
            18,
            Margins {
                top: 20,
                left: 10,
                right: 0,
                bottom: 0,
            },
        );
        assert!((p.x_percent - (28.0 / 1920.0 * 100.0)).abs() < 0.001);
        assert!((p.y_percent - (38.0 / 1080.0 * 100.0)).abs() < 0.001);
    }

    #[test]
    fn center_accounts_for_window_size() {
        let p = corner_placement(
            output(),
            (480, 270),
            Placement::Center,
            18,
            Margins::default(),
        );
        assert!((p.x_percent - 37.5).abs() < 0.01);
        assert!((p.y_percent - 37.5).abs() < 0.01);
    }
}
