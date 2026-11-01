# Wallpaper Splitter

Split one image into a wallpaper for each monitor, so the picture continues across the gap as if your monitors were one window. This works even when they are different sizes, resolutions and pixel densities.

- Monitors are measured in millimetres. Physical size is read from each monitor, and you can correct it by hand.
- Bezel gaps and per-monitor nudges are supported.
- Preview in the app and full screen on every monitor, with an alignment grid.
- Exports named files (PNG, JPG, or HDR JPEG XR from HDR sources) and sets each monitor's wallpaper.
- Opens Lightroom HDR exports (Ultra HDR JPEG, AVIF, JPEG XL), 16-bit PNG and TIFF, and everyday JPG, PNG and WebP.
- Can switch a monitor's HDR on and off. It remembers your last image, framing and settings.

Windows 11 only. Runs as a portable exe with no installer.

> Status: in development (v0.1). Download, screenshots and usage come with v1.0.

## Build from source

Needs Node 24 and Rust 1.99.

```powershell
npm ci
npm run build   # → src-tauri\target\release\WallpaperSplitter.exe
```

## License

MIT. The Inter font is under the SIL Open Font License 1.1 (`ui/fonts/LICENSE`).
