use super::*;

fn close(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn pq_203_nits_round_trips_to_scrgb() {
    let s = pq_inverse_eotf(203.0);
    assert!(close(s, 0.5806, 1e-3), "signal {s}");
    assert!(close(pq_eotf(s) / 80.0, 2.5375, 1e-3));
    assert!(close(pq_inverse_eotf(1000.0), 0.7518, 1e-3));
    assert!(close(pq_eotf(1.0), 10000.0, 1.0));
}

#[test]
fn srgb_round_trips() {
    for i in 0..=255 {
        let v = i as f32 / 255.0;
        assert!(close(srgb_oetf(srgb_eotf(v)), v, 1e-5), "{v}");
    }
    assert!(close(srgb_eotf(0.5), 0.2140, 1e-4));
}

#[test]
fn hlg_matches_the_fixture_definition() {
    // Fixture README: 203 nits is signal 0.7498, 1000 nits is 1.0 (grey).
    assert!(close(hlg_to_nits([1.0; 3])[0], 1000.0, 1.0));
    assert!(close(hlg_to_nits([0.74987; 3])[0], 203.0, 1.0));
}

#[test]
fn matrices_map_white_to_white() {
    for g in [
        Gamut::Bt709,
        Gamut::DisplayP3,
        Gamut::Bt2020,
        Gamut::AdobeRgb,
        Gamut::ProPhoto,
    ] {
        let w = apply(&to_bt709(g), [1.0; 3]);
        assert!(w.iter().all(|v| close(*v, 1.0, 1e-4)), "{g:?} {w:?}");
    }
}

#[test]
fn matrices_match_published_values() {
    let p3 = to_bt709(Gamut::DisplayP3);
    assert!(close(p3[0][0], 1.2249, 1e-3) && close(p3[2][1], -0.0786, 1e-3));
    let bt2020 = to_bt709(Gamut::Bt2020);
    assert!(close(bt2020[0][0], 1.6605, 1e-3) && close(bt2020[1][0], -0.1246, 1e-3));
    let red = apply(&bt2020, [2.5375, 0.0, 0.0]);
    assert!(close(red[0], 4.2135, 0.01) && close(red[1], -0.316, 0.01));
}

#[test]
fn tone_map_keeps_shadows_and_lands_peak_on_one() {
    assert_eq!(tone_map([0.5, 0.2, 0.1], 4.9), [0.5, 0.2, 0.1]);
    let p = tone_map([4.9, 4.9, 4.9], 4.9);
    assert!(close(p[0], 1.0, 1e-5));
    let mut last = 0.0;
    for i in 0..100 {
        let v = tone_map([i as f32 * 0.05; 3], 4.9)[0];
        assert!(v >= last && v <= 1.0 + 1e-6);
        last = v;
    }
    let c = tone_map([3.0, 1.5, 0.0], 4.9);
    assert!(close(c[1] / c[0], 0.5, 1e-5), "hue kept");
}
