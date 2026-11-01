# Design system

Wallpaper Splitter is a sibling of LrC-AVG (`hud/ui/style.css`, spec `docs/hud/lrc-avg-hud-spec-v2.md` §8). It uses the same graphite tokens, the same Inter type and the same square edges, with its own accent: **registration cyan**, the cyan of printers' crop marks.

**Subject.** A photographer lays one print across several pieces of glass. The design takes its words from print finishing: registration marks, the cutting mat and the light table. The light table is the one bold element. It draws each monitor at true physical scale, shows the real bezel gaps, and puts cyan registration ticks on the selected pane. Everything around it stays quiet.

**Rules.**
- No blur, glass, gradients or purple.
- One transparency only: `--mask`, which dims the photo outside the panes.
- Sentence case everywhere.
- No all-caps labels.
- No mid-dot meta strings.

## Tokens (`ui/style.css :root`)

Contrast ratios come from LrC-AVG's `docs/hud/tools/contrast.cjs` (WCAG relative luminance), run on 2026-11-01.

| Token | Value | Use | Contrast |
|---|---|---|---|
| `--bg` | #202122 | window, light-table surround | — |
| `--raised` | #2a2b2d | sidebar, inputs' groups | — |
| `--inset` | #161718 | light-table well, inputs, bezel gaps | — |
| `--line` | #3a3b3e | 1 px hairlines (never the only cue) | 1.44 on bg |
| `--text` | #e4e6e8 | values, titles | 12.89 on bg, 10.39 on hover |
| `--text2` | #aeb2b6 | labels | 7.56 on bg, 6.64 on raised, 6.10 on hover |
| `--text3` | #8d9196 | captions, key hints. **Never on raised (4.47)** | 5.09 on bg |
| `--accent` | #5ec4d8 | primary fill, selected pane, crop ticks, active nav | 7.96 on bg, 6.99 on raised, 8.86 on inset |
| `--accent-hover` | #7dd0e0 | primary hover | on-accent 9.05 |
| `--accent-press` | #49aabd | primary pressed | on-accent 5.87 |
| `--on-accent` | #06262d | text on accent | 7.83 |
| `--warn` | #eda447 | upscale warning (LrC-AVG amber, the family link) | 7.68 on bg, 6.75 on raised |
| `--danger` | #ee8064 | errors | 6.07 on bg |
| `--ok` | #86c296 | done (always with a check glyph) | 7.81 on bg |
| `--hover` / `--press` | #303134 / #252628 | secondary and nav fills | — |
| `--mask` | rgb(22 23 24 / .72) | dims the photo outside panes | — |
| `--grid-10` / `--grid-100` | #262729 / #303134 | millimetre grid on the Monitors page | — |

## Type

Inter variable, bundled in `ui/fonts` (OFL-1.1). The file is the same subset LrC-AVG ships, with no →, ✓ or ⚠ glyphs, so state glyphs are inline SVG.

| Size / line | Weight | Use |
|---|---|---|
| 11/14 | 500 | key hints and captions only |
| 12/16 | 400 | body, values (`.num` → `"tnum" 1` on digit runs only) |
| 13/16 | 500 | controls, navigation (buttons 600) |
| 15/20 | 600 | panel titles |
| 20/24 | 600 | the empty-state line only |

## Space, shape, depth

- **Rhythm:** a 4 px grid. `--s1` 4, `--s2` 8, `--s3` 12, `--s4` 16, `--s6` 24, `--s8` 32.
- **Control heights:** 32 normal, 28 for inputs and compact controls, 24 inline.
- **Fixed sizes:** sidebar 184, inspector 296, footer 28, nav row 32.
- **Radii:**
  - 0 on the window and panels.
  - 8 (`--r-card`) on cards and menus.
  - 6 (`--r-btn`) on buttons and inputs.
  - 4 (`--r-thumb`) on thumbnails, badges and keys.
  - Panes on the light table are square.
- **Depth:** set by tone (inset < bg < raised). There is one shadow, `--shadow-float`, used on menus and tooltips only.

## States

| State | Look |
|---|---|
| Hover | `--hover` fill. Primary uses `--accent-hover`. |
| Pressed | `--press`. Primary uses `--accent-press`. |
| Focus | `outline: 2px solid var(--text); outline-offset: 2px`. On the canvas, the focused pane gets accent and registration ticks. |
| Disabled | `--text3` text, `--line` border, no fill. A tooltip says why. |
| Selected (segments) | `--raised` fill, `--text`, 2 px accent underline. |

Colour never carries meaning alone: every warning has a glyph and words.

## Motion

- Hover and press take 120 ms ease-out.
- Pages and panels have no entrance animations.
- The only orchestrated moment is the full-screen preview fading in from black over 200 ms.
- `prefers-reduced-motion` turns every transition off.

## Overlays on photos

- **Crop frame:** 2 px accent with a 1 px outer edge of rgb(0 0 0 / .6), so it stays readable on bright skies.
- **Monitor labels:** always on a `--raised` chip, never as text directly on the photo.
- **Upscaled monitor:** an amber badge with a warning glyph and the scale ("Enlarged 1.4×").

## Shell

```
┌ native dark title bar ───────────────────────────────────────────────┐
│ nav 184 │ light table (inset well)                    │ inspector 296│
│ Split   │   panes at true mm scale, bezel gaps        │ Source       │
│ Monitors│                                             │ Framing      │
│ Prefs   │                                             │ Monitors     │
│         │                                             │ [Set wallpaper]
├─────────┴─────────────────────────────────────────────┴──────────────┤
│ footer 28: one cell per monitor with its HDR switch │ last action    │
└──────────────────────────────────────────────────────────────────────┘
```
