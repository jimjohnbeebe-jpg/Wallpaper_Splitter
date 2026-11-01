// geometry: desk millimetres -> per-monitor crop rectangles, default placement, span canvas.
// The ONLY layout maths in the app (CLAUDE.md). Signatures are the contract; bodies are built in pass 1.
#![allow(dead_code)]
use crate::model::*;

/// Crop rectangle (source pixels) and output size for every placed monitor.
pub fn compute_layout(
    width: u32,
    height: u32,
    monitors: &[PlacedMonitor],
    framing: &Framing,
) -> Layout {
    let _ = (width, height, monitors, framing);
    Layout { tiles: Vec::new() }
}

/// Starting arrangement from Windows' order and each monitor's physical size.
pub fn default_placement(
    monitors: &[MonitorInfo],
    align: Align,
    bezel_mm: f64,
) -> Vec<PlacedMonitor> {
    let _ = (monitors, align, bezel_mm);
    Vec::new()
}

/// Bounding box of the monitors' Windows pixel rectangles.
pub fn span_canvas(monitors: &[MonitorInfo]) -> Span {
    let _ = monitors;
    Span {
        w: 0,
        h: 0,
        origin_x: 0,
        origin_y: 0,
    }
}
