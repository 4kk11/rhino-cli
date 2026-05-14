use crate::error::{CliError, Result};

pub fn launch_app(_app: &str, _script: Option<&str>) -> Result<()> {
    Err(CliError::Other(
        "Rhino launch is only supported on macOS, Windows, and WSL.".to_string(),
    ))
}

pub fn request_quit(_app: &str) -> Result<()> {
    Err(CliError::Other(
        "Rhino shutdown is only supported on macOS, Windows, and WSL.".to_string(),
    ))
}

pub fn is_app_running(_app: &str) -> Result<bool> {
    Err(CliError::Other(
        "Rhino app state query is only supported on macOS, Windows, and WSL.".to_string(),
    ))
}

pub fn capture_window(
    _app: &str,
    _out: &std::path::Path,
    _window_id: Option<u64>,
    _activate: bool,
    _no_shadow: bool,
) -> Result<()> {
    Err(CliError::Other(
        "Rhino window screenshot is only supported on macOS.".to_string(),
    ))
}
