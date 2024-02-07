use image::{DynamicImage, RgbaImage};
use std::{ffi::c_void, mem};
use windows::Win32::{
    Foundation::{HWND, RECT},
    Graphics::{
        Dwm::DwmIsCompositionEnabled,
        Gdi::{
            BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, GetCurrentObject, GetDIBits,
            GetObjectW, SelectObject, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS,
            OBJ_BITMAP, SRCCOPY,
        },
    },
    Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS},
    UI::WindowsAndMessaging::GetDesktopWindow,
};

use crate::error::{XCapError, XCapResult};

use super::{
    boxed::{BoxHBITMAP, BoxHDC},
    utils::{get_cropped_window_rect, get_os_major_version},
};

fn to_rgba_image(
    box_hdc_mem: BoxHDC,
    box_h_bitmap: BoxHBITMAP,
    width: i32,
    height: i32,
) -> XCapResult<RgbaImage> {
    let buffer_size = width * height * 4;
    let mut bitmap_info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biSizeImage: buffer_size as u32,
            biCompression: 0,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut buffer = vec![0u8; buffer_size as usize];

    unsafe {
        // 读取数据到 buffer 中
        let is_success = GetDIBits(
            *box_hdc_mem,
            *box_h_bitmap,
            0,
            height as u32,
            Some(buffer.as_mut_ptr().cast()),
            &mut bitmap_info,
            DIB_RGB_COLORS,
        ) == 0;

        if is_success {
            return Err(XCapError::new("Get RGBA data failed"));
        }
    };

    for src in buffer.chunks_exact_mut(4) {
        src.swap(0, 2);
        // fix https://github.com/nashaofu/xcap/issues/92#issuecomment-1910014951
        if src[3] == 0 && get_os_major_version() < 8 {
            src[3] = 255;
        }
    }

    RgbaImage::from_raw(width as u32, height as u32, buffer)
        .ok_or_else(|| XCapError::new("RgbaImage::from_raw failed"))
}

#[allow(unused)]
pub fn capture_monitor(x: i32, y: i32, width: i32, height: i32) -> XCapResult<RgbaImage> {
    unsafe {
        let hwnd = GetDesktopWindow();
        let box_hdc_desktop_window = BoxHDC::from(hwnd);

        // 内存中的HDC，使用 DeleteDC 函数释放
        // https://learn.microsoft.com/zh-cn/windows/win32/api/wingdi/nf-wingdi-createcompatibledc
        let box_hdc_mem = BoxHDC::new(CreateCompatibleDC(*box_hdc_desktop_window), None);
        let box_h_bitmap = BoxHBITMAP::new(CreateCompatibleBitmap(
            *box_hdc_desktop_window,
            width,
            height,
        ));

        // 使用SelectObject函数将这个位图选择到DC中
        SelectObject(*box_hdc_mem, *box_h_bitmap);

        // 拷贝原始图像到内存
        // 这里不需要缩放图片，所以直接使用BitBlt
        // 如需要缩放，则使用 StretchBlt
        BitBlt(
            *box_hdc_mem,
            0,
            0,
            width,
            height,
            *box_hdc_desktop_window,
            x,
            y,
            SRCCOPY,
        )?;

        to_rgba_image(box_hdc_mem, box_h_bitmap, width, height)
    }
}

#[derive(Debug, Clone)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl Rect {
    fn width(&self) -> i32 {
        self.right - self.left
    }

    fn height(&self) -> i32 {
        self.bottom - self.top
    }

    fn translate(&mut self, dx: i32, dy: i32) -> &mut Self {
        self.left += dx;
        self.top += dy;
        self.right += dx;
        self.bottom += dy;

        self
    }

    fn extend(
        &mut self,
        left_offset: i32,
        top_offset: i32,
        right_offset: i32,
        bottom_offset: i32,
    ) -> &mut Self {
        self.left -= left_offset;
        self.top -= top_offset;
        self.right += right_offset;
        self.bottom += bottom_offset;

        self
    }

    fn scale(&mut self, horizontal: f64, vertical: f64) {
        self.right += (self.width() as f64 * (horizontal - 1.0)).round() as i32;
        self.bottom += (self.height() as f64 * (vertical - 1.0)).round() as i32;
    }
}

impl From<RECT> for Rect {
    fn from(value: RECT) -> Self {
        Rect {
            left: value.left,
            top: value.top,
            right: value.right,
            bottom: value.bottom,
        }
    }
}

// fn GetFullscreenRect() {
//     return DesktopRect::MakeXYWH(GetSystemMetrics(SM_XVIRTUALSCREEN),
//                                  GetSystemMetrics(SM_YVIRTUALSCREEN),
//                                  GetSystemMetrics(SM_CXVIRTUALSCREEN),
//                                  GetSystemMetrics(SM_CYVIRTUALSCREEN));
//   }

#[allow(unused)]
pub fn capture_window(
    hwnd: HWND,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> XCapResult<RgbaImage> {
    unsafe {
        let box_hdc_window: BoxHDC = BoxHDC::from(hwnd);
        let (original_rect, cropped_rect) = get_cropped_window_rect(hwnd, true)?;

        let mut original_rect = Rect::from(original_rect);
        let mut cropped_rect = Rect::from(cropped_rect);

        let hgdi_obj = GetCurrentObject(*box_hdc_window, OBJ_BITMAP);
        let mut bitmap = BITMAP::default();

        let mut horizontal_scale = 1.0;
        let mut vertical_scale = 1.0;

        if GetObjectW(
            hgdi_obj,
            mem::size_of::<BITMAP>() as i32,
            Some(&mut bitmap as *mut BITMAP as *mut c_void),
        ) != 0
        {
            horizontal_scale = (bitmap.bmWidth as f64) / (original_rect.width() as f64);
            vertical_scale = (bitmap.bmHeight as f64) / (original_rect.height() as f64);

            original_rect.scale(horizontal_scale, vertical_scale);
            cropped_rect.scale(horizontal_scale, vertical_scale);
            // Translate `cropped_rect` to the left so that its position within
            // `original_rect` remains accurate after scaling.
            // See crbug.com/1083527 for more info.
            let translate_left =
                ((cropped_rect.left - original_rect.left) as f64 * (horizontal_scale - 1.0)) as i32;
            let translate_top =
                ((cropped_rect.top - original_rect.top) as f64 * (vertical_scale - 1.0)) as i32;

            cropped_rect.translate(translate_left, translate_top);
        }

        // 内存中的HDC，使用 DeleteDC 函数释放
        // https://learn.microsoft.com/zh-cn/windows/win32/api/wingdi/nf-wingdi-createcompatibledc
        let box_hdc_mem = BoxHDC::new(CreateCompatibleDC(*box_hdc_window), None);
        let box_h_bitmap = BoxHBITMAP::new(CreateCompatibleBitmap(
            *box_hdc_window,
            original_rect.width(),
            original_rect.height(),
        ));

        let previous_object = SelectObject(*box_hdc_mem, *box_h_bitmap);

        let mut is_success = false;

        // https://webrtc.googlesource.com/src.git/+/refs/heads/main/modules/desktop_capture/win/window_capturer_win_gdi.cc#301
        if get_os_major_version() >= 8 {
            is_success = PrintWindow(hwnd, *box_hdc_mem, PRINT_WINDOW_FLAGS(2)).as_bool();
        }

        if !is_success && DwmIsCompositionEnabled()?.as_bool() {
            is_success = PrintWindow(hwnd, *box_hdc_mem, PRINT_WINDOW_FLAGS(0)).as_bool();
        }

        if !is_success {
            is_success = BitBlt(
                *box_hdc_mem,
                0,
                0,
                original_rect.width(),
                original_rect.height(),
                *box_hdc_window,
                0,
                0,
                SRCCOPY,
            )
            .is_ok();
        }

        println!(
            "size {:?} {:?}",
            (&original_rect, original_rect.width()),
            (&cropped_rect, cropped_rect.width())
        );

        SelectObject(*box_hdc_mem, previous_object);

        let img = to_rgba_image(
            box_hdc_mem,
            box_h_bitmap,
            original_rect.width(),
            original_rect.height(),
        )?;

        let img = DynamicImage::from(img).crop(
            (cropped_rect.left - original_rect.left) as u32,
            (cropped_rect.top - original_rect.top) as u32,
            cropped_rect.width() as u32,
            cropped_rect.height() as u32,
        );

        Ok(img.into_rgba8())
    }
}
