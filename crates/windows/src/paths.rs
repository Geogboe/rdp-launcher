use std::path::PathBuf;

use rdp_launch_core::AppPaths;

pub fn default_app_paths() -> AppPaths {
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        return AppPaths::from_root(PathBuf::from(local_app_data).join("RdpLaunch"));
    }

    if let Some(xdg_data_home) = std::env::var_os("XDG_DATA_HOME") {
        return AppPaths::from_root(PathBuf::from(xdg_data_home).join("rdp-launch"));
    }

    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    AppPaths::from_root(home.join(".local").join("share").join("rdp-launch"))
}
