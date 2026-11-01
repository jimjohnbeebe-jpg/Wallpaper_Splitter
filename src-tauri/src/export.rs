// export: render each monitor's tile, write PNG/JPG/JXR named from the template, optional span image.
use crate::color::{self, Rgb};
use crate::model::{ExportFormat, ExportJob, ExportResult, ExportedFile, MonitorInfo};
use crate::{decode, geometry, source, wic};
use half::f16;
use image::{ExtendedColorType, ImageFormat};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Encoded output pixels: 8-bit sRGB RGB for PNG/JPG, scRGB f16 RGBA for JXR.
enum Out {
    Rgb8(Vec<u8>),
    Rgba16f(Vec<f16>),
}

/// Linear BT.709 -> 8-bit sRGB. HDR (peak above 1) is tone mapped first.
pub fn to_srgb8(rgb: &[Rgb], peak: f32) -> Vec<u8> {
    rgb.iter()
        .flat_map(|&p| {
            let p = color::tone_map(p, peak);
            p.map(|v| (color::srgb_oetf(v.clamp(0.0, 1.0)) * 255.0).round() as u8)
        })
        .collect()
}

fn encode(rgb: &[Rgb], jxr: bool, peak: f32, sdr_white_nits: f64) -> Out {
    if jxr {
        let k = (sdr_white_nits / 80.0) as f32;
        Out::Rgba16f(
            rgb.iter()
                .flat_map(|p| [p[0] * k, p[1] * k, p[2] * k, 1.0].map(f16::from_f32))
                .collect(),
        )
    } else {
        Out::Rgb8(to_srgb8(rgb, peak))
    }
}

/// `#rrggbb` -> sRGB bytes. Anything unparsable is black.
pub fn fill_bytes(hex: &str) -> [u8; 3] {
    let h = hex.trim().trim_start_matches('#');
    let c = |i: usize| h.get(i..i + 2).and_then(|s| u8::from_str_radix(s, 16).ok());
    match (h.len(), c(0), c(2), c(4)) {
        (6, Some(r), Some(g), Some(b)) => [r, g, b],
        _ => [0; 3],
    }
}

/// `#rrggbb` -> linear BT.709. Anything unparsable is black.
pub fn parse_fill(hex: &str) -> Rgb {
    fill_bytes(hex).map(|v| color::srgb_eotf(v as f32 / 255.0))
}

fn write(path: &Path, w: u32, h: u32, out: &Out, format: &ExportFormat) -> Result<(), String> {
    let fail = |e: String| format!("Could not write {}: {e}", path.display());
    match (out, format) {
        (Out::Rgba16f(px), _) => {
            // WIC does not truncate an existing file, so start from nothing.
            let _ = std::fs::remove_file(path);
            wic::write_jxr(&path.to_string_lossy(), w, h, px).map_err(|e| fail(e.message()))
        }
        (Out::Rgb8(px), ExportFormat::Jpg { quality }) => {
            let bytes = crate::uhdr::jpeg_bytes(
                px,
                w,
                h,
                ExtendedColorType::Rgb8,
                (*quality).clamp(1, 100),
            )?;
            std::fs::write(path, bytes).map_err(|e| fail(e.to_string()))
        }
        (Out::Rgb8(px), _) => image::save_buffer_with_format(
            path,
            px,
            w,
            h,
            ExtendedColorType::Rgb8,
            ImageFormat::Png,
        )
        .map_err(|e| fail(e.to_string())),
    }
}

fn sanitize(s: &str) -> String {
    let kept: String = s
        .chars()
        .filter(|c| {
            !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') && !c.is_control()
        })
        .collect();
    kept.trim().trim_end_matches(['.', ' ']).to_string()
}

/// One file name from the template. Tokens: {image} {monitor} {index} {w} {h}.
pub fn file_name(
    template: &str,
    image: &str,
    monitor: &str,
    index: usize,
    w: u32,
    h: u32,
    ext: &str,
) -> String {
    // Spaces become hyphens in both names, as the UI's example shows (ui/state.ts renderTemplate).
    let dash = |s: &str| sanitize(&s.split_whitespace().collect::<Vec<_>>().join("-"));
    let (monitor, image) = (dash(monitor), dash(image));
    let name = template
        .replace("{monitor}", &monitor)
        .replace("{index}", &index.to_string())
        .replace("{w}", &w.to_string())
        .replace("{h}", &h.to_string())
        .replace("{image}", &image);
    let name = sanitize(&name);
    let name = if name.is_empty() {
        "wallpaper".into()
    } else {
        name
    };
    format!("{name}.{ext}")
}

/// Keeps two monitors with the same model and size (or the source photo itself) from sharing a file.
/// A fallback name can itself be taken (source `a-2.png`, template `a`), so keep counting until one is free.
fn unique(name: String, used: &mut HashSet<String>, index: usize) -> String {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), format!(".{e}")),
        None => (name.clone(), String::new()),
    };
    let mut candidate = name;
    let mut n = index;
    while used.contains(&candidate.to_lowercase()) {
        candidate = format!("{stem}-{n}{ext}");
        n += 1;
    }
    used.insert(candidate.to_lowercase());
    candidate
}

fn pictures_folder() -> Result<PathBuf, String> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{FOLDERID_Pictures, SHGetKnownFolderPath, KNOWN_FOLDER_FLAG};
    unsafe {
        let p = SHGetKnownFolderPath(&FOLDERID_Pictures, KNOWN_FOLDER_FLAG(0), None)
            .map_err(|e| format!("Could not find your Pictures folder: {}", e.message()))?;
        let s = p.to_string();
        CoTaskMemFree(Some(p.0 as *const _));
        s.map(PathBuf::from)
            .map_err(|_| "Could not read the Pictures folder path.".into())
    }
}

/// The span image under construction, in the output encoding.
struct Canvas {
    w: u32,
    h: u32,
    origin: (i32, i32),
    out: Out,
}

impl Canvas {
    fn new(span: crate::model::Span, fill: Out) -> Self {
        let n = span.w as usize * span.h as usize;
        let out = match fill {
            Out::Rgb8(p) => Out::Rgb8(p.repeat(n)),
            Out::Rgba16f(p) => Out::Rgba16f(p.repeat(n)),
        };
        Canvas {
            w: span.w,
            h: span.h,
            origin: (span.origin_x, span.origin_y),
            out,
        }
    }

    /// Copies a `w` x `h` tile so its top-left lands at desktop pixel (x, y), clipped to the canvas.
    fn paste(&mut self, tile: &Out, w: u32, h: u32, x: f64, y: f64) {
        let (ox, oy) = (
            x.round() as i64 - self.origin.0 as i64,
            y.round() as i64 - self.origin.1 as i64,
        );
        let (cw, ch) = (self.w as i64, self.h as i64);
        for ty in 0..h as i64 {
            let cy = oy + ty;
            let (x0, x1) = (ox.max(0), (ox + w as i64).min(cw));
            if cy < 0 || cy >= ch || x0 >= x1 {
                continue;
            }
            let (src, dst) = ((ty * w as i64 + x0 - ox) as usize, (cy * cw + x0) as usize);
            let n = (x1 - x0) as usize;
            match (&mut self.out, tile) {
                (Out::Rgb8(d), Out::Rgb8(s)) => {
                    d[dst * 3..(dst + n) * 3].copy_from_slice(&s[src * 3..(src + n) * 3])
                }
                (Out::Rgba16f(d), Out::Rgba16f(s)) => {
                    d[dst * 4..(dst + n) * 4].copy_from_slice(&s[src * 4..(src + n) * 4])
                }
                _ => {}
            }
        }
    }
}

fn create(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("Could not create the folder {}: {e}", dir.display()))
}

/// `root`\<stem>, created; or `root`\<stem> (2), (3)… when that one holds another photo's files. Each
/// default folder keeps its photo's full path in `.source`, so two photos with the same name never
/// overwrite each other's wallpapers.
fn default_folder(root: &Path, stem: &str, image: &Path) -> Result<PathBuf, String> {
    let me = std::path::absolute(image).unwrap_or_else(|_| image.to_path_buf());
    let me = me.to_string_lossy();
    let name = sanitize(stem);
    let mut n = 1;
    loop {
        let dir = match n {
            1 => root.join(&name),
            _ => root.join(format!("{name} ({n})")),
        };
        n += 1;
        let marker = dir.join(".source");
        if let Ok(other) = std::fs::read_to_string(&marker) {
            if !other.trim().eq_ignore_ascii_case(&me) {
                continue;
            }
        }
        create(&dir)?;
        std::fs::write(&marker, me.as_bytes())
            .map_err(|e| format!("Could not write {}: {e}", marker.display()))?;
        return Ok(dir);
    }
}

/// `folder`, or the default folder under Pictures\Wallpaper Splitter, created. Also returns the file names
/// already taken: the source photo's own, when it lives in that folder, so an export never overwrites it.
fn output_folder(
    folder: &str,
    image: &Path,
    stem: &str,
) -> Result<(PathBuf, HashSet<String>), String> {
    let folder = if folder.trim().is_empty() {
        default_folder(&pictures_folder()?.join("Wallpaper Splitter"), stem, image)?
    } else {
        let f = PathBuf::from(folder.trim());
        create(&f)?;
        f
    };
    let canon = |p: &Path| p.canonicalize().ok();
    let mut used = HashSet::new();
    if image.parent().and_then(canon).is_some() && image.parent().and_then(canon) == canon(&folder)
    {
        let name = image.file_name().unwrap_or_default().to_string_lossy();
        used.insert(name.to_lowercase());
    }
    Ok((folder, used))
}

/// Writes one file per monitor (and the span image when asked). `sdr_white_nits` scales HDR output.
pub fn export(
    job: &ExportJob,
    monitors: &[MonitorInfo],
    sdr_white_nits: f64,
) -> Result<ExportResult, String> {
    let prefs = &job.prefs;
    let jxr = matches!(prefs.format, ExportFormat::Jxr);
    let src = decode::load(&job.image, jxr, fill_bytes(&prefs.fill))?;
    if jxr && !src.hdr {
        return Err(
            "JPEG XR is for HDR images, and this image is SDR. Choose PNG or JPG instead.".into(),
        );
    }
    let layout = geometry::compute_layout(src.w, src.h, &job.monitors, &job.framing);
    if layout.tiles.is_empty() {
        return Err("There are no monitors to export for.".into());
    }
    let image_path = Path::new(&job.image);
    let stem = image_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (folder, mut used) = output_folder(&prefs.folder, image_path, &stem)?;
    let ext = match prefs.format {
        ExportFormat::Png => "png",
        ExportFormat::Jpg { .. } => "jpg",
        ExportFormat::Jxr => "jxr",
    };
    let fill = parse_fill(&prefs.fill);
    let enc = |rgb: &[Rgb]| encode(rgb, jxr, src.headroom, sdr_white_nits);
    let mut canvas = prefs
        .span
        .then(|| Canvas::new(geometry::span_canvas(monitors), enc(&[fill])));

    // {index} counts left to right (then top to bottom) on the desk; tiles follow job.monitors.
    let mut order: Vec<usize> = (0..layout.tiles.len()).collect();
    let key = |i: usize| (job.monitors[i].mm.x, job.monitors[i].mm.y);
    order.sort_by(|&a, &b| {
        key(a)
            .partial_cmp(&key(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut files = Vec::new();
    for (rank, &i) in order.iter().enumerate() {
        let tile = &layout.tiles[i];
        let info = monitors.iter().find(|m| m.id == tile.monitor_id);
        let name = info.map_or("monitor", |m| m.name.as_str());
        let out = enc(&source::render(
            &src, tile.crop, tile.out_w, tile.out_h, fill,
        )?);
        let file = file_name(
            &prefs.template,
            &stem,
            name,
            rank + 1,
            tile.out_w,
            tile.out_h,
            ext,
        );
        let path = folder.join(unique(file, &mut used, rank + 1));
        write(&path, tile.out_w, tile.out_h, &out, &prefs.format)?;
        if let (Some(c), Some(m)) = (canvas.as_mut(), info) {
            c.paste(&out, tile.out_w, tile.out_h, m.px.x, m.px.y);
        }
        files.push(ExportedFile {
            monitor_id: tile.monitor_id.clone(),
            path: path.to_string_lossy().into_owned(),
        });
    }
    let span_file = match canvas {
        Some(c) if c.w > 0 && c.h > 0 => {
            let file = file_name(&prefs.template, &stem, "span", 0, c.w, c.h, ext);
            let path = folder.join(unique(file, &mut used, 0));
            write(&path, c.w, c.h, &c.out, &prefs.format)?;
            Some(path.to_string_lossy().into_owned())
        }
        _ => None,
    };
    Ok(ExportResult {
        files,
        span_file,
        folder: folder.to_string_lossy().into_owned(),
    })
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
