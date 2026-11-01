use super::*;
use crate::color::{self, Rgb};

pub fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

pub fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wps-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn at(src: &Source, x: i64, y: i64) -> Rgb {
    src.region(x, y, 1, 1, [0.0; 3], [0, 0, src.w as i64, src.h as i64])[0].0
}

/// Linear pixel -> 8/16-bit sRGB code, to compare with the fixture README's code values.
fn code(v: f32, max: f32) -> f32 {
    color::srgb_oetf(v) * max
}

fn check_sdr(name: &str, max: f32, tol: f32) {
    let src = load(&fixture(name), false, [0; 3]).unwrap();
    assert_eq!((src.w, src.h, src.hdr), (128, 64, false), "{name}");
    let expect = [
        (32, 0, 16513.0),
        (64, 0, 33026.0),
        (96, 0, 49538.0),
        (127, 0, 65535.0),
    ];
    for (x, y, v16) in expect {
        let want = (v16 / 65535.0 * max).round();
        let got = code(at(&src, x, y)[0], max);
        assert!(
            (got - want).abs() <= tol + 0.01,
            "{name} x={x}: {got} vs {want}"
        );
    }
    let red = at(&src, 80, 56).map(|v| code(v, max));
    assert!(
        (red[0] - max).abs() <= tol + 0.01 && red[1] <= tol + 0.01 && red[2] <= tol + 0.01,
        "{name} {red:?}"
    );
    assert!(at(&src, 110, 56)
        .iter()
        .all(|v| code(*v, max) <= tol + 0.01));
}

#[test]
fn sdr_fixtures_decode_to_their_code_values() {
    check_sdr("sdr8.png", 255.0, 0.0);
    check_sdr("sdr8.webp", 255.0, 0.0);
    check_sdr("sdr8.jpg", 255.0, 12.0);
    check_sdr("sdr16.png", 65535.0, 0.0);
    check_sdr("sdr16.tif", 65535.0, 0.0);
}

/// HDR fixtures in linear BT.709, 1.0 = 203 nits: ramp end 1000 nits, white patch 203, and a red patch
/// with the white patch's R signal: BT.2020 red at 203 nits (BT.709 red for Ultra HDR).
fn check_hdr(src: &Source, name: &str, rel: f32, red_is_2020: bool) {
    assert_eq!((src.w, src.h, src.hdr), (128, 64, true), "{name}");
    let close = |a: f32, b: f32| (a - b).abs() <= rel * b.abs().max(1.0);
    let peak = at(src, 127, 0);
    assert!(
        close(peak[0], 1000.0 / 203.0) && close(peak[2], 1000.0 / 203.0),
        "{name} peak {peak:?}"
    );
    let white = at(src, 16, 56);
    assert!(
        close(white[0], 1.0) && close(white[1], 1.0),
        "{name} white {white:?}"
    );
    let red = at(src, 80, 56);
    let want = if red_is_2020 {
        // HLG's OOTF scales by the pixel's own luminance, so its red lands below 203 nits.
        let r = if name.contains("hlg") {
            color::hlg_to_nits([0.74987, 0.0, 0.0])[0] / 203.0
        } else {
            1.0
        };
        color::apply(&color::to_bt709(Gamut::Bt2020), [r, 0.0, 0.0])
    } else {
        [1.0, 0.0, 0.0]
    };
    assert!(
        close(red[0], want[0]) && (red[1] - want[1]).abs() < 0.05,
        "{name} red {red:?} want {want:?}"
    );
    assert!(src.headroom > 4.5, "{name} headroom {}", src.headroom);
}

#[test]
fn jxl_pq_and_hlg_decode_to_linear_light() {
    for name in ["hdr_pq16_bt2020.jxl", "hdr_hlg16_bt2020.jxl"] {
        let src = load(&fixture(name), true, [0; 3]).unwrap();
        check_hdr(&src, name, 0.005, true);
    }
}

#[test]
fn ultrahdr_decodes_hdr_and_its_sdr_base() {
    let p = fixture("hdr_ultrahdr.jpg");
    assert_eq!(sniff(&p).unwrap(), Format::UltraHdr);
    let src = load(&p, true, [0; 3]).unwrap();
    check_hdr(&src, "ultrahdr", 0.03, false);
    let base = load(&p, false, [0; 3]).unwrap();
    assert_eq!((base.w, base.h, base.hdr), (128, 64, false));
    assert!(at(&base, 127, 0)[0] <= 1.0);
}

#[test]
fn avif_decodes_through_wic_when_the_extension_is_installed() {
    let p = fixture("hdr_pq10_bt2020.avif");
    assert_eq!(sniff(&p).unwrap(), Format::Avif);
    if wic::probe(&p).is_err() {
        eprintln!("skipped: no AV1/HEIF extension (as on CI runners)");
        assert_eq!(load(&p, true, [0; 3]).err().as_deref(), Some(AVIF_HELP));
        return;
    }
    check_hdr(&load(&p, true, [0; 3]).unwrap(), "avif", 0.03, true);
}

#[test]
fn open_reports_formats_and_writes_reusable_proxies() {
    let cache = temp_dir("proxy");
    let expect = [
        ("sdr8.png", "png", false),
        ("sdr8.jpg", "jpeg", false),
        ("sdr8.webp", "webp", false),
        ("sdr16.tif", "tiff", false),
        ("hdr_ultrahdr.jpg", "ultrahdr", true),
        ("hdr_pq16_bt2020.jxl", "jxl", true),
    ];
    for (name, format, hdr) in expect {
        let p = fixture(name);
        let info = open(&p, &cache).unwrap();
        assert_eq!(
            (info.width, info.height, info.format.as_str(), info.hdr),
            (128, 64, format, hdr),
            "{name}"
        );
        if matches!(format, "tiff" | "jxl") {
            assert!(
                info.preview_path.starts_with(&*cache.to_string_lossy()),
                "{name}"
            );
            assert_eq!(
                image::image_dimensions(&info.preview_path).unwrap(),
                (128, 64)
            );
            let again = open(&p, &cache).unwrap();
            assert_eq!(again.preview_path, info.preview_path);
        } else {
            assert_eq!(info.preview_path, p);
        }
    }
    // The HDR JXL's proxy is itself an Ultra HDR JPEG.
    let jxl = open(&fixture("hdr_pq16_bt2020.jxl"), &cache).unwrap();
    assert_eq!(sniff(&jxl.preview_path).unwrap(), Format::UltraHdr);
    let fixtures_readme = fixture("README.md");
    assert!(open(&fixtures_readme, &cache)
        .unwrap_err()
        .contains("not an image"));
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn icc_descriptions_pick_the_gamut() {
    assert_eq!(icc_space(None), (Transfer::Srgb, Gamut::Bt709));
    assert_eq!(icc_space(Some(b"junk")), (Transfer::Srgb, Gamut::Bt709));
}

#[test]
fn transparency_is_laid_onto_the_fill() {
    // Fully transparent white, half-transparent white, opaque black over a red fill.
    let px = vec![255, 255, 255, 0, 255, 255, 255, 128, 0, 0, 0, 255];
    let img = DynamicImage::ImageRgba8(image::ImageBuffer::from_raw(3, 1, px).unwrap());
    let DynamicImage::ImageRgb16(out) = flatten(img, [255, 0, 0]) else {
        panic!("not flattened to RGB16");
    };
    let p = |i: u32| {
        out.get_pixel(i, 0)
            .0
            .map(|v| (v as f32 / 257.0).round() as u8)
    };
    assert_eq!(p(0), [255, 0, 0]);
    assert_eq!(p(1), [255, 128, 128]);
    assert_eq!(p(2), [0, 0, 0]);
}
