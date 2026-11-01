// source: a decoded image kept compact (u8/u16/f16 RGB) plus its colour encoding, and rendering one crop
// of it to linear BT.709 f32 (1.0 = SDR white) at an exact output size. Only the visible part of a crop
// (plus the Lanczos margin) is ever converted to f32, so memory stays near the size of the source.
use crate::color::{self, Gamut, Mat3, Rgb, Transfer};
use crate::model::Rect;
use fast_image_resize::images::{TypedImage, TypedImageRef};
use fast_image_resize::pixels::F32x3;
use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer};
use half::f16;
use image::metadata::Orientation;
use image::{DynamicImage, ImageBuffer};

/// Interleaved RGB samples. U8/U16 are code values; F16 is already linear.
pub enum Pixels {
    U8(Vec<u8>),
    U16(Vec<u16>),
    F16(Vec<f16>),
}

pub struct Source {
    pub w: u32,
    pub h: u32,
    pub px: Pixels,
    pub transfer: Transfer,
    pub gamut: Gamut,
    /// Multiplies the transfer's output to reach 1.0 = SDR white (PQ/HLG nits -> 1/203).
    pub scale: f32,
    /// The source carries light above SDR white (PQ, HLG, Ultra HDR, HDR AVIF).
    pub hdr: bool,
    /// The brightest value in the image, in SDR-white units (1.0 for SDR).
    pub headroom: f32,
}

/// Turns stored samples into linear BT.709.
struct Linearise {
    lut: Vec<f32>,
    transfer: Transfer,
    scale: f32,
    matrix: Option<Mat3>,
}

impl Linearise {
    fn new(src: &Source) -> Self {
        let max = match src.px {
            Pixels::U8(_) => 255,
            Pixels::U16(_) => 65535,
            Pixels::F16(_) => 0,
        };
        let decode = |v: f32| match src.transfer {
            Transfer::Srgb => color::srgb_eotf(v),
            Transfer::Gamma(g) => v.max(0.0).powf(g),
            Transfer::Linear => v,
            Transfer::Pq => color::pq_eotf(v),
            Transfer::Hlg => color::hlg_inverse_oetf(v),
        };
        let lut = if max == 0 {
            Vec::new()
        } else {
            (0..=max).map(|c| decode(c as f32 / max as f32)).collect()
        };
        Linearise {
            lut,
            transfer: src.transfer,
            scale: src.scale,
            matrix: (src.gamut != Gamut::Bt709).then(|| color::to_bt709(src.gamut)),
        }
    }

    /// Pixel starting at sample index `i`.
    #[inline]
    fn at(&self, px: &Pixels, i: usize) -> Rgb {
        let c = match px {
            Pixels::U8(v) => [0, 1, 2].map(|k| self.lut[v[i + k] as usize]),
            Pixels::U16(v) => [0, 1, 2].map(|k| self.lut[v[i + k] as usize]),
            Pixels::F16(v) => [0, 1, 2].map(|k| v[i + k].to_f32()),
        };
        self.finish(c)
    }

    fn finish(&self, c: Rgb) -> Rgb {
        let c = if self.transfer == Transfer::Hlg {
            color::hlg_ootf(c)
        } else {
            c
        };
        let c = c.map(|v| v * self.scale);
        match &self.matrix {
            Some(m) => color::apply(m, c),
            None => c,
        }
    }
}

impl Source {
    /// Builds a source and measures its headroom.
    pub fn new(
        (w, h): (u32, u32),
        px: Pixels,
        transfer: Transfer,
        gamut: Gamut,
        scale: f32,
        hdr: bool,
    ) -> Self {
        let mut s = Source {
            w,
            h,
            px,
            transfer,
            gamut,
            scale,
            hdr,
            headroom: 1.0,
        };
        s.measure_headroom();
        s
    }

    fn measure_headroom(&mut self) {
        if !self.hdr {
            self.headroom = 1.0;
            return;
        }
        // The brightest BT.709 channel after conversion: saturated wide-gamut colours outgrow grey.
        let lin = Linearise::new(self);
        let peak = (0..self.w as usize * self.h as usize)
            .map(|p| lin.at(&self.px, p * 3).into_iter().fold(0.0, f32::max))
            .fold(0.0, f32::max);
        self.headroom = peak.max(1.0);
    }

    /// Applies an EXIF-style orientation (rotations and flips only move samples).
    pub fn orient(self, o: Orientation) -> Self {
        if o == Orientation::NoTransforms {
            return self;
        }
        let (w, h) = (self.w, self.h);
        let bits = |v: Vec<f16>| v.into_iter().map(f16::to_bits).collect::<Vec<u16>>();
        let (mut img, float) = match self.px {
            Pixels::U8(v) => (
                DynamicImage::ImageRgb8(ImageBuffer::from_raw(w, h, v).unwrap()),
                false,
            ),
            Pixels::U16(v) => (
                DynamicImage::ImageRgb16(ImageBuffer::from_raw(w, h, v).unwrap()),
                false,
            ),
            Pixels::F16(v) => (
                DynamicImage::ImageRgb16(ImageBuffer::from_raw(w, h, bits(v)).unwrap()),
                true,
            ),
        };
        img.apply_orientation(o);
        let (w, h) = (img.width(), img.height());
        let px = match img {
            DynamicImage::ImageRgb8(b) => Pixels::U8(b.into_raw()),
            DynamicImage::ImageRgb16(b) if float => {
                Pixels::F16(b.into_raw().into_iter().map(f16::from_bits).collect())
            }
            other => Pixels::U16(other.into_rgb16().into_raw()),
        };
        Source { w, h, px, ..self }
    }

    /// Linear BT.709 pixels of the `w` x `h` block at (`x0`, `y0`). Each position is first clamped into
    /// `keep` ([x0, y0, x1, y1], which holds the whole image); anything still outside the image is `fill`.
    /// So past `keep` the nearest edge repeats, and Lanczos never pulls `fill` into an image edge.
    pub fn region(
        &self,
        x0: i64,
        y0: i64,
        w: u32,
        h: u32,
        fill: Rgb,
        keep: [i64; 4],
    ) -> Vec<F32x3> {
        let lin = Linearise::new(self);
        let (sw, sh) = (self.w as i64, self.h as i64);
        let mut out = vec![F32x3::new(fill); w as usize * h as usize];
        for ry in 0..h as i64 {
            let sy = (y0 + ry).clamp(keep[1], keep[3] - 1);
            if sy < 0 || sy >= sh {
                continue;
            }
            let row = &mut out[(ry * w as i64) as usize..][..w as usize];
            for (rx, p) in row.iter_mut().enumerate() {
                let sx = (x0 + rx as i64).clamp(keep[0], keep[2] - 1);
                if (0..sw).contains(&sx) {
                    *p = F32x3::new(lin.at(&self.px, ((sy * sw + sx) * 3) as usize));
                }
            }
        }
        out
    }
}

/// Renders `crop` (source pixels, may reach outside the image) to exactly `out_w` x `out_h` with Lanczos3
/// on linear light. Returns linear BT.709 RGB, row-major.
pub fn render(
    src: &Source,
    crop: Rect,
    out_w: u32,
    out_h: u32,
    fill: Rgb,
) -> Result<Vec<Rgb>, String> {
    let mut out = vec![fill; out_w as usize * out_h as usize];
    if crop.w <= 0.0 || crop.h <= 0.0 || out_w == 0 || out_h == 0 {
        return Ok(out);
    }
    let (sx, sy) = (out_w as f64 / crop.w, out_h as f64 / crop.h);
    // The output block that shows image (snapped outwards to whole pixels), and its source box.
    let span = |c0: f64, len: f64, img: f64, s: f64, n: u32| {
        let a = ((c0.max(0.0) - c0) * s).floor().clamp(0.0, n as f64);
        let b = (((c0 + len).min(img) - c0) * s).ceil().clamp(0.0, n as f64);
        (a as u32, b as u32)
    };
    let (dx0, dx1) = span(crop.x, crop.w, src.w as f64, sx, out_w);
    let (dy0, dy1) = span(crop.y, crop.h, src.h as f64, sy, out_h);
    if dx1 <= dx0 || dy1 <= dy0 {
        return Ok(out);
    }
    let (bx, by) = (crop.x + dx0 as f64 / sx, crop.y + dy0 as f64 / sy);
    let (bw, bh) = ((dx1 - dx0) as f64 / sx, (dy1 - dy0) as f64 / sy);
    // Lanczos3 reaches 3 output pixels each side, i.e. 3/s source pixels when shrinking.
    let margin = (3.0 / sx.min(sy).min(1.0)).ceil() as i64 + 2;
    let (rx, ry) = (bx.floor() as i64 - margin, by.floor() as i64 - margin);
    let rw = ((bx + bw).ceil() as i64 + margin - rx) as u32;
    let rh = ((by + bh).ceil() as i64 + margin - ry) as u32;
    // Fill only where the crop itself leaves the image; past the crop and the image, edges repeat.
    let keep = [
        (crop.x.round() as i64).min(0),
        (crop.y.round() as i64).min(0),
        ((crop.x + crop.w).round() as i64).max(src.w as i64),
        ((crop.y + crop.h).round() as i64).max(src.h as i64),
    ];
    let region = src.region(rx, ry, rw, rh, fill, keep);
    let view = TypedImageRef::new(rw, rh, &region).map_err(|_| "Internal error: bad crop size")?;
    let (tw, th) = (dx1 - dx0, dy1 - dy0);
    let mut dst = TypedImage::<F32x3>::new(tw, th);
    let opts = ResizeOptions::new()
        .resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3))
        .crop(bx - rx as f64, by - ry as f64, bw, bh);
    Resizer::new()
        .resize_typed(&view, &mut dst, &opts)
        .map_err(|e| format!("Resizing failed: {e}"))?;
    drop(region);
    for (y, row) in dst.pixels().chunks_exact(tw as usize).enumerate() {
        let o = (dy0 as usize + y) * out_w as usize + dx0 as usize;
        for (d, p) in out[o..o + tw as usize].iter_mut().zip(row) {
            *d = p.0;
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
