use super::*;

fn grey_u8(w: u32, h: u32, v: u8) -> Source {
    let px = Pixels::U8(vec![v; (w * h * 3) as usize]);
    Source::new((w, h), px, Transfer::Srgb, Gamut::Bt709, 1.0, false)
}

#[test]
fn render_fills_outside_and_resizes_exactly() {
    let src = grey_u8(100, 50, 255);
    let fill = [0.25, 0.0, 0.0];
    // Left half of the crop lies outside the image.
    let crop = Rect {
        x: -100.0,
        y: 0.0,
        w: 200.0,
        h: 50.0,
    };
    let out = render(&src, crop, 40, 10, fill).unwrap();
    assert_eq!(out.len(), 400);
    assert_eq!(out[5 * 40 + 2], fill);
    let inside = out[5 * 40 + 35];
    assert!((inside[0] - 1.0).abs() < 1e-3, "{inside:?}");
    // Entirely outside: all fill.
    let away = Rect { x: 500.0, ..crop };
    assert!(render(&src, away, 4, 2, fill)
        .unwrap()
        .iter()
        .all(|p| *p == fill));
}

#[test]
fn hdr_sources_linearise_and_measure_headroom() {
    // PQ code for 1000 nits -> 1000/203 SDR whites.
    let code = (crate::color::pq_inverse_eotf(1000.0) * 65535.0).round() as u16;
    let px = Pixels::U16(vec![code; 12]);
    let src = Source::new((2, 2), px, Transfer::Pq, Gamut::Bt2020, 1.0 / 203.0, true);
    assert!(
        (src.headroom - 1000.0 / 203.0).abs() < 0.01,
        "{}",
        src.headroom
    );
    let p = src.region(0, 0, 1, 1, [0.0; 3])[0].0;
    assert!(
        (p[0] - 4.926).abs() < 0.01 && (p[2] - 4.926).abs() < 0.01,
        "{p:?}"
    );
}

#[test]
fn orientation_rotates_and_keeps_f16_values() {
    let v: Vec<f16> = (0..6 * 3).map(|i| f16::from_f32(i as f32 * 0.5)).collect();
    let src = Source::new(
        (3, 2),
        Pixels::F16(v),
        Transfer::Linear,
        Gamut::Bt709,
        1.0,
        true,
    );
    let r = src.orient(Orientation::Rotate90);
    assert_eq!((r.w, r.h), (2, 3));
    let Pixels::F16(px) = &r.px else {
        panic!("lost f16")
    };
    // Rotating 90 degrees clockwise puts the bottom-left pixel (0,1) at the top-left.
    assert_eq!(px[0].to_f32(), (3 * 3) as f32 * 0.5);
}
