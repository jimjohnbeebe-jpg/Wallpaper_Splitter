# Wallpaper Splitter — announcement drafts

Every feature claim below comes from `../README.md` lines 3–14 and 29–36.
Links used: repo https://github.com/jimjohnbeebe-jpg/Wallpaper_Splitter ·
release https://github.com/jimjohnbeebe-jpg/Wallpaper_Splitter/releases/latest

Channel list is **unverified**: check each community still exists and allows
self-promotion before posting (read its sidebar rules). Post one or two a day,
not all at once, and answer comments the same day.

---

## 1. Reddit — multi-monitor / desktop communities
(candidates: r/MultipleMonitors, r/Windows11, r/desktops — unverified)

**Title:** I made a free Windows 11 app that splits one image across monitors of different sizes so it lines up across the bezel

**Body:**

My two monitors don't match (32" and 28", both 4K), and every span-wallpaper
tool I tried made the picture jump at the bezel because it works in pixels.
So I wrote one that works in millimetres.

- Reads each monitor's physical size (you can correct it by hand), plus bezel gap and per-monitor nudges
- Full-screen preview on every monitor with an alignment grid, so you can check a straight line really stays straight
- Exports PNG/JPG (or HDR JPEG XR from HDR sources) and sets each monitor's wallpaper
- Opens Ultra HDR JPEG, AVIF, JPEG XL, 16-bit PNG/TIFF as well as normal JPG/PNG/WebP

Free, MIT-licensed, one portable exe, no installer, no network requests.
https://github.com/jimjohnbeebe-jpg/Wallpaper_Splitter

Feedback welcome, especially from people with stacked or mixed-DPI setups.

[attach docs/screenshots/split.png]

---

## 2. Reddit — photography / HDR angle
(candidates: r/Lightroom, r/HDR — unverified)

**Title:** Free tool to use your Lightroom HDR exports (Ultra HDR / AVIF / JXL) as multi-monitor wallpaper on Windows 11

**Body:**

I wanted my HDR exports from Lightroom as wallpaper across two monitors.
Wallpaper Splitter opens Ultra HDR gain-map JPEGs, HDR AVIF and HDR JPEG XL,
lines the picture up across monitors of different sizes in millimetres, and
can write HDR JPEG XR wallpapers. It can also switch a monitor's HDR on and off.

Open source (MIT), portable exe: https://github.com/jimjohnbeebe-jpg/Wallpaper_Splitter

---

## 3. Hacker News — Show HN
Post on a weekday morning US time. Title must start with "Show HN:".

**Title:** Show HN: Wallpaper Splitter – span one image across mismatched monitors, in millimetres

**URL:** https://github.com/jimjohnbeebe-jpg/Wallpaper_Splitter

**First comment (post it yourself right after submitting):**

Most span-wallpaper tools cut the image in pixels, so on monitors with
different pixel densities the picture jumps at the bezel. This one reads each
monitor's physical size and lays the image out in millimetres, with bezel
gaps and nudges, then previews full screen with a grid so you can check
alignment. Built with Tauri 2 (Rust backend, vanilla TypeScript UI, no
framework). It also handles HDR sources (Ultra HDR JPEG, AVIF, JPEG XL).
Windows 11 only for now. Happy to answer questions about the geometry or the
HDR wallpaper path.

---

## 4. Reddit — developer angle
(candidates: r/rust, r/tauri — unverified; r/rust prefers technical write-ups)

**Title:** Wallpaper Splitter: a small Tauri 2 app (Rust + vanilla TS, no bundler) for mismatched multi-monitor wallpapers

**Body:** Short version of the HN comment above, plus one paragraph on
something you found interesting to build (for example the millimetre layout
in `src-tauri/src/geometry.rs`, or setting per-monitor wallpaper and HDR from Rust).

---

## 5. Listings (one-time, keep working after launch day)

| Where | What to do |
|---|---|
| winget | Your PR microsoft/winget-pkgs#468169 is waiting — see the status note in the chat reply |
| AlternativeTo | Add it as an alternative to DisplayFusion / other span-wallpaper tools |
| awesome-tauri (GitHub list) | Open a PR adding it under Applications |
| Microsoft Store | Not submitted yet — see chat reply |

---

## 6. Short post (X / Bluesky / Mastodon / LinkedIn)

My monitors don't match, so span wallpapers never lined up. I built Wallpaper
Splitter: free, open-source Windows 11 app that splits one image across
monitors of any size in millimetres, so lines stay straight across the bezel.
HDR sources supported. https://github.com/jimjohnbeebe-jpg/Wallpaper_Splitter
