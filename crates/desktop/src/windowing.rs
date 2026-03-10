#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) struct DesktopShellSpec {
    pub(crate) title: &'static str,
    pub(crate) show_native_menu: bool,
    pub(crate) initial_size: (f64, f64),
    pub(crate) min_size: (f64, f64),
}

const MIN_WINDOW_SIZE: (f64, f64) = (540.0, 560.0);
const FALLBACK_WINDOW_SIZE: (f64, f64) = (660.0, 640.0);
const MIN_COMPACT_SIZE: (f64, f64) = (600.0, 560.0);
const MAX_COMPACT_SIZE: (f64, f64) = (720.0, 760.0);
const WORK_AREA_WIDTH_FACTOR: f64 = 0.31;
const WORK_AREA_HEIGHT_FACTOR: f64 = 0.70;

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) fn desktop_shell_spec() -> DesktopShellSpec {
    DesktopShellSpec {
        title: "RDP Launch",
        show_native_menu: false,
        initial_size: initial_window_size(desktop_work_area_size()),
        min_size: MIN_WINDOW_SIZE,
    }
}

fn initial_window_size(work_area: Option<(f64, f64)>) -> (f64, f64) {
    let Some((width, height)) = work_area else {
        return FALLBACK_WINDOW_SIZE;
    };

    let compact_width = (width * WORK_AREA_WIDTH_FACTOR).round();
    let compact_height = (height * WORK_AREA_HEIGHT_FACTOR).round();

    (
        compact_width.clamp(MIN_COMPACT_SIZE.0, MAX_COMPACT_SIZE.0),
        compact_height.clamp(MIN_COMPACT_SIZE.1, MAX_COMPACT_SIZE.1),
    )
}

#[cfg(target_os = "windows")]
fn desktop_work_area_size() -> Option<(f64, f64)> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{SPI_GETWORKAREA, SystemParametersInfoW};

    let mut work_area = RECT::default();
    unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some((&mut work_area as *mut RECT).cast()),
            Default::default(),
        )
        .ok()
        .map(|_| {
            (
                f64::from(work_area.right - work_area.left),
                f64::from(work_area.bottom - work_area.top),
            )
        })
    }
}

#[cfg(not(target_os = "windows"))]
fn desktop_work_area_size() -> Option<(f64, f64)> {
    None
}

#[cfg(target_os = "windows")]
pub(crate) fn desktop_launch_config() -> dioxus::desktop::Config {
    use dioxus::desktop::{Config, LogicalSize, WindowBuilder, muda::Menu};

    let spec = desktop_shell_spec();
    let window = WindowBuilder::new()
        .with_title(spec.title)
        .with_inner_size(LogicalSize::new(spec.initial_size.0, spec.initial_size.1))
        .with_min_inner_size(LogicalSize::new(spec.min_size.0, spec.min_size.1));

    let config = Config::new().with_window(window);
    if spec.show_native_menu {
        config
    } else {
        config.with_menu(None::<Menu>)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FALLBACK_WINDOW_SIZE, MIN_COMPACT_SIZE, MIN_WINDOW_SIZE, desktop_shell_spec,
        initial_window_size,
    };

    #[test]
    fn desktop_shell_spec_uses_app_title_and_trims_native_menu() {
        let spec = desktop_shell_spec();

        assert_eq!(spec.title, "RDP Launch");
        assert!(!spec.show_native_menu);
    }

    #[test]
    fn desktop_shell_spec_sets_sane_initial_window_size() {
        let spec = desktop_shell_spec();

        assert!(spec.initial_size.0 >= MIN_COMPACT_SIZE.0.min(FALLBACK_WINDOW_SIZE.0));
        assert!(spec.initial_size.1 >= MIN_WINDOW_SIZE.1.min(FALLBACK_WINDOW_SIZE.1));
        assert_eq!(spec.min_size, MIN_WINDOW_SIZE);
    }

    #[test]
    fn initial_window_size_clamps_small_work_areas_to_compact_minimums() {
        assert_eq!(initial_window_size(Some((1366.0, 768.0))), (600.0, 560.0));
    }

    #[test]
    fn initial_window_size_prefers_compact_stacked_default_on_standard_desktops() {
        assert_eq!(initial_window_size(Some((1920.0, 1080.0))), (600.0, 756.0));
    }

    #[test]
    fn initial_window_size_caps_large_work_areas_at_compact_maximums() {
        assert_eq!(initial_window_size(Some((2560.0, 1440.0))), (720.0, 760.0));
    }

    #[test]
    fn initial_window_size_uses_fallback_when_work_area_is_unavailable() {
        assert_eq!(initial_window_size(None), FALLBACK_WINDOW_SIZE);
    }
}
