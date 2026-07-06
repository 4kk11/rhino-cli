use std::time::{Duration, Instant};

use crate::error::{CliError, Result};

const MACOS_COMMAND_TIMEOUT_SECS: u64 = 15;

pub fn launch_app(app: &str, script: Option<&str>) -> Result<()> {
    use std::process::Command;

    let mut command = Command::new("open");
    command.args(["-a", app]);
    if let Some(script) = script {
        command.args(["--args", "-runscript", script]);
    }

    let output = command
        .output()
        .map_err(|error| CliError::Other(format!("failed to launch {app}: {error}")))?;
    if output.status.success() {
        return Ok(());
    }

    Err(CliError::Other(format!(
        "failed to launch {app}: {}",
        command_output_message(&output)
    )))
}

pub fn capture_window(
    app: &str,
    out: &std::path::Path,
    window_id: Option<u64>,
    activate: bool,
    no_shadow: bool,
) -> Result<()> {
    use std::process::Command;

    if !is_app_running(app)? {
        return Err(CliError::Connect(format!("{app} is not running")));
    }
    ensure_screen_capture_access()?;

    let window_id = match window_id {
        Some(window_id) => window_id,
        None => app_window_id(app, activate)?,
    };

    let mut command = Command::new("screencapture");
    command.args(["-x"]);
    if no_shadow {
        command.arg("-o");
    }
    command.arg("-l");
    command.arg(window_id.to_string());
    command.arg(out);

    let output = run_command_with_timeout(
        &mut command,
        Duration::from_secs(MACOS_COMMAND_TIMEOUT_SECS),
    )
    .map_err(|error| CliError::Other(format!("failed to run screencapture: {error}")))?;
    if output.status.success() && out.exists() {
        return Ok(());
    }

    if output.status.success() {
        return Err(CliError::Other(format!(
            "screencapture finished but did not create {}. Check macOS Screen Recording permission for this terminal.",
            out.display()
        )));
    }

    Err(CliError::Other(format!(
        "screencapture failed: {}. Check macOS Screen Recording permission for this terminal.",
        command_output_message(&output)
    )))
}

fn ensure_screen_capture_access() -> Result<()> {
    let access = core_graphics::access::ScreenCaptureAccess::default();
    if access.preflight() || access.request() {
        return Ok(());
    }

    Err(CliError::Other(
        "macOS Screen Recording permission is required to capture the Rhino window. Grant permission to the terminal running rhino-cli, then retry.".to_string(),
    ))
}

fn app_window_id(app: &str, activate: bool) -> Result<u64> {
    if activate {
        activate_app(app)?;
        std::thread::sleep(Duration::from_millis(500));
    }

    visible_app_window_id(app)
}

fn activate_app(app: &str) -> Result<()> {
    use std::process::Command;

    let mut command = Command::new("open");
    command.args(["-a", app]);

    let output = run_command_with_timeout(
        &mut command,
        Duration::from_secs(MACOS_COMMAND_TIMEOUT_SECS),
    )
    .map_err(|error| CliError::Other(format!("failed to activate {app}: {error}")))?;
    if output.status.success() {
        return Ok(());
    }

    Err(CliError::Other(format!(
        "failed to activate {app}: {}",
        command_output_message(&output)
    )))
}

fn visible_app_window_id(app: &str) -> Result<u64> {
    use core_foundation::array::CFArray;
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::number::CFNumber;
    use core_foundation::string::{CFString, CFStringRef};
    use core_graphics::window::{
        kCGNullWindowID, kCGWindowLayer, kCGWindowListExcludeDesktopElements,
        kCGWindowListOptionOnScreenOnly, kCGWindowName, kCGWindowNumber, kCGWindowOwnerName,
        CGWindowListCopyWindowInfo,
    };

    fn value_for_key(
        window: &CFDictionary<CFString, CFType>,
        key_ref: CFStringRef,
    ) -> Option<CFType> {
        let key = unsafe { CFString::wrap_under_get_rule(key_ref) };
        window.find(&key).map(|value| (*value).clone())
    }

    fn string_for_key(
        window: &CFDictionary<CFString, CFType>,
        key_ref: CFStringRef,
    ) -> Option<String> {
        value_for_key(window, key_ref)
            .and_then(|value| value.downcast::<CFString>())
            .map(|value| value.to_string())
    }

    fn i64_for_key(window: &CFDictionary<CFString, CFType>, key_ref: CFStringRef) -> Option<i64> {
        value_for_key(window, key_ref)
            .and_then(|value| value.downcast::<CFNumber>())
            .and_then(|value| value.to_i64())
    }

    let options = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
    let windows_ref = unsafe { CGWindowListCopyWindowInfo(options, kCGNullWindowID) };
    if windows_ref.is_null() {
        return Err(CliError::Other(
            "failed to read macOS window list from CoreGraphics".to_string(),
        ));
    }
    let windows: CFArray<CFDictionary<CFString, CFType>> =
        unsafe { TCFType::wrap_under_create_rule(windows_ref) };
    let mut matching_owners = Vec::new();

    for window in windows.iter() {
        let owner = string_for_key(&window, unsafe { kCGWindowOwnerName }).unwrap_or_default();
        if !owner_matches_app(&owner, app) {
            continue;
        }
        let title = string_for_key(&window, unsafe { kCGWindowName }).unwrap_or_default();
        matching_owners.push(if title.is_empty() {
            owner.clone()
        } else {
            format!("{owner}: {title}")
        });

        let layer = i64_for_key(&window, unsafe { kCGWindowLayer }).unwrap_or(-1);
        if layer != 0 {
            continue;
        }

        if let Some(window_id) = i64_for_key(&window, unsafe { kCGWindowNumber }) {
            if window_id > 0 {
                return Ok(window_id as u64);
            }
        }
    }

    let details = if matching_owners.is_empty() {
        "no matching owner windows were visible".to_string()
    } else {
        format!("matching windows: {}", matching_owners.join(", "))
    };
    Err(CliError::Other(format!(
        "failed to find a visible {app} window id via CoreGraphics ({details})"
    )))
}

fn owner_matches_app(owner: &str, app: &str) -> bool {
    owner == app
        || (app.starts_with("Rhino") && (owner == "Rhinoceros" || owner.starts_with("Rhino")))
}

pub fn request_quit(app: &str) -> Result<()> {
    let output = run_osascript(&format!("quit app \"{app}\""))
        .map_err(|error| CliError::Other(format!("failed to ask {app} to quit: {error}")))?;
    if output.status.success() {
        return Ok(());
    }

    Err(CliError::Other(format!(
        "failed to ask {app} to quit: {}",
        command_output_message(&output)
    )))
}

pub fn is_app_running(app: &str) -> Result<bool> {
    let output = run_osascript(&format!("application \"{app}\" is running"))
        .map_err(|error| CliError::Other(format!("failed to query {app} state: {error}")))?;
    if !output.status.success() {
        return Err(CliError::Other(format!(
            "failed to query {app} state: {}",
            command_output_message(&output)
        )));
    }

    match String::from_utf8_lossy(&output.stdout).trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(CliError::Other(format!(
            "unexpected app state response for {app}: {other}"
        ))),
    }
}

fn run_osascript(script: &str) -> std::io::Result<std::process::Output> {
    let mut command = std::process::Command::new("osascript");
    command.args(["-e", script]);
    run_command_with_timeout(
        &mut command,
        Duration::from_secs(MACOS_COMMAND_TIMEOUT_SECS),
    )
}

fn run_command_with_timeout(
    command: &mut std::process::Command,
    timeout: Duration,
) -> std::io::Result<std::process::Output> {
    use std::io;
    use std::process::Stdio;

    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let started = Instant::now();

    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output();
        }

        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("timed out after {}s", timeout.as_secs()),
            ));
        }

        std::thread::sleep(Duration::from_millis(50));
    }
}

fn command_output_message(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        return stdout;
    }

    output.status.to_string()
}
