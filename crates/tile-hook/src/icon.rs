//! App icons from the shell (`IShellItemImageFactory`, which winsafe doesn't wrap), drawn with
//! their alpha channel, and the monitor DPI to size them by.

use std::{ffi::c_void, mem, ptr};

use windows_sys::{
    Win32::Foundation::{POINT, SIZE},
    Win32::Graphics::Gdi::{
        AC_SRC_ALPHA, AC_SRC_OVER, AlphaBlend, BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
        BLENDFUNCTION, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC,
        DeleteObject, GetDC, GetDIBits, GetObjectW, HBITMAP, HDC, MONITOR_DEFAULTTONEAREST,
        MonitorFromPoint, ReleaseDC, SelectObject,
    },
    Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
    Win32::UI::Shell::{SHCreateItemFromParsingName, SIIGBF, SIIGBF_ICONONLY},
    core::{GUID, HRESULT},
};

const IID_ISHELLITEMIMAGEFACTORY: GUID = GUID::from_u128(0xbcc18b79_ba16_442f_80c4_8a59c30c463b);

#[repr(C)]
struct ImageFactoryVtbl {
    _query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    _add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_image: unsafe extern "system" fn(*mut c_void, SIZE, SIIGBF, *mut HBITMAP) -> HRESULT,
}

/// An app icon: a premultiplied 32-bit bitmap, freed on drop.
#[derive(Debug)]
pub struct Icon {
    bitmap: HBITMAP,
    width: i32,
    height: i32,
}

impl Drop for Icon {
    fn drop(&mut self) {
        // SAFETY: we own the bitmap, it is never selected into a DC outside `draw`, and it is
        // deleted once.
        unsafe { DeleteObject(self.bitmap) };
    }
}

fn bitmap_info(width: i32, height: i32) -> BITMAPINFO {
    BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            // Negative: rows top to bottom.
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// The shell's bitmaps come with straight, premultiplied or no alpha; make it premultiplied,
/// as `AlphaBlend` needs.
fn premultiply(pixels: &mut [u8]) {
    let (pixels, _) = pixels.as_chunks_mut::<4>();
    if pixels.iter().all(|p| p[3] == 0) {
        pixels.iter_mut().for_each(|p| p[3] = 255);
        return;
    }
    if pixels
        .iter()
        .any(|p| p[0] > p[3] || p[1] > p[3] || p[2] > p[3])
    {
        for p in pixels {
            let a = u16::from(p[3]);
            for c in &mut p[..3] {
                *c = ((u16::from(*c) * a + 127) / 255) as u8;
            }
        }
    }
}

/// Reads a bitmap's pixels as top-down BGRA.
fn read_pixels(bitmap: HBITMAP) -> Option<(Vec<u8>, i32, i32)> {
    let mut bm = BITMAP::default();
    // SAFETY: `bm` is a BITMAP of the size we pass, for a bitmap handle we own.
    let got = unsafe {
        GetObjectW(
            bitmap,
            mem::size_of::<BITMAP>() as i32,
            (&raw mut bm).cast(),
        )
    };
    let (width, height) = (bm.bmWidth, bm.bmHeight.abs());
    if got == 0 || width <= 0 || height <= 0 {
        return None;
    }
    let mut pixels = vec![0u8; width as usize * height as usize * 4];
    let mut info = bitmap_info(width, height);
    // SAFETY: the screen DC is released below; `pixels` holds `height` rows of `width` 32-bit
    // pixels, the format `info` asks for.
    let lines = unsafe {
        let screen = GetDC(ptr::null_mut());
        let lines = GetDIBits(
            screen,
            bitmap,
            0,
            height as u32,
            pixels.as_mut_ptr().cast(),
            &raw mut info,
            DIB_RGB_COLORS,
        );
        ReleaseDC(ptr::null_mut(), screen);
        lines
    };
    (lines == height).then_some((pixels, width, height))
}

/// The icon of the shell item `parsing_name` (like `shell:AppsFolder\<AppUserModelID>`), at
/// `size` pixels. COM must be initialised on this thread.
pub fn shell_icon(parsing_name: &str, size: i32) -> Option<Icon> {
    let name: Vec<u16> = parsing_name.encode_utf16().chain([0]).collect();
    let mut factory: *mut c_void = ptr::null_mut();
    // SAFETY: `name` is NUL-terminated and outlives the call; on success `factory` is an
    // IShellItemImageFactory we own a reference to.
    let hr = unsafe {
        SHCreateItemFromParsingName(
            name.as_ptr(),
            ptr::null_mut(),
            &IID_ISHELLITEMIMAGEFACTORY,
            &raw mut factory,
        )
    };
    if hr < 0 || factory.is_null() {
        return None;
    }
    // SAFETY: a COM object's first field is its vtable pointer, and this one's layout is
    // IUnknown followed by GetImage. The reference is released once, after the last call.
    let original = unsafe {
        let vtbl = *factory.cast::<*const ImageFactoryVtbl>();
        let mut bitmap: HBITMAP = ptr::null_mut();
        let hr = ((*vtbl).get_image)(
            factory,
            SIZE { cx: size, cy: size },
            SIIGBF_ICONONLY,
            &raw mut bitmap,
        );
        ((*vtbl).release)(factory);
        if hr < 0 || bitmap.is_null() {
            return None;
        }
        bitmap
    };
    let pixels = read_pixels(original);
    // SAFETY: GetImage handed us this bitmap; it is deleted once, here.
    unsafe { DeleteObject(original) };
    let (mut pixels, width, height) = pixels?;
    premultiply(&mut pixels);

    let info = bitmap_info(width, height);
    let mut bits: *mut c_void = ptr::null_mut();
    // SAFETY: on success `bits` points at the section's `width * height` 32-bit pixels, which
    // `pixels` exactly fills.
    let bitmap = unsafe {
        let bitmap = CreateDIBSection(
            ptr::null_mut(),
            &raw const info,
            DIB_RGB_COLORS,
            &raw mut bits,
            ptr::null_mut(),
            0,
        );
        if bitmap.is_null() || bits.is_null() {
            return None;
        }
        ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u8>(), pixels.len());
        bitmap
    };
    Some(Icon {
        bitmap,
        width,
        height,
    })
}

impl Icon {
    /// Draws the icon at `x, y` on the device context `hdc`, scaled to `size` pixels.
    pub fn draw(&self, hdc: isize, x: i32, y: i32, size: i32) -> bool {
        let hdc = hdc as HDC;
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        // SAFETY: Windows validates `hdc`; the memory DC is ours, gets its old bitmap back and
        // is deleted before returning, so our bitmap is only ever selected here.
        unsafe {
            let mem = CreateCompatibleDC(hdc);
            if mem.is_null() {
                return false;
            }
            let old = SelectObject(mem, self.bitmap);
            let ok = AlphaBlend(
                hdc,
                x,
                y,
                size,
                size,
                mem,
                0,
                0,
                self.width,
                self.height,
                blend,
            );
            SelectObject(mem, old);
            DeleteDC(mem);
            ok != 0
        }
    }
}

/// The DPI of the monitor nearest the point `x, y` (96 is 100% scaling).
pub fn monitor_dpi(x: i32, y: i32) -> u32 {
    let (mut dpi_x, mut dpi_y) = (0u32, 0u32);
    // SAFETY: MonitorFromPoint takes no pointers and always returns a monitor with this flag;
    // the out pointers are live locals.
    let hr = unsafe {
        let monitor = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &raw mut dpi_x, &raw mut dpi_y)
    };
    if hr < 0 || dpi_x == 0 { 96 } else { dpi_x }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premultiplies_straight_alpha_only() {
        let mut straight = vec![200, 100, 50, 128];
        premultiply(&mut straight);
        assert_eq!(straight, [100, 50, 25, 128]);

        let mut already = vec![60, 20, 10, 128];
        premultiply(&mut already);
        assert_eq!(already, [60, 20, 10, 128]);

        let mut opaque = vec![1, 2, 3, 0, 4, 5, 6, 0];
        premultiply(&mut opaque);
        assert_eq!(opaque, [1, 2, 3, 255, 4, 5, 6, 255]);
    }
}
