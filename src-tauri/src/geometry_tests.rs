use super::*;

/// Panel size of a 16:9 monitor from its diagonal in inches.
fn mm_16x9(diag_in: f64) -> Size {
    let d = diag_in * 25.4 / (16f64 * 16.0 + 9.0 * 9.0).sqrt();
    Size {
        w: 16.0 * d,
        h: 9.0 * d,
    }
}

fn info(id: &str, x: f64, y: f64, w: f64, h: f64, edid: Option<Size>) -> MonitorInfo {
    MonitorInfo {
        id: id.into(),
        gdi_name: format!("\\\\.\\{id}"),
        name: id.into(),
        px: Rect { x, y, w, h },
        scale: 1.0,
        primary: false,
        edid_mm: edid,
        hdr: HdrState::default(),
    }
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

/// SplitforDualMonitor28and32.py, center mode, on a 6000x2000 image: left 28" crop 2800x1575 at
/// (0, 213), right 32" crop 3199x1800 at (2800, 100) (the script truncates; the exact maths gives 3200).
#[test]
fn reproduces_the_old_script_center_mode() {
    let mons = [
        info("L", 0.0, 0.0, 3840.0, 2160.0, Some(mm_16x9(28.0))),
        info("R", 3840.0, 0.0, 3840.0, 2160.0, Some(mm_16x9(32.0))),
    ];
    let placed = default_placement(&mons, Align::Center, 0.0);
    let tiles = compute_layout(6000, 2000, &placed, &Framing::default()).tiles;
    let (l, r) = (&tiles[0].crop, &tiles[1].crop);
    assert!(near(l.w, 2800.0, 1.0) && near(l.h, 1575.0, 1.0), "{l:?}");
    assert!(near(r.w, 3200.0, 1.0) && near(r.h, 1800.0, 1.0), "{r:?}");
    assert!(near(l.x, 0.0, 1.0) && near(l.y, 213.0, 1.0), "{l:?}");
    assert!(near(r.x, 2800.0, 1.0) && near(r.y, 100.0, 1.0), "{r:?}");
    assert_eq!((tiles[0].out_w, tiles[0].out_h), (3840, 2160));
    assert!(near(tiles[1].upscale, 1.2, 0.001));
}

#[test]
fn top_and_bottom_alignment_match_the_script() {
    let mons = [
        info("L", 0.0, 0.0, 3840.0, 2160.0, Some(mm_16x9(28.0))),
        info("R", 3840.0, 0.0, 3840.0, 2160.0, Some(mm_16x9(32.0))),
    ];
    let crops = |a| {
        let p = default_placement(&mons, a, 0.0);
        compute_layout(6000, 2000, &p, &Framing::default()).tiles
    };
    let top = crops(Align::Top);
    assert!(near(top[0].crop.y, top[1].crop.y, 0.01));
    let bottom = crops(Align::Bottom);
    let end = |t: &Tile| t.crop.y + t.crop.h;
    assert!(near(end(&bottom[0]), end(&bottom[1]), 0.01));
}

#[test]
fn contain_zoom_offset_and_nudge() {
    let mut p = default_placement(
        &[info(
            "A",
            0.0,
            0.0,
            1000.0,
            500.0,
            Some(Size { w: 200.0, h: 100.0 }),
        )],
        Align::Center,
        0.0,
    );
    // 1000x1000 image on a 200x100 mm desk: cover = 5 px/mm, contain = 10 px/mm.
    let f = |fit, zoom, ox| Framing {
        fit,
        zoom,
        offset_mm: Point { x: ox, y: 0.0 },
    };
    let c = compute_layout(1000, 1000, &p, &f(Fit::Cover, 1.0, 0.0)).tiles[0].crop;
    assert_eq!(
        c,
        Rect {
            x: 0.0,
            y: 250.0,
            w: 1000.0,
            h: 500.0
        }
    );
    let c = compute_layout(1000, 1000, &p, &f(Fit::Contain, 1.0, 0.0)).tiles[0].crop;
    assert_eq!(
        c,
        Rect {
            x: -500.0,
            y: 0.0,
            w: 2000.0,
            h: 1000.0
        }
    );
    let c = compute_layout(1000, 1000, &p, &f(Fit::Cover, 2.0, 10.0)).tiles[0].crop;
    assert_eq!(
        c,
        Rect {
            x: 225.0,
            y: 375.0,
            w: 500.0,
            h: 250.0
        }
    );
    // Nudge 10 output px right: one output px is 1000/1000 = 1 source px at cover.
    p[0].nudge_px = Point { x: 10.0, y: -4.0 };
    let c = compute_layout(1000, 1000, &p, &f(Fit::Cover, 1.0, 0.0)).tiles[0].crop;
    assert_eq!((c.x, c.y), (10.0, 246.0));
    // 4:3 resolution on the 2:1 panel: an output pixel is 1 source px across but 0.5 down.
    p[0].px_h = 750;
    p[0].nudge_px = Point { x: 0.0, y: 10.0 };
    let c = compute_layout(1000, 1000, &p, &f(Fit::Cover, 1.0, 0.0)).tiles[0].crop;
    assert_eq!(c.y, 250.0 + 10.0 * 500.0 / 750.0);
}

#[test]
fn bezel_gap_and_missing_edid_use_the_known_pitch() {
    let mons = [
        info(
            "A",
            0.0,
            0.0,
            1000.0,
            500.0,
            Some(Size { w: 300.0, h: 150.0 }),
        ),
        info("B", 1000.0, 0.0, 2000.0, 1000.0, None),
    ];
    let p = default_placement(&mons, Align::Top, 12.0);
    assert_eq!(
        p[0].mm,
        Rect {
            x: 0.0,
            y: 0.0,
            w: 300.0,
            h: 150.0
        }
    );
    assert_eq!(
        p[1].mm,
        Rect {
            x: 312.0,
            y: 0.0,
            w: 600.0,
            h: 300.0
        }
    );
    assert_eq!(
        (p[1].px_w, p[1].px_h, p[1].nudge_px),
        (2000, 1000, Point::default())
    );
    let p = default_placement(&[info("C", 0.0, 0.0, 100.0, 50.0, None)], Align::Top, 0.0);
    assert_eq!(
        p[0].mm,
        Rect {
            x: 0.0,
            y: 0.0,
            w: 20.0,
            h: 10.0
        }
    );
}

#[test]
fn portrait_monitor_swaps_edid_and_windows_order_wins() {
    let mons = [
        // Listed out of order: the portrait one sits to the right in Windows.
        info(
            "P",
            2560.0,
            -200.0,
            1440.0,
            2560.0,
            Some(Size { w: 597.0, h: 336.0 }),
        ),
        info(
            "L",
            0.0,
            0.0,
            2560.0,
            1440.0,
            Some(Size { w: 597.0, h: 336.0 }),
        ),
    ];
    let p = default_placement(&mons, Align::Center, 0.0);
    assert_eq!((p[0].mm.w, p[0].mm.h), (336.0, 597.0));
    assert_eq!(p[0].mm.x, 597.0);
    assert_eq!(p[1].mm.x, 0.0);
    assert!(near(p[1].mm.y, (597.0 - 336.0) / 2.0, 1e-9));
}

#[test]
fn stacked_monitors_go_below_and_center_horizontally() {
    let mons = [
        info(
            "T",
            0.0,
            -1080.0,
            1920.0,
            1080.0,
            Some(Size { w: 400.0, h: 225.0 }),
        ),
        info(
            "B",
            0.0,
            0.0,
            3840.0,
            2160.0,
            Some(Size { w: 600.0, h: 337.5 }),
        ),
    ];
    let p = default_placement(&mons, Align::Center, 5.0);
    assert_eq!(p[0].mm.y, 0.0);
    assert_eq!(p[1].mm.y, 230.0);
    assert_eq!(p[0].mm.x, 100.0);
    assert_eq!(p[1].mm.x, 0.0);
}

#[test]
fn span_canvas_is_the_pixel_bounding_box() {
    let mons = [
        info("A", -1920.0, 200.0, 1920.0, 1080.0, None),
        info("B", 0.0, 0.0, 3840.0, 2160.0, None),
    ];
    let s = span_canvas(&mons);
    assert_eq!((s.w, s.h, s.origin_x, s.origin_y), (5760, 2160, -1920, 0));
    assert_eq!(span_canvas(&[]).w, 0);
}

#[test]
fn degenerate_inputs_give_no_tiles() {
    assert!(compute_layout(100, 100, &[], &Framing::default())
        .tiles
        .is_empty());
    let p = default_placement(&[info("A", 0.0, 0.0, 10.0, 10.0, None)], Align::Top, 0.0);
    assert!(compute_layout(0, 100, &p, &Framing::default())
        .tiles
        .is_empty());
}
