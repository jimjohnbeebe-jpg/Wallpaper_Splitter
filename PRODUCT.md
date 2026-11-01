# Product

## Users

The app is for photographers and wallpaper fans on Windows 11 whose monitors don't match: different sizes, resolutions or pixel densities, side by side or stacked. Jim, the author, has a 32" BenQ EW3270U and a 28" Samsung LU28R55, both 4K. His photos include Lightroom HDR exports (Ultra HDR gain-map JPEG, AVIF, JPEG XL).

## Purpose

You pick one image and the app splits it into one wallpaper per monitor, so the picture continues across the gap as if the monitors were one window. A line that crosses the bezel stays straight, and a horizon sits at the same height on every screen. The app previews the result in its own window and full screen on all monitors, exports named files, and sets each monitor's wallpaper. It remembers everything for next time.

It is not a background app. It has no slideshow, tray icon or hotkeys.

## Personality

Clear, calm, precise. It talks like a framer: short sentences in the user's words ("monitor", "bezel", "enlarged"), never the system's ("device path", "DPI", "EDID").

## Principles

1. **True to the desk.** What the light table shows is what the monitors show, measured in millimetres.
2. **One obvious action.** Set wallpaper is the primary action, and Export only is the secondary one.
3. **Say what happened.** Every action reports its result in place, for example "Wallpaper set on 2 monitors · Open folder". It never reports through a dialog.
4. **Keyboard first-class.** Every drag has a keyboard path (arrow keys, number fields).

## Accessibility

- Text is 12 px or larger (11 px only for captions and key hints).
- Text contrast is at least 4.5:1.
- The focus ring is visible.
- Colour never carries meaning alone.
- `prefers-reduced-motion` is respected.
