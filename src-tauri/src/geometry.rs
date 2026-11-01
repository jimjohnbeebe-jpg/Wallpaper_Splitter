// geometry: desk millimetres -> per-monitor crop rectangles, default placement, span canvas.
// The ONLY layout maths in the app (CLAUDE.md).
use crate::model::*;

/// Fallback pixel pitch when no monitor reports a physical size.
const DEFAULT_MM_PER_PX: f64 = 0.2;
/// Pixel rectangles from Windows are whole numbers; this absorbs float noise in "touching" tests.
const EPS: f64 = 0.5;

fn union_mm(monitors: &[PlacedMonitor]) -> Rect {
    let (mut x0, mut y0) = (f64::INFINITY, f64::INFINITY);
    let (mut x1, mut y1) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for m in monitors {
        x0 = x0.min(m.mm.x);
        y0 = y0.min(m.mm.y);
        x1 = x1.max(m.mm.x + m.mm.w);
        y1 = y1.max(m.mm.y + m.mm.h);
    }
    Rect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    }
}

/// Crop rectangle (source pixels) and output size for every placed monitor.
pub fn compute_layout(
    width: u32,
    height: u32,
    monitors: &[PlacedMonitor],
    framing: &Framing,
) -> Layout {
    let b = union_mm(monitors);
    if monitors.is_empty() || b.w <= 0.0 || b.h <= 0.0 || width == 0 || height == 0 {
        return Layout { tiles: Vec::new() };
    }
    let (w, h) = (width as f64, height as f64);
    // Source pixels per desk millimetre.
    let fit = match framing.fit {
        Fit::Cover => (w / b.w).min(h / b.h),
        Fit::Contain => (w / b.w).max(h / b.h),
    };
    let zoom = if framing.zoom > 0.0 {
        framing.zoom
    } else {
        1.0
    };
    let k = fit / zoom;
    let (cx, cy) = (b.x + b.w / 2.0, b.y + b.h / 2.0);
    let off = framing.offset_mm;
    let tiles = monitors
        .iter()
        .map(|m| {
            let (cw, ch) = (m.mm.w * k, m.mm.h * k);
            let (px_w, px_h) = (m.px_w.max(1), m.px_h.max(1));
            // Nudge is in output pixels; one output pixel is cw / px_w source pixels across and ch / px_h
            // down, which differ when the resolution's shape is not the panel's (4:3 on a 16:9 panel).
            let crop = Rect {
                x: w / 2.0 + (m.mm.x - cx - off.x) * k + m.nudge_px.x * cw / px_w as f64,
                y: h / 2.0 + (m.mm.y - cy - off.y) * k + m.nudge_px.y * ch / px_h as f64,
                w: cw,
                h: ch,
            };
            Tile {
                monitor_id: m.id.clone(),
                crop,
                out_w: px_w,
                out_h: px_h,
                upscale: px_w as f64 / cw,
            }
        })
        .collect();
    Layout { tiles }
}

/// Physical panel size in mm: EDID (turned to match a portrait pixel aspect), else the pitch of the
/// first monitor that has EDID, else 0.2 mm per pixel.
fn physical_mm(m: &MonitorInfo, fallback_pitch: f64) -> Size {
    match m.edid_mm {
        Some(s) if s.w > 0.0 && s.h > 0.0 => {
            if (s.w > s.h) != (m.px.w > m.px.h) && s.w != s.h && m.px.w != m.px.h {
                Size { w: s.h, h: s.w }
            } else {
                s
            }
        }
        _ => Size {
            w: m.px.w * fallback_pitch,
            h: m.px.h * fallback_pitch,
        },
    }
}

fn cross_offset(align: Align, extent: f64, max_extent: f64) -> f64 {
    match align {
        Align::Top => 0.0,
        Align::Center => (max_extent - extent) / 2.0,
        Align::Bottom => max_extent - extent,
    }
}

/// Starting arrangement from Windows' order and each monitor's physical size. A monitor goes to the
/// right of every monitor whose Windows rectangle ends at or before its left edge (plus `bezel_mm`), and
/// below every monitor that ends at or above its top edge. Monitors with nothing to their left (or
/// above) line up on that axis by `align` (top/center/bottom; left/center/right for a stacked column).
pub fn default_placement(
    monitors: &[MonitorInfo],
    align: Align,
    bezel_mm: f64,
) -> Vec<PlacedMonitor> {
    let pitch = monitors
        .iter()
        .find(|m| m.edid_mm.is_some_and(|s| s.w > 0.0 && s.h > 0.0) && m.px.w > 0.0)
        .map(|m| physical_mm(m, 0.0).w / m.px.w)
        .unwrap_or(DEFAULT_MM_PER_PX);
    let sizes: Vec<Size> = monitors.iter().map(|m| physical_mm(m, pitch)).collect();
    let axis = |px: fn(&Rect) -> (f64, f64), mm: fn(&Size) -> f64| {
        let px: Vec<(f64, f64)> = monitors.iter().map(|m| px(&m.px)).collect();
        let mm: Vec<f64> = sizes.iter().map(mm).collect();
        place_axis(&px, &mm, align, bezel_mm)
    };
    let xs = axis(|r| (r.x, r.w), |s| s.w);
    let ys = axis(|r| (r.y, r.h), |s| s.h);
    monitors
        .iter()
        .enumerate()
        .map(|(i, m)| PlacedMonitor {
            id: m.id.clone(),
            px_w: m.px.w.round().max(0.0) as u32,
            px_h: m.px.h.round().max(0.0) as u32,
            mm: Rect {
                x: xs[i],
                y: ys[i],
                w: sizes[i].w,
                h: sizes[i].h,
            },
            nudge_px: Point::default(),
        })
        .collect()
}

/// One axis of `default_placement`: `px` is each monitor's (start, length) in Windows pixels on this
/// axis, `mm` its physical length. Returns each monitor's start in mm.
fn place_axis(px: &[(f64, f64)], mm: &[f64], align: Align, bezel_mm: f64) -> Vec<f64> {
    let n = px.len();
    let before = |j: usize, i: usize| j != i && px[j].0 + px[j].1 <= px[i].0 + EPS;
    let first = |i: usize| !(0..n).any(|j| before(j, i));
    let first_max = (0..n)
        .filter(|&i| first(i))
        .map(|i| mm[i])
        .fold(0.0, f64::max);
    // Pixel order places every monitor before the ones that sit after it.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| px[a].0.total_cmp(&px[b].0));
    let mut pos = vec![0.0; n];
    for i in order {
        pos[i] = if first(i) {
            cross_offset(align, mm[i], first_max)
        } else {
            (0..n)
                .filter(|&j| before(j, i))
                .map(|j| pos[j] + mm[j] + bezel_mm)
                .fold(0.0, f64::max)
        };
    }
    pos
}

/// Bounding box of the monitors' Windows pixel rectangles.
pub fn span_canvas(monitors: &[MonitorInfo]) -> Span {
    if monitors.is_empty() {
        return Span {
            w: 0,
            h: 0,
            origin_x: 0,
            origin_y: 0,
        };
    }
    let x0 = monitors
        .iter()
        .map(|m| m.px.x)
        .fold(f64::INFINITY, f64::min);
    let y0 = monitors
        .iter()
        .map(|m| m.px.y)
        .fold(f64::INFINITY, f64::min);
    let x1 = monitors
        .iter()
        .map(|m| m.px.x + m.px.w)
        .fold(f64::NEG_INFINITY, f64::max);
    let y1 = monitors
        .iter()
        .map(|m| m.px.y + m.px.h)
        .fold(f64::NEG_INFINITY, f64::max);
    Span {
        w: (x1 - x0).round() as u32,
        h: (y1 - y0).round() as u32,
        origin_x: x0.round() as i32,
        origin_y: y0.round() as i32,
    }
}

#[cfg(test)]
#[path = "geometry_tests.rs"]
mod tests;
