use sysinfo::System;
use windows::Win32::{
    Foundation::{HWND, RECT},
    UI::WindowsAndMessaging::{
        GetSystemMetrics, GetWindowLongW, GetWindowPlacement, GetWindowRect, DS_MODALFRAME,
        GWL_STYLE, SM_CXBORDER, SM_CXSIZEFRAME, SM_CYBORDER, SM_CYSIZEFRAME, SW_SHOWMAXIMIZED,
        WINDOWPLACEMENT, WS_THICKFRAME,
    },
};

use crate::error::XCapResult;

pub(super) fn wide_string_to_string(wide_string: &[u16]) -> XCapResult<String> {
    let string = if let Some(null_pos) = wide_string.iter().position(|pos| *pos == 0) {
        String::from_utf16(&wide_string[..null_pos])?
    } else {
        String::from_utf16(&wide_string)?
    };

    Ok(string)
}

pub(super) fn get_os_major_version() -> u8 {
    System::os_version()
        .map(|os_version| {
            let strs: Vec<&str> = os_version.split(" ").collect();
            strs[0].parse::<u8>().unwrap_or(0)
        })
        .unwrap_or(0)
}

pub(super) fn is_window_maximized(hwnd: HWND) -> XCapResult<bool> {
    unsafe {
        let mut placement = WINDOWPLACEMENT::default();
        GetWindowPlacement(hwnd, &mut placement)?;

        Ok(placement.showCmd == SW_SHOWMAXIMIZED.0 as u32)
    }
}

pub(super) fn get_cropped_window_rect(
    hwnd: HWND,
    avoid_cropping_border: bool,
) -> XCapResult<(RECT, RECT)> {
    unsafe {
        let mut window_rect = RECT::default();

        GetWindowRect(hwnd, &mut window_rect)?;

        let original_rect = window_rect;
        let mut cropped_rect = window_rect.clone();
        let is_maximized = is_window_maximized(hwnd)?;

        // As of Windows8, transparent resize borders are added by the OS at
        // left/bottom/right sides of a resizeable window. If the cropped window
        // doesn't remove these borders, the background will be exposed a bit.
        if get_os_major_version() >= 8 || is_maximized {
            // Only apply this cropping to windows with a resize border (otherwise,
            // it'd clip the edges of captured pop-up windows without this border).
            let gwl_style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
            if gwl_style & WS_THICKFRAME.0 != 0 || gwl_style & DS_MODALFRAME as u32 != 0 {
                let mut width = GetSystemMetrics(SM_CXSIZEFRAME);
                let mut bottom_height = GetSystemMetrics(SM_CYSIZEFRAME);
                let visible_border_height = GetSystemMetrics(SM_CYBORDER);
                let mut top_height = visible_border_height;

                // If requested, avoid cropping the visible window border. This is used
                // for pop-up windows to include their border, but not for the outermost
                // window (where a partially-transparent border  may expose the
                // background a bit).
                if avoid_cropping_border {
                    width = (width - GetSystemMetrics(SM_CXBORDER)).max(0);
                    bottom_height = (bottom_height - visible_border_height).max(0);
                    top_height = 0;
                }

                let left_offset = -width;
                let top_offset = -top_height;
                let right_offset = -width;
                let bottom_offset = -bottom_height;

                cropped_rect.left -= left_offset;
                cropped_rect.top -= top_offset;
                cropped_rect.right += right_offset;
                cropped_rect.bottom += bottom_offset;
            }
        }

        Ok((original_rect, cropped_rect))
    }
}
