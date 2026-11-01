// edid: the monitor name and physical size from a raw EDID base block (VESA E-EDID 1.4). Pure.
#![allow(dead_code)] // called from commands.rs once the pass-1 branches are merged
use crate::model::Size;

const HEADER: [u8; 8] = [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];

#[derive(Debug, Clone, PartialEq)]
pub struct Edid {
    /// The 0xFC monitor-name descriptor, or the PnP code ("BNQ7950") when there is none.
    pub name: String,
    pub mm: Option<Size>,
}

pub fn parse(b: &[u8]) -> Option<Edid> {
    if b.len() < 128 || b[..8] != HEADER {
        return None;
    }
    let descriptors = || b[54..126].as_chunks::<18>().0.iter();
    let name = descriptors()
        .find(|d| d[..3] == [0, 0, 0] && d[3] == 0xFC)
        .map(|d| text(&d[5..]))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| pnp_code(b));
    // The first detailed timing descriptor (pixel clock non-zero) carries the size in mm; bytes 21/22 in cm.
    let dtd = descriptors()
        .find(|d| d[0] != 0 || d[1] != 0)
        .map(|d| {
            let w = d[12] as u32 | ((d[14] as u32 >> 4) << 8);
            let h = d[13] as u32 | ((d[14] as u32 & 0x0F) << 8);
            (w as f64, h as f64)
        })
        .filter(|&(w, h)| plausible(w, h));
    let cm = Some((b[21] as f64 * 10.0, b[22] as f64 * 10.0)).filter(|&(w, h)| plausible(w, h));
    let mm = dtd.or(cm).map(|(w, h)| Size { w, h });
    Some(Edid { name, mm })
}

/// Zero means "unknown or a projector"; some panels write an aspect ratio (16x9) or cm in the mm field.
fn plausible(w: f64, h: f64) -> bool {
    (50.0..=5000.0).contains(&w) && (50.0..=5000.0).contains(&h)
}

fn text(raw: &[u8]) -> String {
    let end = raw.iter().position(|&c| c == 0x0A).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).trim().to_string()
}

/// Manufacturer (three 5-bit letters, big-endian) plus product code (little-endian), e.g. "BNQ7950".
fn pnp_code(b: &[u8]) -> String {
    let m = u16::from_be_bytes([b[8], b[9]]);
    let letter = |shift: u16| (b'A' - 1 + ((m >> shift) & 0x1F) as u8) as char;
    let product = u16::from_le_bytes([b[10], b[11]]);
    format!("{}{}{}{product:04X}", letter(10), letter(5), letter(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A BenQ EW3270U-like base block: 3840x2160 DTD at 698x393 mm, 70x39 cm, name descriptor, no serial.
    const BENQ: [u8; 128] = [
        0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x09, 0xD1, 0x50, 0x79, 0x00, 0x00, 0x00,
        0x00, 0x0C, 0x21, 0x01, 0x04, 0xB5, 0x46, 0x27, 0x78, 0x3A, 0x7F, 0x6E, 0xB4, 0x50, 0x29,
        0x45, 0xB4, 0x25, 0x0E, 0x50, 0x21, 0x08, 0x00, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
        0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x08, 0xE8, 0x00, 0x30, 0xF2, 0x70,
        0x5A, 0x80, 0xB0, 0x58, 0x8A, 0x00, 0xBA, 0x89, 0x21, 0x00, 0x00, 0x1E, 0x00, 0x00, 0x00,
        0xFD, 0x00, 0x28, 0x3C, 0x87, 0x87, 0x3C, 0x01, 0x0A, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20,
        0x00, 0x00, 0x00, 0xFC, 0x00, 0x42, 0x65, 0x6E, 0x51, 0x20, 0x45, 0x57, 0x33, 0x32, 0x37,
        0x30, 0x55, 0x0A, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x4E,
    ];

    #[test]
    fn checksum_is_valid() {
        assert_eq!(BENQ.iter().fold(0u8, |a, &b| a.wrapping_add(b)), 0);
    }

    #[test]
    fn reads_name_and_dtd_size() {
        let e = parse(&BENQ).unwrap();
        assert_eq!(e.name, "BenQ EW3270U");
        assert_eq!(e.mm, Some(Size { w: 698.0, h: 393.0 }));
    }

    #[test]
    fn falls_back_to_pnp_code_and_cm() {
        let mut b = BENQ;
        b[93] = 0x10; // name descriptor -> dummy
        b[66..69].copy_from_slice(&[0, 0, 0]); // DTD size zero
        let e = parse(&b).unwrap();
        assert_eq!(e.name, "BNQ7950");
        assert_eq!(e.mm, Some(Size { w: 700.0, h: 390.0 }));
    }

    #[test]
    fn implausible_or_missing_size_is_none() {
        let mut b = BENQ;
        b[66..69].copy_from_slice(&[16, 9, 0]); // aspect ratio written as mm
        b[21] = 0;
        b[22] = 0;
        assert_eq!(parse(&b).unwrap().mm, None);
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse(&[0u8; 128]), None);
        assert_eq!(parse(&BENQ[..100]), None);
    }
}
