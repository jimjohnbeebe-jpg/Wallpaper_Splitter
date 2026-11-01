// uhdr: Ultra HDR (gain-map) JPEG. Decode = base JPEG + gain map JPEG (zune-jpeg) through ultrahdr-rs's
// apply_gainmap; encode (for HDR preview proxies) = image's JpegEncoder + encode_ultrahdr. Built without
// ultrahdr-rs's AGPL zencodec feature, so its own Decoder::decode_hdr is not available (spike notes).
#![allow(dead_code)]
use crate::color::{Gamut, Rgb};
use half::f16;
use image::codecs::jpeg::JpegEncoder;
use image::ExtendedColorType;
use ultrahdr_core::metadata::iso21496::parse_iso21496;
use ultrahdr_core::Iso21496Format;
use ultrahdr_rs::color::tonemap::tonemap_image_to_srgb8;
use ultrahdr_rs::gainmap::apply::apply_gainmap;
use ultrahdr_rs::gainmap::compute::compute_gainmap;
use ultrahdr_rs::{
    encode_ultrahdr, ColorGamut, ColorTransfer, Decoder, GainMap, GainMapConfig, GainMapMetadata,
    HdrOutputFormat, PixelFormat, RawImage, Unstoppable,
};
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

const ISO_NS: &[u8] = b"urn:iso:std:iso:ts:21496:-1\0";

pub fn is_ultrahdr(data: &[u8]) -> bool {
    Decoder::new(data).is_ok_and(|d| d.is_ultrahdr() && d.gainmap_jpeg().is_some())
}

/// 8-bit pixels in `cs`, plus the ICC profile and the JPEG's component count.
type Decoded = (u32, u32, Vec<u8>, Option<Vec<u8>>);

fn zune(jpeg: &[u8], cs: ColorSpace) -> Result<Decoded, String> {
    let opts = DecoderOptions::default()
        .jpeg_set_out_colorspace(cs)
        .set_max_width(1 << 16)
        .set_max_height(1 << 16);
    let mut d = JpegDecoder::new_with_options(std::io::Cursor::new(jpeg), opts);
    let px = d
        .decode()
        .map_err(|e| format!("The JPEG could not be decoded: {e}"))?;
    let info = d.info().ok_or("The JPEG has no image header.")?;
    Ok((info.width as u32, info.height as u32, px, d.icc_profile()))
}

fn components(jpeg: &[u8]) -> Result<u8, String> {
    let mut d = JpegDecoder::new(std::io::Cursor::new(jpeg));
    d.decode_headers()
        .map_err(|e| format!("The gain map is unreadable: {e}"))?;
    Ok(d.info().map_or(1, |i| i.components))
}

/// ultrahdr-rs 0.3.5 misreads Lightroom's per-channel XMP (GainMapMax/Gamma come back 0), so the
/// ISO 21496-1 APP2 block in the gain-map JPEG goes first and XMP is the fallback.
fn metadata(dec: &Decoder, gm_jpeg: &[u8]) -> Option<GainMapMetadata> {
    ultrahdr_rs::container::scan_segments(gm_jpeg)
        .iter()
        .filter(|s| s.marker_num == 2 && s.data.starts_with(ISO_NS))
        .find_map(|s| parse_iso21496(&s.data[ISO_NS.len()..], Iso21496Format::JpegApp2).ok())
        .or_else(|| dec.metadata().cloned())
}

fn to_core_gamut(g: Gamut) -> ColorGamut {
    match g {
        Gamut::DisplayP3 => ColorGamut::DisplayP3,
        Gamut::Bt2020 => ColorGamut::Bt2020,
        _ => ColorGamut::Bt709,
    }
}

/// The full HDR rendition: linear RGB f16 in the base image's gamut, 1.0 = SDR white.
pub fn decode_hdr(data: &[u8]) -> Result<(u32, u32, Vec<f16>, Gamut), String> {
    let bad = |e: String| format!("This Ultra HDR JPEG could not be read: {e}");
    let dec = Decoder::new(data).map_err(|e| bad(e.to_string()))?;
    let gm_jpeg = dec
        .gainmap_jpeg()
        .ok_or_else(|| bad("no gain map".into()))?;
    let meta = metadata(&dec, gm_jpeg).ok_or_else(|| bad("no gain map metadata".into()))?;
    let primary = dec
        .primary_jpeg()
        .ok_or_else(|| bad("no base image".into()))?;
    let (w, h, rgb, icc) = zune(primary, ColorSpace::RGB)?;
    let gamut = crate::decode::icc_space(icc.or_else(|| dec.icc_profile()).as_deref()).1;
    let channels = if components(gm_jpeg)? == 1 { 1 } else { 3 };
    let cs = if channels == 1 {
        ColorSpace::Luma
    } else {
        ColorSpace::RGB
    };
    let (gw, gh, gdata, _) = zune(gm_jpeg, cs)?;
    let boost = 2f32.powf(meta.alternate_hdr_headroom as f32);
    // A full-resolution gain map (Lightroom's) maps row for row, so apply it in strips to keep memory
    // low. ponytail: other gain-map sizes are applied in one pass (Rgba32F of the whole image).
    let rows = if (gw, gh) == (w, h) { 256 } else { h };
    let mut out = Vec::with_capacity(w as usize * h as usize * 3);
    for y in (0..h).step_by(rows as usize) {
        let rh = rows.min(h - y);
        let sdr = RawImage {
            width: w,
            height: rh,
            format: PixelFormat::Rgb8,
            gamut: to_core_gamut(gamut),
            transfer: ColorTransfer::Srgb,
            data: rgb[(y * w * 3) as usize..((y + rh) * w * 3) as usize].to_vec(),
            stride: w * 3,
        };
        let (gy, grh) = if rows == h { (0, gh) } else { (y, rh) };
        let c = channels as u32;
        let gm = GainMap {
            width: gw,
            height: grh,
            channels,
            data: gdata[(gy * gw * c) as usize..((gy + grh) * gw * c) as usize].to_vec(),
        };
        let hdr = apply_gainmap(
            &sdr,
            &gm,
            &meta,
            boost,
            HdrOutputFormat::LinearFloat,
            Unstoppable,
        )
        .map_err(|e| bad(e.to_string()))?;
        for p in hdr.data.as_chunks::<16>().0 {
            let c = |k: usize| f32::from_le_bytes([p[k], p[k + 1], p[k + 2], p[k + 3]]);
            out.extend([c(0), c(4), c(8)].map(f16::from_f32));
        }
    }
    Ok((w, h, out, gamut))
}

pub fn jpeg_bytes(
    px: &[u8],
    w: u32,
    h: u32,
    t: ExtendedColorType,
    q: u8,
) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, q)
        .encode(px, w, h, t)
        .map_err(|e| format!("JPEG encoding failed: {e}"))?;
    Ok(out)
}

/// Ultra HDR JPEG from linear BT.709 (1.0 = SDR white = 203 nits) whose brightest value is `peak`.
pub fn encode(rgb: &[Rgb], w: u32, h: u32, peak: f32) -> Result<Vec<u8>, String> {
    let fail = |e: String| format!("The HDR preview could not be written: {e}");
    let data: Vec<u8> = rgb
        .iter()
        .flat_map(|p| [p[0].max(0.0), p[1].max(0.0), p[2].max(0.0), 1.0])
        .flat_map(f32::to_le_bytes)
        .collect();
    let hdr = RawImage {
        width: w,
        height: h,
        format: PixelFormat::Rgba32F,
        gamut: ColorGamut::Bt709,
        transfer: ColorTransfer::Linear,
        data,
        stride: w * 16,
    };
    let sdr_px =
        tonemap_image_to_srgb8(&hdr, ColorGamut::Bt709).map_err(|e| fail(e.to_string()))?;
    let sdr = RawImage {
        width: w,
        height: h,
        format: PixelFormat::Rgba8,
        gamut: ColorGamut::Bt709,
        transfer: ColorTransfer::Srgb,
        data: sdr_px,
        stride: w * 4,
    };
    let boost = peak.max(1.01);
    let cfg = GainMapConfig {
        scale_factor: 1,
        max_boost: boost,
        alternate_hdr_headroom: boost,
        ..Default::default()
    };
    let (gm, meta) =
        compute_gainmap(&hdr, &sdr, &cfg, Unstoppable).map_err(|e| fail(e.to_string()))?;
    drop(hdr);
    let base: Vec<u8> = sdr
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    let base = jpeg_bytes(&base, w, h, ExtendedColorType::Rgb8, 90)?;
    let gmj = jpeg_bytes(&gm.data, gm.width, gm.height, ExtendedColorType::L8, 90)?;
    encode_ultrahdr(&base, &gmj, &meta, ColorGamut::Bt709).map_err(|e| fail(e.to_string()))
}
