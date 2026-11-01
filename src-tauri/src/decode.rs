// decode: every source format -> ImageInfo, a WebView2-readable preview, and a Source for export.
// Formats are recognised by content, never by extension.
use crate::color::{Gamut, Transfer, REF_WHITE_NITS};
use crate::model::{ImageInfo, Rect};
use crate::source::{self, Pixels, Source};
use crate::{uhdr, wic};
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageReader};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Jpeg,
    UltraHdr,
    Png,
    Tiff,
    Webp,
    Avif,
    Jxl,
}

impl Format {
    pub fn name(self) -> &'static str {
        match self {
            Format::Jpeg => "jpeg",
            Format::UltraHdr => "ultrahdr",
            Format::Png => "png",
            Format::Tiff => "tiff",
            Format::Webp => "webp",
            Format::Avif => "avif",
            Format::Jxl => "jxl",
        }
    }
}

/// Longest side of a preview proxy.
const PROXY_MAX: u32 = 8192;
const AVIF_HELP: &str =
    "Windows could not read this AVIF image. Install the AV1 Video Extension and \
    the HEIF Image Extensions from the Microsoft Store, then open it again.";

fn read_err(path: &str, e: impl std::fmt::Display) -> String {
    format!("Could not read {path}: {e}")
}

pub fn sniff(path: &str) -> Result<Format, String> {
    let mut head = [0u8; 32];
    let n = std::fs::File::open(path)
        .and_then(|mut f| f.read(&mut head))
        .map_err(|e| read_err(path, e))?;
    let h = &head[..n];
    let fmt = if h.starts_with(&[0xFF, 0xD8, 0xFF]) {
        let data = std::fs::read(path).map_err(|e| read_err(path, e))?;
        if uhdr::is_ultrahdr(&data) {
            Format::UltraHdr
        } else {
            Format::Jpeg
        }
    } else if h.starts_with(b"\x89PNG\r\n\x1a\n") {
        Format::Png
    } else if h.starts_with(b"II*\0") || h.starts_with(b"MM\0*") {
        Format::Tiff
    } else if h.len() >= 12 && &h[..4] == b"RIFF" && &h[8..12] == b"WEBP" {
        Format::Webp
    } else if h.len() >= 12
        && &h[4..8] == b"ftyp"
        && h.windows(4).any(|b| b == b"avif" || b == b"avis")
    {
        Format::Avif
    } else if h.starts_with(&[0xFF, 0x0A]) || h.starts_with(b"\0\0\0\x0cJXL \r\n\x87\n") {
        Format::Jxl
    } else {
        return Err(format!(
            "{path} is not an image this app can open. It opens JPEG, Ultra HDR JPEG, PNG, TIFF, \
             WebP, AVIF and JPEG XL."
        ));
    };
    Ok(fmt)
}

/// (transfer, gamut) from an ICC profile's description. Anything unrecognised is treated as sRGB.
pub fn icc_space(icc: Option<&[u8]>) -> (Transfer, Gamut) {
    let d = icc
        .and_then(icc_description)
        .unwrap_or_default()
        .to_lowercase();
    if d.contains("p3") {
        (Transfer::Srgb, Gamut::DisplayP3)
    } else if d.contains("2020") || d.contains("2100") {
        (Transfer::Gamma(2.4), Gamut::Bt2020)
    } else if d.contains("adobe rgb") {
        (Transfer::Gamma(563.0 / 256.0), Gamut::AdobeRgb)
    } else if d.contains("prophoto") || d.contains("romm") {
        (Transfer::Gamma(1.8), Gamut::ProPhoto)
    } else {
        (Transfer::Srgb, Gamut::Bt709)
    }
}

/// The ICC 'desc' tag text (v2 'desc' or v4 'mluc'). None for anything malformed.
fn icc_description(icc: &[u8]) -> Option<String> {
    let be32 = |o: usize| -> Option<usize> {
        Some(u32::from_be_bytes(icc.get(o..o + 4)?.try_into().ok()?) as usize)
    };
    let count = be32(128)?.min(1000);
    let entry = (0..count)
        .map(|i| 132 + i * 12)
        .find(|&e| icc.get(e..e + 4) == Some(b"desc"))?;
    let (off, len) = (be32(entry + 4)?, be32(entry + 8)?);
    let tag = icc.get(off..off.checked_add(len)?)?;
    match tag.get(0..4)? {
        b"desc" => {
            let n = be32(off + 8)?;
            let text = tag.get(12..12 + n.saturating_sub(1))?;
            Some(String::from_utf8_lossy(text).into())
        }
        b"mluc" => {
            let (slen, soff) = (be32(off + 20)?, be32(off + 24)?);
            let u: Vec<u16> = tag
                .get(soff..soff.checked_add(slen)?)?
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_be_bytes(*c))
                .collect();
            Some(String::from_utf16_lossy(&u))
        }
        _ => None,
    }
}

fn image_decoder(path: &str) -> Result<impl ImageDecoder, String> {
    ImageReader::open(path)
        .and_then(|r| r.with_guessed_format())
        .map_err(|e| read_err(path, e))?
        .into_decoder()
        .map_err(|e| read_err(path, e))
}

fn turned(o: Orientation) -> bool {
    use Orientation::*;
    matches!(o, Rotate90 | Rotate270 | Rotate90FlipH | Rotate270FlipH)
}

/// Displayed size (EXIF orientation applied) and orientation, from the header only.
fn header(path: &str) -> Result<(u32, u32, Orientation), String> {
    let mut d = image_decoder(path)?;
    let o = d.orientation().unwrap_or(Orientation::NoTransforms);
    let (w, h) = d.dimensions();
    Ok(if turned(o) { (h, w, o) } else { (w, h, o) })
}

/// Lays a transparent image onto `fill` (sRGB bytes). It blends the encoded values, as the browser does
/// when the light table shows the image over the fill, so the export matches the preview.
pub(crate) fn flatten(img: DynamicImage, fill: [u8; 3]) -> DynamicImage {
    if !img.color().has_alpha() {
        return img;
    }
    let rgba = img.into_rgba16();
    let (w, h) = rgba.dimensions();
    let f = fill.map(|c| c as f32 * 257.0);
    let px = rgba
        .pixels()
        .flat_map(|p| {
            let a = p[3] as f32 / 65535.0;
            [0, 1, 2].map(|i| (p[i] as f32 * a + f[i] * (1.0 - a)).round() as u16)
        })
        .collect();
    DynamicImage::ImageRgb16(image::ImageBuffer::from_raw(w, h, px).unwrap())
}

/// JPEG (base image for Ultra HDR), PNG, TIFF and WebP through the image crate.
fn load_image(path: &str, fill: [u8; 3]) -> Result<Source, String> {
    let mut d = image_decoder(path)?;
    let icc = d.icc_profile().ok().flatten();
    let o = d.orientation().unwrap_or(Orientation::NoTransforms);
    let img = flatten(
        DynamicImage::from_decoder(d).map_err(|e| read_err(path, e))?,
        fill,
    );
    let (w, h) = (img.width(), img.height());
    let px = if img.color().bytes_per_pixel() / img.color().channel_count() > 1 {
        Pixels::U16(img.into_rgb16().into_raw())
    } else {
        Pixels::U8(img.into_rgb8().into_raw())
    };
    let (transfer, gamut) = icc_space(icc.as_deref());
    Ok(Source::new((w, h), px, transfer, gamut, 1.0, false).orient(o))
}

fn load_ultrahdr(path: &str) -> Result<Source, String> {
    let data = std::fs::read(path).map_err(|e| read_err(path, e))?;
    let (w, h, px, gamut) = uhdr::decode_hdr(&data)?;
    let o = header(path)?.2;
    Ok(Source::new((w, h), Pixels::F16(px), Transfer::Linear, gamut, 1.0, true).orient(o))
}

fn load_avif(path: &str) -> Result<Source, String> {
    let (w, h, px, hdr) = wic::read(path).map_err(|_| AVIF_HELP.to_string())?;
    Ok(if hdr {
        // scRGB: 1.0 = 80 nits; SDR white is the 203-nit reference.
        let scale = 80.0 / REF_WHITE_NITS;
        Source::new((w, h), px, Transfer::Linear, Gamut::Bt709, scale, true)
    } else {
        Source::new((w, h), px, Transfer::Srgb, Gamut::Bt709, 1.0, false)
    })
}

/// (transfer, gamut) from the CICP code points jxl-oxide renders to.
fn cicp_space(cicp: Option<[u8; 4]>) -> (Transfer, Gamut) {
    let Some([p, t, ..]) = cicp else {
        return (Transfer::Srgb, Gamut::Bt709);
    };
    let gamut = match p {
        9 => Gamut::Bt2020,
        11 | 12 => Gamut::DisplayP3,
        _ => Gamut::Bt709,
    };
    let transfer = match t {
        8 => Transfer::Linear,
        16 => Transfer::Pq,
        18 => Transfer::Hlg,
        1 | 6 | 14 | 15 => Transfer::Gamma(2.4),
        _ => Transfer::Srgb,
    };
    (transfer, gamut)
}

fn open_jxl(path: &str) -> Result<jxl_oxide::JxlImage, String> {
    jxl_oxide::JxlImage::builder()
        .open(path)
        .map_err(|e| read_err(path, e))
}

fn load_jxl(path: &str) -> Result<Source, String> {
    let img = open_jxl(path)?;
    let (transfer, gamut) = cicp_space(img.rendered_cicp());
    let render = img.render_frame(0).map_err(|e| read_err(path, e))?;
    let mut stream = render.stream_no_alpha();
    let (w, h, c) = (stream.width(), stream.height(), stream.channels() as usize);
    let mut buf = vec![0u16; w as usize * h as usize * c];
    stream.write_to_buffer(&mut buf);
    drop(render);
    let px = match c {
        3 => buf,
        _ => buf.chunks_exact(c).flat_map(|p| [p[0]; 3]).collect(),
    };
    let hdr = matches!(transfer, Transfer::Pq | Transfer::Hlg);
    let scale = if hdr { 1.0 / REF_WHITE_NITS } else { 1.0 };
    Ok(Source::new(
        (w, h),
        Pixels::U16(px),
        transfer,
        gamut,
        scale,
        hdr,
    ))
}

/// Loads the whole image for export. `want_hdr` picks the HDR rendition of an Ultra HDR JPEG; without
/// it the JPEG's own SDR base image is used. PQ/HLG/HDR AVIF sources are always HDR. Transparent PNG,
/// TIFF and WebP are laid onto `fill` (sRGB bytes).
pub fn load(path: &str, want_hdr: bool, fill: [u8; 3]) -> Result<Source, String> {
    match sniff(path)? {
        Format::UltraHdr if want_hdr => load_ultrahdr(path),
        Format::Jpeg | Format::UltraHdr | Format::Png | Format::Tiff | Format::Webp => {
            load_image(path, fill)
        }
        Format::Avif => load_avif(path),
        Format::Jxl => load_jxl(path),
    }
}

/// Reads the header (and writes a preview proxy into `cache_dir` for formats WebView2 cannot show).
pub fn open(path: &str, cache_dir: &Path) -> Result<ImageInfo, String> {
    let format = sniff(path)?;
    let (width, height, hdr) = match format {
        Format::Avif => wic::probe(path).map_err(|_| AVIF_HELP.to_string())?,
        Format::Jxl => {
            let img = open_jxl(path)?;
            let hd = img.image_header();
            let (w, h) = (hd.width_with_orientation(), hd.height_with_orientation());
            (w, h, img.hdr_type().is_some())
        }
        _ => {
            let (w, h, _) = header(path)?;
            (w, h, format == Format::UltraHdr)
        }
    };
    let preview_path = match format {
        Format::Tiff | Format::Jxl => proxy(path, cache_dir, hdr)?,
        _ => path.to_string(),
    };
    Ok(ImageInfo {
        path: path.to_string(),
        preview_path,
        width,
        height,
        format: format.name().into(),
        hdr,
    })
}

/// A JPEG the WebView can show: Ultra HDR for HDR sources, else quality-90 sRGB. Named from a hash of
/// the path and modification time so reopening reuses it.
fn proxy(path: &str, cache_dir: &Path, hdr: bool) -> Result<String, String> {
    let mtime = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map_err(|e| read_err(path, e))?;
    let mut hasher = DefaultHasher::new();
    (path, mtime, hdr).hash(&mut hasher);
    let out: PathBuf = cache_dir.join(format!("{:016x}.jpg", hasher.finish()));
    if out.is_file() {
        return Ok(out.to_string_lossy().into_owned());
    }
    let src = load(path, hdr, [0; 3])?; // the default fill; the proxy is cached per file, not per fill
    let s = (PROXY_MAX as f64 / src.w.max(src.h) as f64).min(1.0);
    let size = |v: u32| ((v as f64 * s).round() as u32).max(1);
    let (w, h) = (size(src.w), size(src.h));
    let whole = Rect {
        x: 0.0,
        y: 0.0,
        w: src.w as f64,
        h: src.h as f64,
    };
    let rgb = source::render(&src, whole, w, h, [0.0; 3])?;
    let (is_hdr, peak) = (src.hdr, src.headroom);
    drop(src);
    let bytes = if is_hdr {
        uhdr::encode(&rgb, w, h, peak)?
    } else {
        let px = crate::export::to_srgb8(&rgb, 1.0);
        uhdr::jpeg_bytes(&px, w, h, image::ExtendedColorType::Rgb8, 90)?
    };
    let write_err = |e: std::io::Error| {
        format!(
            "Could not write the preview into {}: {e}",
            cache_dir.display()
        )
    };
    std::fs::create_dir_all(cache_dir).map_err(write_err)?;
    let tmp = out.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(write_err)?;
    std::fs::rename(&tmp, &out).map_err(write_err)?;
    Ok(out.to_string_lossy().into_owned())
}

#[cfg(test)]
#[path = "decode_tests.rs"]
pub(crate) mod tests;
