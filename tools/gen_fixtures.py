r"""Writes the tiny synthetic test images in src-tauri/tests/fixtures/ and their README.

Every fixture is 128x64 with the same layout:
  rows 0..47   grey ramp, x = 0..127 maps to 0..1 of the format's range (SDR) or 0..1000 nits (HDR)
  rows 48..63  x 0..63 white (SDR full scale, HDR 203 nits), x 64..95 red (R only), x 96..127 black

Setup (PowerShell, from the repo root):
  py -3.13 -m venv $env:TEMP\wsfix; & $env:TEMP\wsfix\Scripts\pip install -r tools\requirements.txt
  & $env:TEMP\wsfix\Scripts\python tools\gen_fixtures.py
The Ultra HDR JPEG is written by tools/uhdr_fixture (Rust, ultrahdr-rs), so cargo must be on PATH.
Every file is decoded again after writing and checked against the expected values.
"""

import math
import pathlib
import subprocess

import imagecodecs as ic
import numpy as np
import tifffile

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "src-tauri" / "tests" / "fixtures"
W, H = 128, 64
PEAK, PATCH = 1000.0, 203.0  # nits

# Sample points listed in the README: (name, x, y).
SAMPLES = [("ramp x=0", 0, 0), ("ramp x=32", 32, 0), ("ramp x=64", 64, 0), ("ramp x=96", 96, 0),
           ("ramp x=127", 127, 0), ("white patch", 16, 56), ("red patch", 80, 56), ("black", 112, 56)]


def pq(nits):
    """SMPTE ST 2084 inverse EOTF: absolute nits -> signal 0..1."""
    m1, m2, c1, c2, c3 = 0.1593017578125, 78.84375, 0.8359375, 18.8515625, 18.6875
    y = np.power(np.asarray(nits, np.float64) / 10000.0, m1)
    return np.power((c1 + c2 * y) / (1 + c3 * y), m2)


def hlg(nits):
    """BT.2100 HLG OETF for a 1000-nit display (system gamma 1.2, neutral so Ys = E): nits -> signal."""
    a, b, c = 0.17883277, 0.28466892, 0.55991073
    e = np.power(np.asarray(nits, np.float64) / PEAK, 1 / 1.2)
    with np.errstate(invalid="ignore", divide="ignore"):
        return np.where(e <= 1 / 12, np.sqrt(3 * e), a * np.log(np.maximum(12 * e - b, 1e-12)) + c)


def pattern(top, white, red):
    """RGB float image: `top` is the ramp value per column (W,), `white`/`red` the patch levels."""
    img = np.zeros((H, W, 3), np.float64)
    img[:48] = np.asarray(top)[None, :, None]
    img[48:, :64] = white
    img[48:, 64:96, 0] = red
    return img


def quant(img, bits):
    return np.round(img * (2**bits - 1)).astype(np.uint8 if bits == 8 else np.uint16)


def check(name, decoded, expected, tol):
    err = int(np.abs(decoded.astype(np.int64)[..., :3] - expected.astype(np.int64)).max())
    assert err <= tol, f"{name}: max error {err} > {tol}"
    return err


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    x = np.arange(W) / (W - 1)
    nits = PEAK * x
    sdr8, sdr16 = quant(pattern(x, 1, 1), 8), quant(pattern(x, 1, 1), 16)
    pq10 = quant(pattern(pq(nits), pq(PATCH), pq(PATCH)), 10)
    pq16 = quant(pattern(pq(nits), pq(PATCH), pq(PATCH)), 16)
    hlg16 = quant(pattern(hlg(nits), hlg(PATCH), hlg(PATCH)), 16)

    files = {}  # name -> (bytes, decoder, expected, tolerance, description, bits)
    files["sdr8.png"] = (ic.png_encode(sdr8), ic.png_decode, sdr8, 0, "8-bit sRGB PNG", 8)
    files["sdr16.png"] = (ic.png_encode(sdr16), ic.png_decode, sdr16, 0, "16-bit sRGB PNG", 16)
    files["sdr8.webp"] = (ic.webp_encode(sdr8, lossless=True), ic.webp_decode, sdr8, 0,
                          "8-bit sRGB WebP, lossless", 8)
    files["sdr8.jpg"] = (ic.jpeg8_encode(sdr8, 95, subsampling="444"), ic.jpeg8_decode, sdr8, 12,
                         "8-bit sRGB JPEG, quality 95, 4:4:4", 8)
    files["hdr_pq10_bt2020.avif"] = (
        ic.avif_encode(pq10, 95, bitspersample=10, pixelformat="YUV444", speed=6,
                       primaries="BT2020", transfer="PQ", matrix="BT2020_NCL"),
        ic.avif_decode, pq10, 8, "10-bit AVIF, BT.2020 primaries, PQ, BT.2020 NCL matrix, 4:4:4, quality 95", 10)
    files["hdr_pq16_bt2020.jxl"] = (
        ic.jpegxl_encode(pq16, lossless=True, bitspersample=16, primaries="BT2100", transfer="PQ"),
        ic.jpegxl_decode, pq16, 0, "16-bit JPEG XL, lossless, BT.2020 primaries, PQ", 16)
    files["hdr_hlg16_bt2020.jxl"] = (
        ic.jpegxl_encode(hlg16, lossless=True, bitspersample=16, primaries="BT2100", transfer="HLG"),
        ic.jpegxl_decode, hlg16, 0, "16-bit JPEG XL, lossless, BT.2020 primaries, HLG", 16)

    for name, (data, dec, exp, tol, _, _) in files.items():
        (OUT / name).write_bytes(data)
        print(f"{name}: {len(data)} bytes, max error {check(name, dec(data), exp, tol)}")

    tif = OUT / "sdr16.tif"
    tifffile.imwrite(tif, sdr16, photometric="rgb", compression="zlib")
    print(f"sdr16.tif: {tif.stat().st_size} bytes, max error {check('tif', tifffile.imread(tif), sdr16, 0)}")

    uhdr = OUT / "hdr_ultrahdr.jpg"
    subprocess.run(["cargo", "run", "--release", "--quiet",
                    "--manifest-path", str(ROOT / "tools" / "uhdr_fixture" / "Cargo.toml"),
                    "--target-dir", str(ROOT / "src-tauri" / "target" / "uhdr_fixture"), "--", str(uhdr)],
                   check=True)
    files["sdr16.tif"] = (None, None, sdr16, 0, "16-bit sRGB TIFF, deflate", 16)
    write_readme(files, x, nits)
    total = sum(p.stat().st_size for p in OUT.iterdir())
    assert total < 300_000, total
    print(f"total {total} bytes")


def write_readme(files, x, nits):
    def sdr_row(name):
        exp = files[name][2]
        return " | ".join(f"{tuple(int(v) for v in exp[y, xx])}" for _, xx, y in SAMPLES)

    head = "| file | format | tolerance | " + " | ".join(s[0] for s in SAMPLES) + " |"
    sep = "|" + "---|" * (3 + len(SAMPLES))
    rows = [f"| `{n}` | {f[4]} | ±{f[3]} | {sdr_row(n)} |" for n, f in sorted(files.items())]
    nit_at = lambda xx, y: (PEAK * xx / (W - 1)) if y < 48 else (PATCH if xx < 96 else 0.0)
    lin = " | ".join(f"{nit_at(xx, y):.2f}" for _, xx, y in SAMPLES)
    sc = " | ".join(f"{nit_at(xx, y) / 80:.4f}" for _, xx, y in SAMPLES)
    uh = " | ".join(f"{nit_at(xx, y) / PATCH:.4f}" for _, xx, y in SAMPLES)
    text = f"""# Test fixtures

Tiny synthetic images written by `tools/gen_fixtures.py` (and `tools/uhdr_fixture` for the Ultra HDR
JPEG). Never put photos here. Regenerate with the commands at the top of `tools/gen_fixtures.py`.

## Pattern (all files 128x64)

- Rows 0..47: a grey ramp across x = 0..127.
  - SDR files: the encoded value is `round(x / 127 * max)`, where max is 255 or 65535 (sRGB-encoded, not linear).
  - HDR files: the light is `1000 * x / 127` nits (0 to 1000), encoded with the file's transfer function.
- Rows 48..63:
  - x 0..63: white. SDR full scale; HDR 203 nits (the BT.2408 reference white).
  - x 64..95: red, R channel only. The R value is the same as the white patch; G and B are 0.
  - x 96..127: black.

## Encoded values at sample points (R, G, B), as stored in the file

{head}
{sep}
{chr(10).join(rows)}

Tolerance is the largest difference in code values allowed after decoding (lossy JPEG and AVIF). The generator
checks each file against this table after writing it.

- PQ: `signal = PQ_inverse_EOTF(nits / 10000)`. 203 nits is signal 0.5806, 1000 nits is 0.7518.
- HLG: a 1000-nit display with system gamma 1.2, so `signal = HLG_OETF((nits / 1000) ^ (1 / 1.2))`.
  203 nits is signal 0.7498, 1000 nits is 1.0.
- AVIF codes are 10-bit (0..1023). JXL codes are 16-bit (0..65535).

## Expected linear light for the HDR files

This applies to `hdr_pq10_bt2020.avif`, `hdr_pq16_bt2020.jxl`, `hdr_hlg16_bt2020.jxl` and `hdr_ultrahdr.jpg`.
For grey pixels, BT.2020 and BT.709 give the same values. The red patch is the BT.2020 red primary in
the AVIF and JXL files, and the BT.709 red primary in the Ultra HDR file.

| quantity | {" | ".join(s[0] for s in SAMPLES)} |
|---|{"---|" * len(SAMPLES)}
| nits (R channel) | {lin} |
| scRGB (nits / 80) | {sc} |
| Ultra HDR linear (nits / 203) | {uh} |

## `hdr_ultrahdr.jpg`

- The Ultra HDR (ISO 21496-1 + XMP hdrgm) JPEG from ultrahdr-rs 0.3.5.
- Base image: the ultrahdr-core tone map of the HDR pattern, sRGB, with an sRGB ICC profile.
- Gain map: full resolution, single channel.
- Metadata: HDRCapacityMin 0, HDRCapacityMax log2(1000 / 203) = 2.30, GainMapMax about 2.28 (log2), Gamma 1, offsets 1/64.
- Decoded at full boost, it gives about 4.87 at x = 127, about 1% under the ideal 4.926. Allow 3%, or about 0.15
  absolute, because the base and gain map are JPEG-compressed.
"""
    (OUT / "README.md").write_text(text, encoding="utf-8")


if __name__ == "__main__":
    main()
