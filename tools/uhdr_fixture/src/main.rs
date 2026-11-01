//! Writes a 128x64 Ultra HDR JPEG with the fixture pattern (see src-tauri/tests/fixtures/README.md).
//! Usage: uhdr_fixture <out.jpg>
use image::codecs::jpeg::JpegEncoder;
use image::ExtendedColorType;
use ultrahdr_rs::color::tonemap::tonemap_image_to_srgb8;
use ultrahdr_rs::gainmap::compute::compute_gainmap;
use ultrahdr_rs::{
    encode_ultrahdr, ColorGamut, ColorTransfer, Decoder, GainMapConfig, PixelFormat, RawImage,
    Unstoppable,
};

const W: u32 = 128;
const H: u32 = 64;
const PEAK_NITS: f32 = 1000.0;
const SDR_WHITE_NITS: f32 = 203.0; // ultrahdr linear 1.0

/// Linear BT.709 RGB in nits. Rows 0..47: grey ramp 0..1000. Rows 48..63: x 0..63 white
/// at 203, x 64..95 red (R = 203, G = B = 0), x 96..127 black.
fn pixel_nits(x: u32, y: u32) -> [f32; 3] {
    if y < 48 {
        let v = PEAK_NITS * x as f32 / (W - 1) as f32;
        [v, v, v]
    } else if x < 64 {
        [203.0; 3]
    } else if x < 96 {
        [203.0, 0.0, 0.0]
    } else {
        [0.0; 3]
    }
}

fn jpeg(px: &[u8], w: u32, h: u32, t: ExtendedColorType) -> Vec<u8> {
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 95)
        .encode(px, w, h, t)
        .expect("jpeg encode");
    out
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("usage: uhdr_fixture <out.jpg>");
    let mut data = Vec::with_capacity((W * H * 16) as usize);
    for y in 0..H {
        for x in 0..W {
            let [r, g, b] = pixel_nits(x, y).map(|n| n / SDR_WHITE_NITS);
            for v in [r, g, b, 1.0] {
                data.extend(v.to_le_bytes());
            }
        }
    }
    let hdr = RawImage {
        width: W,
        height: H,
        format: PixelFormat::Rgba32F,
        gamut: ColorGamut::Bt709,
        transfer: ColorTransfer::Linear,
        data,
        stride: W * 16,
    };
    // SDR base: ultrahdr-core's tone mapper. Gain map: full resolution, single channel.
    let sdr_px = tonemap_image_to_srgb8(&hdr, ColorGamut::Bt709).expect("tonemap");
    let sdr = RawImage {
        width: W,
        height: H,
        format: PixelFormat::Rgba8,
        gamut: ColorGamut::Bt709,
        transfer: ColorTransfer::Srgb,
        data: sdr_px,
        stride: W * 4,
    };
    let boost = PEAK_NITS / SDR_WHITE_NITS;
    let cfg = GainMapConfig {
        scale_factor: 1,
        max_boost: boost,
        alternate_hdr_headroom: boost,
        ..Default::default()
    };
    let (gm, meta) = compute_gainmap(&hdr, &sdr, &cfg, Unstoppable).expect("gain map");
    let rgb: Vec<u8> = sdr
        .data
        .chunks(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    let base = jpeg(&rgb, W, H, ExtendedColorType::Rgb8);
    let gm_jpeg = jpeg(&gm.data, gm.width, gm.height, ExtendedColorType::L8);
    // Writes XMP (hdrgm) and ISO 21496-1 metadata, MPF, and an sRGB ICC profile.
    let file = encode_ultrahdr(&base, &gm_jpeg, &meta, ColorGamut::Bt709).expect("assemble");
    let check = Decoder::new(&file).expect("re-parse");
    assert!(check.is_ultrahdr() && check.gainmap_jpeg().is_some());
    std::fs::write(&out, &file).expect("write");
    println!(
        "{out}: {} bytes, HDRCapacityMax log2 {:.4}, GainMapMax log2 {:.4}",
        file.len(),
        meta.alternate_hdr_headroom,
        meta.gain_map_max[0]
    );
}
