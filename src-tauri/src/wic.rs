// wic: Windows Imaging Component. Decodes AVIF (needs the Store's AV1 Video Extension and HEIF Image
// Extensions) and writes scRGB fp16 JPEG XR. Proven by spikes S2 and S3: WIC hands HDR AVIF back as
// 64bppRGBAHalf scRGB (linear BT.709, 1.0 = 80 nits), so never convert HDR through a u16 format (it clips).
use crate::source::Pixels;
use half::f16;
use windows::core::{GUID, HSTRING, PWSTR};
use windows::Win32::Foundation::{E_FAIL, GENERIC_READ, GENERIC_WRITE};
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::StructuredStorage::{IPropertyBag2, PROPBAG2};
use windows::Win32::System::Com::*;
use windows::Win32::System::Variant::VARIANT;

type Res<T> = windows::core::Result<T>;

fn factory() -> Res<IWICImagingFactory> {
    unsafe {
        // S_FALSE (already initialised) and RPC_E_CHANGED_MODE (an STA thread) both leave COM usable.
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)
    }
}

/// Native formats that carry values above 1.0.
fn is_float(f: &GUID) -> bool {
    [
        GUID_WICPixelFormat64bppRGBAHalf,
        GUID_WICPixelFormat64bppPRGBAHalf,
        GUID_WICPixelFormat64bppRGBHalf,
        GUID_WICPixelFormat48bppRGBHalf,
        GUID_WICPixelFormat128bppRGBAFloat,
        GUID_WICPixelFormat128bppPRGBAFloat,
        GUID_WICPixelFormat128bppRGBFloat,
        GUID_WICPixelFormat96bppRGBFloat,
    ]
    .contains(f)
}

fn first_frame(path: &str) -> Res<(IWICImagingFactory, IWICBitmapFrameDecode)> {
    let f = factory()?;
    unsafe {
        let dec = f.CreateDecoderFromFilename(
            &HSTRING::from(path),
            None,
            GENERIC_READ,
            WICDecodeMetadataCacheOnDemand,
        )?;
        let frame = dec.GetFrame(0)?;
        Ok((f, frame))
    }
}

/// (width, height, hdr) without decoding pixels.
pub fn probe(path: &str) -> Res<(u32, u32, bool)> {
    let (_, frame) = first_frame(path)?;
    let (mut w, mut h) = (0, 0);
    unsafe {
        frame.GetSize(&mut w, &mut h)?;
        Ok((w, h, is_float(&frame.GetPixelFormat()?)))
    }
}

/// Decodes the first frame: HDR files as scRGB f16 RGB, everything else as 8-bit sRGB RGB.
pub fn read(path: &str) -> Res<(u32, u32, Pixels, bool)> {
    let (f, frame) = first_frame(path)?;
    unsafe {
        let (mut w, mut h) = (0, 0);
        frame.GetSize(&mut w, &mut h)?;
        let hdr = is_float(&frame.GetPixelFormat()?);
        let target = if hdr {
            GUID_WICPixelFormat64bppRGBAHalf
        } else {
            GUID_WICPixelFormat32bppRGBA
        };
        let conv = f.CreateFormatConverter()?;
        conv.Initialize(
            &frame,
            &target,
            WICBitmapDitherTypeNone,
            None,
            0.0,
            WICBitmapPaletteTypeCustom,
        )?;
        // Copy in strips and drop alpha as we go, so the RGBA copy never exists in full.
        let bpp = if hdr { 8 } else { 4 };
        let rows = 256u32;
        let mut strip = vec![0u8; (w * bpp * rows) as usize];
        let n = w as usize * h as usize * 3;
        let (mut half, mut bytes) = if hdr {
            (Vec::with_capacity(n), Vec::new())
        } else {
            (Vec::new(), Vec::with_capacity(n))
        };
        for y in (0..h).step_by(rows as usize) {
            let rh = rows.min(h - y);
            let rect = WICRect {
                X: 0,
                Y: y as i32,
                Width: w as i32,
                Height: rh as i32,
            };
            let buf = &mut strip[..(w * bpp * rh) as usize];
            conv.CopyPixels(&rect, w * bpp, buf)?;
            for p in buf.chunks_exact(bpp as usize) {
                if hdr {
                    let c = |k: usize| f16::from_le_bytes([p[k], p[k + 1]]);
                    half.extend([c(0), c(2), c(4)]);
                } else {
                    bytes.extend_from_slice(&p[..3]);
                }
            }
        }
        let px = if hdr {
            Pixels::F16(half)
        } else {
            Pixels::U8(bytes)
        };
        Ok((w, h, px, hdr))
    }
}

/// Writes a lossless JPEG XR. `rgba` is scRGB (linear BT.709, 1.0 = 80 nits), 4 x f16 per pixel.
pub fn write_jxr(path: &str, w: u32, h: u32, rgba: &[f16]) -> Res<()> {
    unsafe {
        let f = factory()?;
        let stream = f.CreateStream()?;
        stream.InitializeFromFilename(&HSTRING::from(path), GENERIC_WRITE.0)?;
        let enc = f.CreateEncoder(&GUID_ContainerFormatWmp, std::ptr::null())?;
        enc.Initialize(&stream, WICBitmapEncoderNoCache)?;
        let mut frame: Option<IWICBitmapFrameEncode> = None;
        let mut bag: Option<IPropertyBag2> = None;
        enc.CreateNewFrame(&mut frame, &mut bag)?;
        let fail = || windows::core::Error::from_hresult(E_FAIL);
        let (Some(frame), Some(bag)) = (frame, bag) else {
            return Err(fail());
        };
        let mut name: Vec<u16> = "Lossless\0".encode_utf16().collect();
        let opt = PROPBAG2 {
            pstrName: PWSTR(name.as_mut_ptr()),
            ..Default::default()
        };
        bag.Write(1, &opt, &VARIANT::from(true))?;
        frame.Initialize(&bag)?;
        frame.SetSize(w, h)?;
        let mut fmt = GUID_WICPixelFormat64bppRGBAHalf;
        frame.SetPixelFormat(&mut fmt)?;
        if fmt != GUID_WICPixelFormat64bppRGBAHalf {
            return Err(fail()); // the WMP encoder changed the pixel format
        }
        let bytes: Vec<u8> = rgba.iter().flat_map(|v| v.to_le_bytes()).collect();
        frame.WritePixels(h, w * 8, &bytes)?;
        frame.Commit()?;
        enc.Commit()?;
        Ok(())
    }
}
