// monitors: the connected monitors (EnumDisplayMonitors + GetMonitorInfoW + GetDpiForMonitor), each one's
// device-interface path (EnumDisplayDevicesW, also IDesktopWallpaper's monitor ID) and its EDID from
// SetupAPI, matched on that path. Pattern: LrC-AVG hud/src-tauri/src/monitors.rs.
// Coordinates are physical pixels because Tauri (tao 0.37) makes the process Per-Monitor-DPI-aware v2.
use crate::model::{MonitorInfo, Rect};
use crate::{edid, hdr};
use std::mem::size_of;
use windows::core::{BOOL, PCWSTR};
use windows::Win32::Devices::DeviceAndDriverInstallation::*;
use windows::Win32::Devices::Display::GUID_DEVINTERFACE_MONITOR;
use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Registry::{RegCloseKey, RegQueryValueExW, KEY_READ};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    EDD_GET_DEVICE_INTERFACE_NAME, MONITORINFOF_PRIMARY,
};

fn wide(s: &[u16]) -> String {
    String::from_utf16_lossy(&s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())])
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

/// Every connected monitor, left to right then top to bottom.
pub fn list() -> Vec<MonitorInfo> {
    let edids = edids();
    let mut all: Vec<MonitorInfo> = handles()
        .into_iter()
        .filter_map(|h| monitor(h, &edids))
        .collect();
    all.sort_by(|a, b| a.px.x.total_cmp(&b.px.x).then(a.px.y.total_cmp(&b.px.y)));
    all
}

unsafe extern "system" fn enum_cb(h: HMONITOR, _: HDC, _: *mut RECT, lp: LPARAM) -> BOOL {
    (*(lp.0 as *mut Vec<HMONITOR>)).push(h);
    true.into()
}

fn handles() -> Vec<HMONITOR> {
    let mut all: Vec<HMONITOR> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(enum_cb),
            LPARAM(&mut all as *mut _ as isize),
        );
    }
    all
}

fn monitor(h: HMONITOR, edids: &[(String, Vec<u8>)]) -> Option<MonitorInfo> {
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
    let (mut dx, mut dy) = (96u32, 96u32);
    unsafe {
        if !GetMonitorInfoW(h, &mut info.monitorInfo).as_bool() {
            return None;
        }
        let _ = GetDpiForMonitor(h, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
    }
    let r = info.monitorInfo.rcMonitor;
    let gdi_name = wide(&info.szDevice);
    let (id, device_string) = device_interface(&gdi_name);
    let parsed = find_edid(&id, edids).and_then(edid::parse);
    let name = parsed
        .as_ref()
        .map(|e| e.name.clone())
        .or(device_string)
        .unwrap_or_else(|| gdi_name.trim_start_matches(r"\\.\").to_string());
    Some(MonitorInfo {
        hdr: hdr::get(&gdi_name),
        id,
        gdi_name,
        name,
        px: Rect {
            x: r.left as f64,
            y: r.top as f64,
            w: (r.right - r.left) as f64,
            h: (r.bottom - r.top) as f64,
        },
        scale: dx as f64 / 96.0,
        primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
        edid_mm: parsed.and_then(|e| e.mm),
    })
}

/// The monitor device-interface path on adapter output `gdi_name` (the active one when it is cloned),
/// plus Windows' own monitor description.
fn device_interface(gdi_name: &str) -> (String, Option<String>) {
    let adapter = to_wide(gdi_name);
    let mut first = None;
    for i in 0.. {
        let mut dd = DISPLAY_DEVICEW {
            cb: size_of::<DISPLAY_DEVICEW>() as u32,
            ..Default::default()
        };
        let ok = unsafe {
            EnumDisplayDevicesW(
                PCWSTR(adapter.as_ptr()),
                i,
                &mut dd,
                EDD_GET_DEVICE_INTERFACE_NAME,
            )
        };
        if !ok.as_bool() {
            break;
        }
        let found = (wide(&dd.DeviceID), Some(wide(&dd.DeviceString)));
        if dd.StateFlags & DISPLAY_DEVICE_ACTIVE == DISPLAY_DEVICE_ACTIVE {
            return found;
        }
        first.get_or_insert(found);
    }
    first.unwrap_or_default()
}

/// The EDID whose interface path is `id`, compared case-insensitively (SetupAPI lower-cases it).
fn find_edid<'a>(id: &str, edids: &'a [(String, Vec<u8>)]) -> Option<&'a [u8]> {
    edids
        .iter()
        .find(|(path, _)| !id.is_empty() && path.eq_ignore_ascii_case(id))
        .map(|(_, b)| b.as_slice())
}

/// (interface path, EDID bytes) for every present monitor, from HKLM\...\Device Parameters\EDID.
fn edids() -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    unsafe {
        let Ok(set) = SetupDiGetClassDevsW(
            Some(&GUID_DEVINTERFACE_MONITOR),
            PCWSTR::null(),
            None,
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        ) else {
            return out;
        };
        for i in 0.. {
            let mut iface = SP_DEVICE_INTERFACE_DATA {
                cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                ..Default::default()
            };
            if SetupDiEnumDeviceInterfaces(set, None, &GUID_DEVINTERFACE_MONITOR, i, &mut iface)
                .is_err()
            {
                break;
            }
            out.extend(interface_edid(set, &iface));
        }
        let _ = SetupDiDestroyDeviceInfoList(set);
    }
    out
}

unsafe fn interface_edid(
    set: HDEVINFO,
    iface: &SP_DEVICE_INTERFACE_DATA,
) -> Option<(String, Vec<u8>)> {
    let mut need = 0u32;
    let _ = SetupDiGetDeviceInterfaceDetailW(set, iface, None, 0, Some(&mut need), None);
    if need < 8 {
        return None;
    }
    // SP_DEVICE_INTERFACE_DETAIL_DATA_W: cbSize (8 on x64) then the path; u64 buffer keeps it aligned.
    let mut buf = vec![0u64; (need as usize).div_ceil(8)];
    let detail = buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
    (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
    let mut dev = SP_DEVINFO_DATA {
        cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
        ..Default::default()
    };
    SetupDiGetDeviceInterfaceDetailW(set, iface, Some(detail), need, None, Some(&mut dev)).ok()?;
    let path_len = (need as usize - 4) / 2; // offset of DevicePath is 4 bytes
    let path = std::slice::from_raw_parts((*detail).DevicePath.as_ptr(), path_len);
    let path = wide(path);
    let key = SetupDiOpenDevRegKey(set, &dev, DICS_FLAG_GLOBAL.0, 0, DIREG_DEV, KEY_READ.0).ok()?;
    let mut data = vec![0u8; 4096];
    let mut len = data.len() as u32;
    let name = to_wide("EDID");
    let r = RegQueryValueExW(
        key,
        PCWSTR(name.as_ptr()),
        None,
        None,
        Some(data.as_mut_ptr()),
        Some(&mut len),
    );
    let _ = RegCloseKey(key);
    if r.is_err() {
        return None;
    }
    data.truncate(len as usize);
    Some((path, data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edid_match_ignores_case_and_empty_ids() {
        let edids = vec![(
            r"\\?\display#bnq7950#5&9a89479&0&uid4352#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}"
                .to_string(),
            vec![1u8, 2, 3],
        )];
        let id = r"\\?\DISPLAY#BNQ7950#5&9a89479&0&UID4352#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}";
        assert_eq!(find_edid(id, &edids), Some(&[1u8, 2, 3][..]));
        assert_eq!(find_edid("", &edids), None);
        assert_eq!(find_edid(r"\\?\DISPLAY#SAM1015#x", &edids), None);
    }

    /// Run by hand: `cargo test real_monitors -- --ignored --nocapture`. Reads only.
    #[test]
    #[ignore = "reads the real desktop"]
    fn real_monitors() {
        use windows::Win32::UI::HiDpi::*;
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
        for m in list() {
            println!("{m:?} white {:?}", hdr::sdr_white_nits(&m.gdi_name));
        }
    }
}
