// hdr: per-monitor HDR state and SDR white level through DisplayConfig (ported from spike S4, proven on
// Windows 11 26H2). No admin rights needed. Callers that switch HDR read the old state first and put it
// back (preview.rs does).
#![allow(dead_code)] // called from commands.rs once the pass-1 branches are merged
use crate::model::HdrState;
use std::mem::size_of;
use windows::Win32::Devices::Display::*;
use windows::Win32::Foundation::{ERROR_SUCCESS, LUID};

// Not in windows 0.62.2 (Windows 11 24H2 SDK additions), declared by hand from wingdi.h.
const GET_ADVANCED_COLOR_INFO_2: i32 = 15;
const SET_HDR_STATE: i32 = 16;

/// DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO_2, 36 bytes.
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct AdvancedColorInfo2 {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    /// bit4 highDynamicRangeSupported, bit5 highDynamicRangeUserEnabled (others: see wingdi.h)
    value: u32,
    color_encoding: i32,
    bits_per_color_channel: u32,
    active_color_mode: i32,
}

/// DISPLAYCONFIG_SET_HDR_STATE, 24 bytes. bit0 enableHdr.
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct SetHdrState {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    value: u32,
}

fn header(kind: i32, size: usize, adapter: LUID, id: u32) -> DISPLAYCONFIG_DEVICE_INFO_HEADER {
    DISPLAYCONFIG_DEVICE_INFO_HEADER {
        r#type: DISPLAYCONFIG_DEVICE_INFO_TYPE(kind),
        size: size as u32,
        adapterId: adapter,
        id,
    }
}

fn wide(s: &[u16]) -> String {
    String::from_utf16_lossy(&s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())])
}

fn active_paths() -> Vec<DISPLAYCONFIG_PATH_INFO> {
    unsafe {
        loop {
            let (mut np, mut nm) = (0u32, 0u32);
            if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut np, &mut nm) != ERROR_SUCCESS
            {
                return vec![];
            }
            let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); np as usize];
            let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); nm as usize];
            let r = QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut np,
                paths.as_mut_ptr(),
                &mut nm,
                modes.as_mut_ptr(),
                None,
            );
            if r == ERROR_SUCCESS {
                paths.truncate(np as usize);
                return paths;
            }
            // ERROR_INSUFFICIENT_BUFFER: the topology changed between the two calls, so retry.
        }
    }
}

/// `\\.\DISPLAYn` for the path's source (matches MONITORINFOEXW.szDevice). Type 1.
fn gdi_name_of(p: &DISPLAYCONFIG_PATH_INFO) -> String {
    let mut n = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
        header: header(
            1,
            size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>(),
            p.sourceInfo.adapterId,
            p.sourceInfo.id,
        ),
        ..Default::default()
    };
    unsafe { DisplayConfigGetDeviceInfo(&mut n.header) };
    wide(&n.viewGdiDeviceName)
}

fn path_for(gdi_name: &str) -> Option<DISPLAYCONFIG_PATH_INFO> {
    active_paths()
        .into_iter()
        .find(|p| gdi_name_of(p).eq_ignore_ascii_case(gdi_name))
}

fn read(p: &DISPLAYCONFIG_PATH_INFO) -> HdrState {
    let (a, id) = (p.targetInfo.adapterId, p.targetInfo.id);
    let mut i2 = AdvancedColorInfo2 {
        header: header(
            GET_ADVANCED_COLOR_INFO_2,
            size_of::<AdvancedColorInfo2>(),
            a,
            id,
        ),
        ..Default::default()
    };
    if unsafe { DisplayConfigGetDeviceInfo(&mut i2.header) } == 0 {
        return bits_info2(i2.value);
    }
    // Pre-24H2 fallback: bit0 advancedColorSupported, bit1 advancedColorEnabled.
    let mut i1 = DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO {
        header: header(9, size_of::<DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO>(), a, id),
        ..Default::default()
    };
    if unsafe { DisplayConfigGetDeviceInfo(&mut i1.header) } == 0 {
        return bits_info1(unsafe { i1.Anonymous.value });
    }
    HdrState::default()
}

fn bits_info2(v: u32) -> HdrState {
    HdrState {
        supported: v & (1 << 4) != 0,
        enabled: v & (1 << 5) != 0,
    }
}

fn bits_info1(v: u32) -> HdrState {
    HdrState {
        supported: v & 1 != 0,
        enabled: v & 2 != 0,
    }
}

/// HDR state of the monitor showing `\\.\DISPLAYn`; unsupported/off when it is not found.
pub fn get(gdi_name: &str) -> HdrState {
    path_for(gdi_name).map(|p| read(&p)).unwrap_or_default()
}

/// Switches HDR (SET_HDR_STATE, falling back to SET_ADVANCED_COLOR_STATE) and returns the state read back
/// once Windows has applied it (up to about 3 s; the mode change is asynchronous).
pub fn set(gdi_name: &str, on: bool) -> Result<HdrState, String> {
    let p = path_for(gdi_name).ok_or_else(|| format!("{gdi_name} is not connected"))?;
    let before = read(&p);
    if !before.supported {
        return Err(format!("{gdi_name} does not support HDR"));
    }
    if before.enabled == on {
        return Ok(before);
    }
    send(&p, on).map_err(|e| format!("Windows refused to switch HDR on {gdi_name} (error {e})"))?;
    let mut now = before;
    for _ in 0..30 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        now = read(&p);
        if now.enabled == on {
            break;
        }
    }
    Ok(now)
}

fn send(p: &DISPLAYCONFIG_PATH_INFO, on: bool) -> Result<(), i32> {
    let (a, id) = (p.targetInfo.adapterId, p.targetInfo.id);
    let s2 = SetHdrState {
        header: header(SET_HDR_STATE, size_of::<SetHdrState>(), a, id),
        value: on as u32,
    };
    if unsafe { DisplayConfigSetDeviceInfo(&s2.header) } == 0 {
        return Ok(());
    }
    let mut s1 = DISPLAYCONFIG_SET_ADVANCED_COLOR_STATE {
        header: header(
            10,
            size_of::<DISPLAYCONFIG_SET_ADVANCED_COLOR_STATE>(),
            a,
            id,
        ),
        ..Default::default()
    };
    s1.Anonymous.value = on as u32; // bit0 enableAdvancedColor
    match unsafe { DisplayConfigSetDeviceInfo(&s1.header) } {
        0 => Ok(()),
        e => Err(e),
    }
}

/// Windows' SDR content brightness in nits (type 11). It reads 80 while HDR is off, so callers treat 80
/// as "HDR off / default" and prefer a value read with HDR on.
pub fn sdr_white_nits(gdi_name: &str) -> Option<f64> {
    let p = path_for(gdi_name)?;
    let mut w = DISPLAYCONFIG_SDR_WHITE_LEVEL {
        header: header(
            11,
            size_of::<DISPLAYCONFIG_SDR_WHITE_LEVEL>(),
            p.targetInfo.adapterId,
            p.targetInfo.id,
        ),
        ..Default::default()
    };
    (unsafe { DisplayConfigGetDeviceInfo(&mut w.header) } == 0)
        .then(|| w.SDRWhiteLevel as f64 / 1000.0 * 80.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_sizes_match_wingdi() {
        assert_eq!(size_of::<AdvancedColorInfo2>(), 36);
        assert_eq!(size_of::<SetHdrState>(), 24);
    }

    #[test]
    fn decodes_bitfields_seen_on_jims_monitors() {
        // 81 = supported, HDR-capable, WCG-capable, HDR off; 115 = the same with HDR on (spike S4).
        assert_eq!(
            bits_info2(81),
            HdrState {
                supported: true,
                enabled: false
            }
        );
        assert_eq!(
            bits_info2(115),
            HdrState {
                supported: true,
                enabled: true
            }
        );
        assert_eq!(
            bits_info1(0b11),
            HdrState {
                supported: true,
                enabled: true
            }
        );
    }

    #[test]
    fn unknown_monitor_reads_as_unsupported() {
        assert_eq!(get(r"\\.\DISPLAY_NOPE"), HdrState::default());
        assert!(set(r"\\.\DISPLAY_NOPE", true).is_err());
    }
}
