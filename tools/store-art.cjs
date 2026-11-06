// Renders the app icons and Microsoft Store listing art to PNG with Playwright (a dev dependency).
// Setup, once: npm ci; npx playwright install chromium; pip install -r tools/requirements.txt (for Pillow).
// Run: node tools/store-art.cjs <out-dir>; python tools/store-art-ico.py <out-dir>. Then copy <out-dir>/icons to
// src-tauri/icons (without ico/), <out-dir>/msix to packaging/msix/Assets and <out-dir>/store to docs/store.
// Colours are the DESIGN.md tokens; the landscape is flat bands (no gradients), one scene seen through several panes.
const { chromium } = require("playwright");
const path = require("path");
const fs = require("fs");

const OUT = process.argv[2];
const T = { bg: "#202122", raised: "#2a2b2d", inset: "#161718", line: "#3a3b3e", text: "#e4e6e8", text2: "#aeb2b6", accent: "#5ec4d8", onAccent: "#06262d" };
const FONT = fs.readFileSync(path.join(__dirname, "../ui/fonts/inter.woff2")).toString("base64");

// The scene lives in a 1600 x 900 world; a pane is a clipped window onto it, so every line continues across the gaps.
function scene() {
  const sky = ["#2b2f4a", "#463c5e", "#7a4a5e", "#c0614a", "#ec8a3c", "#f6b24a"];
  const bands = sky.map((c, i) => `<rect x="0" y="${i * 70}" width="1600" height="${i === sky.length - 1 ? 520 : 70}" fill="${c}"/>`).join("");
  return `${bands}
    <circle cx="1010" cy="470" r="46" fill="#ffd27a"/>
    <path d="M0 520 L180 470 L330 500 L520 440 L700 480 L860 455 L1000 490 L1160 300 L1250 285 L1330 330 L1600 420 L1600 900 L0 900Z" fill="#5c5a7c"/>
    <path d="M0 590 L240 545 L420 575 L640 530 L900 580 L1120 540 L1360 585 L1600 550 L1600 900 L0 900Z" fill="#3a3550"/>
    <path d="M0 680 L300 640 L560 670 L820 630 L1100 675 L1400 640 L1600 660 L1600 900 L0 900Z" fill="#221f2e"/>
    <path d="M430 640 l14 -70 l14 70z M1240 625 l12 -58 l12 58z M820 630 l10 -46 l10 46z" fill="#16141e"/>`;
}

// Monitors: [x, y, w, h] in world units. Bezels are drawn as inset frames; the scene shows through each pane.
function monitors(list, id, bezel = 14) {
  const defs = list.map(([x, y, w, h], i) => `<clipPath id="${id}${i}"><rect x="${x}" y="${y}" width="${w}" height="${h}"/></clipPath>`).join("");
  const panes = list.map(([x, y, w, h], i) =>
    `<rect x="${x - bezel}" y="${y - bezel}" width="${w + 2 * bezel}" height="${h + 2 * bezel}" rx="${bezel * 0.8}" fill="${T.inset}" stroke="${T.line}" stroke-width="3"/>
     <g clip-path="url(#${id}${i})">${scene()}</g>`).join("");
  return `<defs>${defs}</defs>${panes}`;
}

function cross(x, y, s, sw, c) {
  return `<path d="M${x - s} ${y}H${x + s}M${x} ${y - s}V${y + s}" stroke="${c}" stroke-width="${sw}" stroke-linecap="square"/>`;
}

// The icon: two accent panes of different sizes; one dark ridge line runs across both, through the gap.
function icon(size, { small = false, plate = true } = {}) {
  const ridge = "M40 300 L150 228 L222 270 L304 196 L380 248 L470 222";
  const panes = small ? [[56, 150, 176, 168], [262, 112, 194, 268]] : [[76, 168, 160, 136], [262, 128, 180, 236]];
  const clip = panes.map(([x, y, w, h], i) => `<clipPath id="ic${i}"><rect x="${x}" y="${y}" width="${w}" height="${h}" rx="${small ? 10 : 6}"/></clipPath>`).join("");
  const body = panes.map(([x, y, w, h], i) => `<g clip-path="url(#ic${i})">
      <rect x="${x}" y="${y}" width="${w}" height="${h}" fill="${T.accent}"/>
      <path d="${ridge} L470 512 L40 512Z" fill="${T.onAccent}"/>
    </g>`).join("");
  const crosses = small ? "" : [[58, 112], [454, 112], [58, 400], [454, 400]].map(([x, y]) => cross(x, y, 18, 8, T.text)).join("");
  const bg = plate ? `<rect x="12" y="12" width="488" height="488" rx="${small ? 80 : 96}" fill="${T.bg}" stroke="${T.line}" stroke-width="${small ? 0 : 8}"/>` : "";
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 512 512"><defs>${clip}</defs>${bg}${body}${crosses}</svg>`;
}

function page(w, h, inner) {
  return `<!doctype html><html><head><style>
    @font-face { font-family: Inter; src: url(data:font/woff2;base64,${FONT}) format("woff2"); font-weight: 100 900; }
    html, body { margin: 0; width: ${w}px; height: ${h}px; overflow: hidden; background: transparent; }
    svg { display: block; } text { font-family: Inter; }
  </style></head><body>${inner}</body></html>`;
}

// Store art. Poster (9:16) and box (1:1) carry the name; the hero (16:9) is picture only, as the Store lays its own text over it.
function poster(w, h) {
  const k = w / 1440;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 1440 2160">
    <rect width="1440" height="2160" fill="${T.bg}"/>
    <g transform="translate(120 150) scale(0.42)">${icon(512).replace(/<\/?svg[^>]*>/g, "")}</g>
    <text x="120" y="500" font-size="112" font-weight="700" fill="${T.text}">Wallpaper</text>
    <text x="120" y="630" font-size="112" font-weight="700" fill="${T.text}">Splitter</text>
    <text x="120" y="740" font-size="48" font-weight="500" fill="${T.text2}">One picture across every monitor,</text>
    <text x="120" y="806" font-size="48" font-weight="500" fill="${T.text2}">lined up at the bezel.</text>
    <g transform="translate(17 741) scale(0.82)">${monitors([[140, 300, 470, 270], [660, 210, 560, 330], [1270, 250, 300, 520]], "p", 16)}</g>
    <rect x="120" y="1560" width="120" height="8" fill="${T.accent}"/>
    <text x="120" y="1660" font-size="44" font-weight="500" fill="${T.text}">Any size, resolution and pixel density.</text>
    <text x="120" y="1730" font-size="44" font-weight="500" fill="${T.text}">HDR images welcome.</text>
    <text x="120" y="1800" font-size="44" font-weight="500" fill="${T.text}">Free and open source for Windows 11.</text>
    <text x="120" y="2010" font-size="40" font-weight="600" fill="${T.accent}">Maybe LLC</text>
    <text x="120" y="2066" font-size="36" font-weight="500" fill="${T.text2}">maybellc.com</text>
  </svg>`.replace(/width="1440" height="2160" viewBox/, `width="${w}" height="${h}" data-k="${k}" viewBox`);
}

function box(s) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${s}" height="${s}" viewBox="0 0 2160 2160">
    <rect width="2160" height="2160" fill="${T.bg}"/>
    <g transform="translate(1 216) scale(1.26)">${monitors([[140, 300, 470, 270], [660, 210, 560, 330], [1270, 250, 300, 520]], "b", 16)}</g>
    <text x="1080" y="1560" text-anchor="middle" font-size="200" font-weight="700" fill="${T.text}">Wallpaper Splitter</text>
    <text x="1080" y="1710" text-anchor="middle" font-size="84" font-weight="500" fill="${T.text2}">One picture, lined up across every monitor</text>
  </svg>`;
}

function hero(w, h) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 3840 2160">
    <rect width="3840" height="2160" fill="${T.bg}"/>
    <g transform="translate(79 26) scale(2.15)">${monitors([[140, 300, 470, 270], [660, 210, 560, 330], [1270, 250, 300, 520]], "h", 16)}</g>
  </svg>`;
}

const JOBS = [
  // App icons: Tauri (src-tauri/icons) and MSIX (packaging/msix/Assets).
  ["icons/icon.png", 512, 512, () => icon(512)],
  ["icons/128x128@2x.png", 256, 256, () => icon(256)],
  ["icons/128x128.png", 128, 128, () => icon(128)],
  ["icons/32x32.png", 32, 32, () => icon(32, { small: true })],
  ...[16, 24, 32, 48, 64, 256].map((s) => [`icons/ico/${s}.png`, s, s, () => icon(s, { small: s <= 48 })]),
  ["msix/Square44x44Logo.png", 44, 44, () => icon(44, { small: true })],
  ["msix/Square150x150Logo.png", 150, 150, () => icon(150)],
  ["msix/StoreLogo.png", 50, 50, () => icon(50, { small: true })],
  ["msix/Wide310x150Logo.png", 310, 150, () => `<svg xmlns="http://www.w3.org/2000/svg" width="310" height="150" viewBox="0 0 310 150"><rect width="310" height="150" fill="${T.bg}"/><g transform="translate(16 15) scale(0.235)">${icon(512).replace(/<\/?svg[^>]*>/g, "")}</g><text x="150" y="70" font-size="24" font-weight="700" fill="${T.text}">Wallpaper</text><text x="150" y="98" font-size="24" font-weight="700" fill="${T.text}">Splitter</text></svg>`],
  // Store listing art.
  ["store/poster-1440x2160.png", 1440, 2160, () => poster(1440, 2160)],
  ["store/poster-720x1080.png", 720, 1080, () => poster(720, 1080)],
  ["store/box-2160x2160.png", 2160, 2160, () => box(2160)],
  ["store/box-1080x1080.png", 1080, 1080, () => box(1080)],
  ["store/hero-3840x2160.png", 3840, 2160, () => hero(3840, 2160)],
  ["store/hero-1920x1080.png", 1920, 1080, () => hero(1920, 1080)],
  ["store/logo-300x300.png", 300, 300, () => icon(300)],
];

(async () => {
  const browser = await chromium.launch();
  const p = await browser.newPage({ deviceScaleFactor: 1 });
  for (const [file, w, h, make] of JOBS) {
    const dest = path.join(OUT, file);
    fs.mkdirSync(path.dirname(dest), { recursive: true });
    await p.setViewportSize({ width: w, height: h });
    await p.setContent(page(w, h, make()));
    await p.evaluate(() => document.fonts.ready);
    await p.screenshot({ path: dest, omitBackground: true, clip: { x: 0, y: 0, width: w, height: h } });
  }
  await browser.close();
  console.log(`wrote ${JOBS.length} images to ${OUT}`);
})();
