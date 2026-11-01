# Test fixtures

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

| file | format | tolerance | ramp x=0 | ramp x=32 | ramp x=64 | ramp x=96 | ramp x=127 | white patch | red patch | black |
|---|---|---|---|---|---|---|---|---|---|---|
| `hdr_hlg16_bt2020.jxl` | 16-bit JPEG XL, lossless, BT.2020 primaries, HLG | ±0 | (0, 0, 0) | (51442, 51442, 51442) | (58621, 58621, 58621) | (62727, 62727, 62727) | (65535, 65535, 65535) | (49143, 49143, 49143) | (49143, 0, 0) | (0, 0, 0) |
| `hdr_pq10_bt2020.avif` | 10-bit AVIF, BT.2020 primaries, PQ, BT.2020 NCL matrix, 4:4:4, quality 95 | ±8 | (0, 0, 0) | (617, 617, 617) | (693, 693, 693) | (738, 738, 738) | (769, 769, 769) | (594, 594, 594) | (594, 0, 0) | (0, 0, 0) |
| `hdr_pq16_bt2020.jxl` | 16-bit JPEG XL, lossless, BT.2020 primaries, PQ | ±0 | (0, 0, 0) | (39543, 39543, 39543) | (44395, 44395, 44395) | (47274, 47274, 47274) | (49271, 49271, 49271) | (38055, 38055, 38055) | (38055, 0, 0) | (0, 0, 0) |
| `sdr16.png` | 16-bit sRGB PNG | ±0 | (0, 0, 0) | (16513, 16513, 16513) | (33026, 33026, 33026) | (49538, 49538, 49538) | (65535, 65535, 65535) | (65535, 65535, 65535) | (65535, 0, 0) | (0, 0, 0) |
| `sdr16.tif` | 16-bit sRGB TIFF, deflate | ±0 | (0, 0, 0) | (16513, 16513, 16513) | (33026, 33026, 33026) | (49538, 49538, 49538) | (65535, 65535, 65535) | (65535, 65535, 65535) | (65535, 0, 0) | (0, 0, 0) |
| `sdr8.jpg` | 8-bit sRGB JPEG, quality 95, 4:4:4 | ±12 | (0, 0, 0) | (64, 64, 64) | (129, 129, 129) | (193, 193, 193) | (255, 255, 255) | (255, 255, 255) | (255, 0, 0) | (0, 0, 0) |
| `sdr8.png` | 8-bit sRGB PNG | ±0 | (0, 0, 0) | (64, 64, 64) | (129, 129, 129) | (193, 193, 193) | (255, 255, 255) | (255, 255, 255) | (255, 0, 0) | (0, 0, 0) |
| `sdr8.webp` | 8-bit sRGB WebP, lossless | ±0 | (0, 0, 0) | (64, 64, 64) | (129, 129, 129) | (193, 193, 193) | (255, 255, 255) | (255, 255, 255) | (255, 0, 0) | (0, 0, 0) |

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

| quantity | ramp x=0 | ramp x=32 | ramp x=64 | ramp x=96 | ramp x=127 | white patch | red patch | black |
|---|---|---|---|---|---|---|---|---|
| nits (R channel) | 0.00 | 251.97 | 503.94 | 755.91 | 1000.00 | 203.00 | 203.00 | 0.00 |
| scRGB (nits / 80) | 0.0000 | 3.1496 | 6.2992 | 9.4488 | 12.5000 | 2.5375 | 2.5375 | 0.0000 |
| Ultra HDR linear (nits / 203) | 0.0000 | 1.2412 | 2.4824 | 3.7237 | 4.9261 | 1.0000 | 1.0000 | 0.0000 |

## `hdr_ultrahdr.jpg`

- The Ultra HDR (ISO 21496-1 + XMP hdrgm) JPEG from ultrahdr-rs 0.3.5.
- Base image: the ultrahdr-core tone map of the HDR pattern, sRGB, with an sRGB ICC profile.
- Gain map: full resolution, single channel.
- Metadata: HDRCapacityMin 0, HDRCapacityMax log2(1000 / 203) = 2.30, GainMapMax about 2.28 (log2), Gamma 1, offsets 1/64.
- Decoded at full boost, it gives about 4.87 at x = 127, about 1% under the ideal 4.926. Allow 3%, or about 0.15
  absolute, because the base and gain map are JPEG-compressed.
