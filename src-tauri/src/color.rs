// color: transfer functions, gamut matrices and the SDR tone map. Linear values use 1.0 = SDR white
// unless a function says otherwise (PQ and HLG work in nits).

pub type Rgb = [f32; 3];
pub type Mat3 = [[f32; 3]; 3];

/// The BT.2408 reference white: PQ/HLG content at this level maps to the SDR white (1.0).
pub const REF_WHITE_NITS: f32 = 203.0;

/// How the stored values of a source map to linear light.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Transfer {
    Srgb,
    Gamma(f32),
    Linear,
    Pq,
    Hlg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gamut {
    Bt709,
    DisplayP3,
    Bt2020,
    AdobeRgb,
    ProPhoto,
}

pub fn srgb_eotf(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

pub fn srgb_oetf(l: f32) -> f32 {
    if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    }
}

const PQ_M1: f32 = 2610.0 / 16384.0;
const PQ_M2: f32 = 2523.0 / 4096.0 * 128.0;
const PQ_C1: f32 = 3424.0 / 4096.0;
const PQ_C2: f32 = 2413.0 / 4096.0 * 32.0;
const PQ_C3: f32 = 2392.0 / 4096.0 * 32.0;

/// SMPTE ST 2084: signal 0..1 -> nits.
pub fn pq_eotf(e: f32) -> f32 {
    let p = e.max(0.0).powf(1.0 / PQ_M2);
    10000.0 * ((p - PQ_C1).max(0.0) / (PQ_C2 - PQ_C3 * p)).powf(1.0 / PQ_M1)
}

/// SMPTE ST 2084 inverse: nits -> signal 0..1.
#[cfg(test)] // the decoders only go signal -> light
pub fn pq_inverse_eotf(nits: f32) -> f32 {
    let y = (nits / 10000.0).max(0.0).powf(PQ_M1);
    ((PQ_C1 + PQ_C2 * y) / (1.0 + PQ_C3 * y)).powf(PQ_M2)
}

const HLG_A: f32 = 0.178_832_77;
const HLG_B: f32 = 0.284_668_92;
const HLG_C: f32 = 0.559_910_7;

/// BT.2100 HLG inverse OETF: signal 0..1 -> scene light 0..1.
pub fn hlg_inverse_oetf(e: f32) -> f32 {
    if e <= 0.5 {
        e * e / 3.0
    } else {
        (((e - HLG_C) / HLG_A).exp() + HLG_B) / 12.0
    }
}

/// HLG signal (BT.2020 primaries) -> display nits through the BT.2100 OOTF for a 1000-nit display.
#[cfg(test)]
pub fn hlg_to_nits(signal: Rgb) -> Rgb {
    hlg_ootf(signal.map(hlg_inverse_oetf))
}

/// BT.2100 HLG OOTF (system gamma 1.2, 1000-nit display): scene light 0..1 -> nits.
pub fn hlg_ootf(s: Rgb) -> Rgb {
    let ys = 0.2627 * s[0] + 0.6780 * s[1] + 0.0593 * s[2];
    let k = 1000.0 * ys.max(0.0).powf(0.2);
    s.map(|v| v * k)
}

type Xy = [f64; 2];
const D65: Xy = [0.3127, 0.3290];
const D50: Xy = [0.3457, 0.3585];

fn primaries(g: Gamut) -> ([Xy; 3], Xy) {
    match g {
        Gamut::Bt709 => ([[0.64, 0.33], [0.30, 0.60], [0.15, 0.06]], D65),
        Gamut::DisplayP3 => ([[0.680, 0.320], [0.265, 0.690], [0.150, 0.060]], D65),
        Gamut::Bt2020 => ([[0.708, 0.292], [0.170, 0.797], [0.131, 0.046]], D65),
        Gamut::AdobeRgb => ([[0.64, 0.33], [0.21, 0.71], [0.15, 0.06]], D65),
        Gamut::ProPhoto => ([[0.7347, 0.2653], [0.1596, 0.8404], [0.0366, 0.0001]], D50),
    }
}

type M64 = [[f64; 3]; 3];

fn mul(a: &M64, b: &M64) -> M64 {
    let mut m = [[0.0; 3]; 3];
    for (i, row) in m.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    m
}

fn inv(m: &M64) -> M64 {
    let c = |r: usize, k: usize| {
        let (r1, r2, k1, k2) = ((r + 1) % 3, (r + 2) % 3, (k + 1) % 3, (k + 2) % 3);
        m[r1][k1] * m[r2][k2] - m[r1][k2] * m[r2][k1]
    };
    let det: f64 = (0..3).map(|k| m[0][k] * c(0, k)).sum();
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = c(j, i) / det; // adjugate: transpose of the cofactors
        }
    }
    out
}

fn xyz(p: Xy) -> [f64; 3] {
    [p[0] / p[1], 1.0, (1.0 - p[0] - p[1]) / p[1]]
}

fn rgb_to_xyz(g: Gamut) -> M64 {
    let (p, w) = primaries(g);
    let cols = p.map(xyz);
    let m = [0, 1, 2].map(|r| [cols[0][r], cols[1][r], cols[2][r]]);
    let s = inv(&m);
    let wv = xyz(w);
    let k: [f64; 3] = [0, 1, 2].map(|i| (0..3).map(|j| s[i][j] * wv[j]).sum());
    [0, 1, 2].map(|r| [m[r][0] * k[0], m[r][1] * k[1], m[r][2] * k[2]])
}

/// Bradford chromatic adaptation D50 -> D65 (only ProPhoto needs it).
fn d50_to_d65() -> M64 {
    let b = [
        [0.8951, 0.2664, -0.1614],
        [-0.7502, 1.7135, 0.0367],
        [0.0389, -0.0685, 1.0296],
    ];
    let (s, d) = (xyz(D50), xyz(D65));
    let cone = |v: [f64; 3]| [0, 1, 2].map(|i| (0..3).map(|j| b[i][j] * v[j]).sum::<f64>());
    let (cs, cd) = (cone(s), cone(d));
    let diag = [
        [cd[0] / cs[0], 0.0, 0.0],
        [0.0, cd[1] / cs[1], 0.0],
        [0.0, 0.0, cd[2] / cs[2]],
    ];
    mul(&inv(&b), &mul(&diag, &b))
}

/// Linear RGB in `g` -> linear BT.709 (out-of-gamut colours come out negative or above 1).
pub fn to_bt709(g: Gamut) -> Mat3 {
    let mut m = rgb_to_xyz(g);
    if primaries(g).1 == D50 {
        m = mul(&d50_to_d65(), &m);
    }
    let m = mul(&inv(&rgb_to_xyz(Gamut::Bt709)), &m);
    m.map(|r| r.map(|v| v as f32))
}

pub fn apply(m: &Mat3, c: Rgb) -> Rgb {
    m.map(|r| r[0] * c[0] + r[1] * c[1] + r[2] * c[2])
}

/// Brings HDR (1.0 = SDR white, brightest value `peak`) into 0..1 for SDR output. Linear below the knee,
/// then an extended-Reinhard shoulder that lands `peak` exactly on 1.0. Works on the largest channel so
/// hue is kept and no channel clips.
pub fn tone_map(c: Rgb, peak: f32) -> Rgb {
    const KNEE: f32 = 0.7;
    let m = c[0].max(c[1]).max(c[2]);
    if peak <= 1.0 || m <= KNEE {
        return c;
    }
    let s = 1.0 - KNEE;
    let w = (peak - KNEE) / s;
    let x = (m.min(peak) - KNEE) / s;
    let mapped = KNEE + s * x * (1.0 + x / (w * w)) / (1.0 + x);
    c.map(|v| v * mapped / m)
}

#[cfg(test)]
#[path = "color_tests.rs"]
mod tests;
