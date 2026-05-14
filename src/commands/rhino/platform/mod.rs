#[cfg(target_os = "macos")]
mod macos;

#[cfg(any(target_os = "windows", target_os = "linux"))]
mod windows;

#[cfg(not(target_os = "windows"))]
mod unsupported;

#[cfg(target_os = "macos")]
pub use macos::{capture_window, is_app_running, launch_app, request_quit};

#[cfg(target_os = "windows")]
pub use windows::{capture_window, is_app_running, launch_app, request_quit};

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub use unsupported::{capture_window, is_app_running, launch_app, request_quit};

// On Linux we dispatch at runtime: WSL → windows.rs (Rhino.exe), pure Linux → unsupported.
#[cfg(target_os = "linux")]
pub use linux_dispatch::{capture_window, is_app_running, launch_app, request_quit};

#[cfg(target_os = "linux")]
mod linux_dispatch {
    use std::path::Path;

    use crate::error::Result;

    pub fn launch_app(app: &str, script: Option<&str>) -> Result<()> {
        if super::is_wsl() {
            super::windows::launch_app(app, script)
        } else {
            super::unsupported::launch_app(app, script)
        }
    }

    pub fn request_quit(app: &str) -> Result<()> {
        if super::is_wsl() {
            super::windows::request_quit(app)
        } else {
            super::unsupported::request_quit(app)
        }
    }

    pub fn is_app_running(app: &str) -> Result<bool> {
        if super::is_wsl() {
            super::windows::is_app_running(app)
        } else {
            super::unsupported::is_app_running(app)
        }
    }

    pub fn capture_window(
        app: &str,
        out: &Path,
        window_id: Option<u64>,
        activate: bool,
        no_shadow: bool,
    ) -> Result<()> {
        if super::is_wsl() {
            super::windows::capture_window(app, out, window_id, activate, no_shadow)
        } else {
            super::unsupported::capture_window(app, out, window_id, activate, no_shadow)
        }
    }
}

#[cfg(target_os = "linux")]
fn is_wsl() -> bool {
    use std::sync::OnceLock;
    static CACHE: OnceLock<bool> = OnceLock::new();
    *CACHE.get_or_init(|| {
        std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .map(|s| is_wsl_from_release(&s))
            .unwrap_or(false)
    })
}

fn is_wsl_from_release(osrelease: &str) -> bool {
    let lower = osrelease.to_ascii_lowercase();
    lower.contains("microsoft") || lower.contains("wsl")
}

#[cfg(test)]
mod tests {
    use super::is_wsl_from_release;

    #[test]
    fn detects_wsl2_release() {
        assert!(is_wsl_from_release("5.15.153.1-microsoft-standard-WSL2"));
    }

    #[test]
    fn detects_wsl1_release() {
        assert!(is_wsl_from_release("4.4.0-19041-Microsoft"));
    }

    #[test]
    fn rejects_native_linux_release() {
        assert!(!is_wsl_from_release("6.6.0-1018-aws"));
        assert!(!is_wsl_from_release("5.10.0-21-amd64"));
    }

    #[test]
    fn rejects_empty_release() {
        assert!(!is_wsl_from_release(""));
    }
}
