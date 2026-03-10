use thiserror::Error;

#[cfg(target_os = "windows")]
pub fn reveal_process_window(process_id: u32) -> Result<bool, RevealWindowError> {
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SW_RESTORE, SetForegroundWindow,
        ShowWindow,
    };
    use windows::core::BOOL;

    #[derive(Default)]
    struct Search {
        process_id: u32,
        found: bool,
    }

    unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = unsafe { &mut *(lparam.0 as *mut Search) };
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }
        if pid == search.process_id && unsafe { IsWindowVisible(hwnd).as_bool() } {
            let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
            let _ = unsafe { SetForegroundWindow(hwnd) };
            search.found = true;
            return BOOL(0);
        }
        BOOL(1)
    }

    let mut search = Search {
        process_id,
        found: false,
    };

    unsafe {
        EnumWindows(
            Some(callback),
            LPARAM((&mut search as *mut Search).cast::<()>() as isize),
        )
        .map_err(|error| RevealWindowError::Api(error.to_string()))?;
    }

    Ok(search.found)
}

#[cfg(not(target_os = "windows"))]
pub fn reveal_process_window(_process_id: u32) -> Result<bool, RevealWindowError> {
    Err(RevealWindowError::UnsupportedPlatform)
}

#[derive(Debug, Error)]
pub enum RevealWindowError {
    #[error("reveal-window is only available on Windows")]
    UnsupportedPlatform,
    #[error("Win32 API error: {0}")]
    Api(String),
}
