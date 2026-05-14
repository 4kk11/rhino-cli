#![allow(dead_code)]
// On Linux these items are unused until Phase 3 wires WSL dispatch through
// them. On Windows native everything is consumed via `platform/mod.rs`.

use std::path::PathBuf;
use std::process::Command;

use crate::error::{CliError, Result};

const RHINO_EXE_ENV: &str = "RHINO_CLI_RHINO_EXE";
const RHINO_EXE_NAME: &str = "Rhino.exe";

#[cfg(target_os = "windows")]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

pub fn launch_app(app: &str, script: Option<&str>) -> Result<()> {
    let exe = resolve_rhino_exe(app)?;

    let mut command = Command::new(&exe);
    if let Some(script) = script {
        command.arg(format!("/runscript={script}"));
    }

    detach(&mut command);

    command
        .spawn()
        .map(|_child| ())
        .map_err(|error| {
            CliError::Other(format!(
                "failed to launch Rhino at {}: {error}",
                exe.display()
            ))
        })
}

pub fn request_quit(_app: &str) -> Result<()> {
    let output = Command::new("taskkill.exe")
        .args(["/IM", RHINO_EXE_NAME])
        .output()
        .map_err(|error| {
            CliError::Other(format!("failed to run taskkill.exe: {error}"))
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    if taskkill_says_not_found(output.status.code(), &stderr, &stdout) {
        return Ok(());
    }

    Err(CliError::Other(format!(
        "taskkill.exe failed (exit {:?}): {}",
        output.status.code(),
        message_from(&stderr, &stdout, output.status.to_string())
    )))
}

pub fn is_app_running(_app: &str) -> Result<bool> {
    let output = Command::new("tasklist.exe")
        .args([
            "/FI",
            &format!("IMAGENAME eq {RHINO_EXE_NAME}"),
            "/FO",
            "CSV",
            "/NH",
        ])
        .output()
        .map_err(|error| {
            CliError::Other(format!("failed to run tasklist.exe: {error}"))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(CliError::Other(format!(
            "tasklist.exe failed (exit {:?}): {}",
            output.status.code(),
            message_from(&stderr, &stdout, output.status.to_string())
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_tasklist_running(&stdout))
}

pub fn capture_window(
    _app: &str,
    _out: &std::path::Path,
    _window_id: Option<u64>,
    _activate: bool,
    _no_shadow: bool,
) -> Result<()> {
    Err(CliError::Other(
        "Rhino window screenshot is not yet implemented on Windows.".to_string(),
    ))
}

#[cfg(target_os = "windows")]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP);
}

#[cfg(not(target_os = "windows"))]
fn detach(_command: &mut Command) {
    // WSL: Rhino is a GUI process, so spawn() alone is enough — the window
    // survives the CLI exit. creation_flags is Windows-only.
}

/// Resolve the Rhino executable to launch, returning the path in a form that
/// the current target can invoke (Windows path on Windows, /mnt path on WSL).
pub(crate) fn resolve_rhino_exe(app: &str) -> Result<PathBuf> {
    if let Some(env_value) = std::env::var_os(RHINO_EXE_ENV) {
        let raw = env_value.to_string_lossy().into_owned();
        let candidate = normalize_path(&raw);
        if candidate.exists() {
            return Ok(candidate);
        }
        return Err(CliError::Other(format!(
            "{RHINO_EXE_ENV}={raw} does not exist (resolved to {})",
            candidate.display()
        )));
    }

    let mut versions: Vec<u32> = Vec::new();
    if let Some(v) = parse_rhino_version(app) {
        versions.push(v);
    }
    for default in [8u32, 7u32] {
        if !versions.contains(&default) {
            versions.push(default);
        }
    }

    let mut tried: Vec<PathBuf> = Vec::with_capacity(versions.len());
    for version in versions {
        let windows_path = format!("C:\\Program Files\\Rhino {version}\\System\\Rhino.exe");
        let candidate = normalize_path(&windows_path);
        if candidate.exists() {
            return Ok(candidate);
        }
        tried.push(candidate);
    }

    let mut message = String::from("could not find Rhino.exe. Tried:");
    for path in &tried {
        message.push_str("\n  ");
        message.push_str(&path.display().to_string());
    }
    message.push_str(&format!(
        "\nSet {RHINO_EXE_ENV} to override (e.g. C:\\Program Files\\Rhino 8\\System\\Rhino.exe)."
    ));
    Err(CliError::Other(message))
}

/// Convert a Windows-style path to whatever this target can actually open.
/// On Windows native this is a pass-through; on Linux (WSL) it rewrites
/// `C:\foo\bar` to `/mnt/c/foo/bar`. `/mnt/...` input is preserved.
fn normalize_path(path: &str) -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        convert_windows_to_wsl(path)
    }
    #[cfg(not(target_os = "linux"))]
    {
        PathBuf::from(path)
    }
}

fn convert_windows_to_wsl(path: &str) -> PathBuf {
    if path.starts_with('/') {
        return PathBuf::from(path);
    }

    let bytes = path.as_bytes();
    if bytes.len() >= 2
        && bytes[1] == b':'
        && bytes[0].is_ascii_alphabetic()
    {
        let drive = (bytes[0] as char).to_ascii_lowercase();
        let rest_raw = if bytes.len() >= 3 && (bytes[2] == b'\\' || bytes[2] == b'/') {
            &path[3..]
        } else {
            &path[2..]
        };
        let rest = rest_raw.replace('\\', "/");
        let rest_trimmed = rest.trim_start_matches('/');
        return if rest_trimmed.is_empty() {
            PathBuf::from(format!("/mnt/{drive}"))
        } else {
            PathBuf::from(format!("/mnt/{drive}/{rest_trimmed}"))
        };
    }

    // Unrecognized (relative path, UNC `\\server\share`, etc.) — pass through.
    PathBuf::from(path)
}

fn parse_rhino_version(app: &str) -> Option<u32> {
    let lower = app.to_ascii_lowercase();
    let mut idx = lower.find("rhino")?;
    idx += "rhino".len();
    let rest = app.get(idx..)?.trim_start();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

fn parse_tasklist_running(stdout: &str) -> bool {
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("INFO:") {
            return false;
        }
        // CSV /NH: first field is the image name, quoted.
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("\"rhino.exe\"") || lower.starts_with("rhino.exe") {
            return true;
        }
    }
    false
}

fn taskkill_says_not_found(exit_code: Option<i32>, stderr: &str, stdout: &str) -> bool {
    if matches!(exit_code, Some(128)) {
        return true;
    }
    let haystack = format!("{stderr}\n{stdout}").to_ascii_lowercase();
    haystack.contains("not found") || haystack.contains("no running")
}

fn message_from(stderr: &str, stdout: &str, fallback: String) -> String {
    let s = stderr.trim();
    if !s.is_empty() {
        return s.to_string();
    }
    let o = stdout.trim();
    if !o.is_empty() {
        return o.to_string();
    }
    fallback
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_simple_c_drive() {
        assert_eq!(
            convert_windows_to_wsl("C:\\Program Files\\Rhino 8\\System\\Rhino.exe"),
            PathBuf::from("/mnt/c/Program Files/Rhino 8/System/Rhino.exe")
        );
    }

    #[test]
    fn convert_alternate_drive() {
        assert_eq!(
            convert_windows_to_wsl("D:\\Tools\\rhino.exe"),
            PathBuf::from("/mnt/d/Tools/rhino.exe")
        );
    }

    #[test]
    fn convert_lowercase_drive() {
        assert_eq!(
            convert_windows_to_wsl("c:\\Foo"),
            PathBuf::from("/mnt/c/Foo")
        );
    }

    #[test]
    fn convert_mnt_pass_through() {
        assert_eq!(
            convert_windows_to_wsl("/mnt/c/Program Files/Rhino 8/System/Rhino.exe"),
            PathBuf::from("/mnt/c/Program Files/Rhino 8/System/Rhino.exe")
        );
    }

    #[test]
    fn convert_unc_pass_through() {
        // UNC `\\server\share` is out of scope; leave as-is.
        assert_eq!(
            convert_windows_to_wsl("\\\\server\\share\\file.exe"),
            PathBuf::from("\\\\server\\share\\file.exe")
        );
    }

    #[test]
    fn parse_version_from_rhino_8() {
        assert_eq!(parse_rhino_version("Rhino 8"), Some(8));
    }

    #[test]
    fn parse_version_from_no_space() {
        assert_eq!(parse_rhino_version("Rhino8"), Some(8));
    }

    #[test]
    fn parse_version_from_decorated() {
        assert_eq!(parse_rhino_version("Rhino 8.0-Test"), Some(8));
    }

    #[test]
    fn parse_version_none_when_no_digits() {
        assert_eq!(parse_rhino_version("RhinoWIP"), None);
        assert_eq!(parse_rhino_version("Rhinoceros"), None);
    }

    #[test]
    fn tasklist_detects_running() {
        let out = "\"Rhino.exe\",\"12345\",\"Console\",\"1\",\"123,456 K\"\r\n";
        assert!(parse_tasklist_running(out));
    }

    #[test]
    fn tasklist_detects_running_case_insensitive() {
        let out = "\"RHINO.EXE\",\"12345\",\"Console\",\"1\",\"123,456 K\"\r\n";
        assert!(parse_tasklist_running(out));
    }

    #[test]
    fn tasklist_detects_not_running_when_info() {
        let out = "INFO: No tasks are running which match the specified criteria.\r\n";
        assert!(!parse_tasklist_running(out));
    }

    #[test]
    fn tasklist_detects_not_running_when_empty() {
        assert!(!parse_tasklist_running(""));
    }

    #[test]
    fn taskkill_recognizes_exit_128() {
        assert!(taskkill_says_not_found(Some(128), "", ""));
    }

    #[test]
    fn taskkill_recognizes_not_found_text() {
        assert!(taskkill_says_not_found(
            Some(1),
            "ERROR: The process \"Rhino.exe\" not found.",
            ""
        ));
    }

    #[test]
    fn taskkill_treats_other_errors_as_error() {
        assert!(!taskkill_says_not_found(Some(1), "Access is denied.", ""));
    }
}
