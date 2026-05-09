use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::json;

use crate::commands::CommandContext;
use crate::error::{CliError, Result};

const DEFAULT_APP: &str = "Rhino 8";
const DEFAULT_LAUNCH_TIMEOUT_SECS: u64 = 120;
const DEFAULT_SHUTDOWN_TIMEOUT_SECS: u64 = 30;
#[cfg(target_os = "macos")]
const MACOS_COMMAND_TIMEOUT_SECS: u64 = 15;

#[derive(Clone, Debug)]
pub struct LaunchArgs {
    pub app: String,
    pub timeout: Duration,
    pub restart: bool,
    pub no_wait: bool,
    pub new_model: bool,
    pub script: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ShutdownArgs {
    pub app: String,
    pub timeout: Duration,
}

#[derive(Clone, Debug)]
pub struct ScreenshotArgs {
    pub app: String,
    pub out: Option<PathBuf>,
    pub no_activate: bool,
    pub window_id: Option<u64>,
    pub no_shadow: bool,
}

impl Default for LaunchArgs {
    fn default() -> Self {
        Self {
            app: DEFAULT_APP.to_string(),
            timeout: Duration::from_secs(DEFAULT_LAUNCH_TIMEOUT_SECS),
            restart: false,
            no_wait: false,
            new_model: false,
            script: None,
        }
    }
}

impl Default for ShutdownArgs {
    fn default() -> Self {
        Self {
            app: DEFAULT_APP.to_string(),
            timeout: Duration::from_secs(DEFAULT_SHUTDOWN_TIMEOUT_SECS),
        }
    }
}

impl Default for ScreenshotArgs {
    fn default() -> Self {
        Self {
            app: DEFAULT_APP.to_string(),
            out: None,
            no_activate: false,
            window_id: None,
            no_shadow: false,
        }
    }
}

pub fn launch(ctx: &CommandContext, args: LaunchArgs) -> Result<()> {
    validate_app_name(&args.app)?;

    if !args.restart && is_plugin_ready(ctx) {
        if args.new_model || args.script.is_some() {
            return Err(CliError::InvalidInput(
                "Rhino is already running; use `rhino-cli launch --restart --new-model` to apply launch-time model opening, or `rhino-cli new-model` inside an existing modeling session."
                    .to_string(),
            ));
        }
        if ctx.verbose && !ctx.quiet {
            eprintln!("Plugin already ready on {}:{}", ctx.host, ctx.port);
        }
        return Ok(());
    }

    if !args.restart && is_app_running(&args.app)? {
        return Err(CliError::InvalidInput(format!(
            "{app} is already running, but RhinoCliPlugin is not reachable at {host}:{port}. Use `rhino-cli launch --restart --port {port}` to apply the requested plugin port.",
            app = args.app,
            host = ctx.host,
            port = ctx.port
        )));
    }

    if args.restart {
        shutdown(
            ctx,
            ShutdownArgs {
                app: args.app.clone(),
                timeout: Duration::from_secs(DEFAULT_SHUTDOWN_TIMEOUT_SECS),
            },
        )?;
    }

    write_plugin_launch_config(ctx.port)?;

    let startup_script = args
        .script
        .as_deref()
        .or_else(|| args.new_model.then_some("_NoEcho"));
    launch_app(&args.app, startup_script)?;

    if args.no_wait {
        return Ok(());
    }

    wait_until_ready(ctx, args.timeout)
}

fn write_plugin_launch_config(port: u16) -> Result<()> {
    let path = plugin_launch_config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            CliError::Other(format!(
                "failed to create RhinoCliPlugin config directory {}: {error}",
                parent.display()
            ))
        })?;
    }

    let content = serde_json::to_string_pretty(&json!({ "port": port }))?;
    std::fs::write(&path, content).map_err(|error| {
        CliError::Other(format!(
            "failed to write RhinoCliPlugin launch config {}: {error}",
            path.display()
        ))
    })
}

fn plugin_launch_config_path() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")
            .ok_or_else(|| CliError::Other("HOME is not set".to_string()))?;
        return Ok(PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("rhino-cli")
            .join("RhinoCliPlugin")
            .join("config.json"));
    }

    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var_os("APPDATA")
            .ok_or_else(|| CliError::Other("APPDATA is not set".to_string()))?;
        return Ok(PathBuf::from(appdata)
            .join("rhino-cli")
            .join("RhinoCliPlugin")
            .join("config.json"));
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let home = std::env::var_os("HOME")
            .ok_or_else(|| CliError::Other("HOME is not set".to_string()))?;
        Ok(PathBuf::from(home)
            .join(".config")
            .join("rhino-cli")
            .join("RhinoCliPlugin")
            .join("config.json"))
    }
}

pub fn shutdown(ctx: &CommandContext, args: ShutdownArgs) -> Result<()> {
    validate_app_name(&args.app)?;

    if !is_app_running(&args.app)? {
        if ctx.verbose && !ctx.quiet {
            eprintln!("{} is not running", args.app);
        }
        return Ok(());
    }

    request_quit(&args.app)?;
    wait_until_not_running(&args.app, args.timeout)
}

pub fn screenshot(ctx: &CommandContext, args: ScreenshotArgs) -> Result<()> {
    validate_app_name(&args.app)?;
    let out = args.out.unwrap_or_else(default_screenshot_path);
    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|error| {
            CliError::Other(format!(
                "failed to create screenshot directory {}: {error}",
                parent.display()
            ))
        })?;
    }

    capture_window(
        &args.app,
        &out,
        args.window_id,
        !args.no_activate,
        args.no_shadow,
    )?;

    if !ctx.quiet {
        println!("{}", out.display());
    }
    Ok(())
}

pub fn app_running(app: &str) -> Result<bool> {
    validate_app_name(app)?;
    is_app_running(app)
}

fn is_plugin_ready(ctx: &CommandContext) -> bool {
    ctx.client()
        .call("system.ping", json!(null))
        .map(|result| result.get("pong").and_then(|value| value.as_bool()) == Some(true))
        .unwrap_or(false)
}

fn wait_until_ready(ctx: &CommandContext, timeout: Duration) -> Result<()> {
    let started = Instant::now();
    let interval = Duration::from_millis(500);
    let mut last_error: Option<String> = None;

    while started.elapsed() < timeout {
        match ctx.client().call("system.ping", json!(null)) {
            Ok(result) if result.get("pong").and_then(|value| value.as_bool()) == Some(true) => {
                return Ok(());
            }
            Ok(_) => {
                last_error = Some("system.ping did not return pong=true".to_string());
            }
            Err(error) => {
                last_error = Some(error.to_string());
            }
        }
        std::thread::sleep(interval);
    }

    let details = last_error
        .map(|error| format!(" Last error: {error}"))
        .unwrap_or_default();
    Err(CliError::Timeout(format!(
        "Rhino plugin was not ready within {}s.{}",
        timeout.as_secs(),
        details
    )))
}

fn wait_until_not_running(app: &str, timeout: Duration) -> Result<()> {
    let started = Instant::now();
    let interval = Duration::from_millis(500);

    while started.elapsed() < timeout {
        if !is_app_running(app)? {
            return Ok(());
        }
        std::thread::sleep(interval);
    }

    Err(CliError::Timeout(format!(
        "Timed out waiting for {app} to quit. Close any save/discard prompt and retry."
    )))
}

pub(crate) fn validate_app_name(app: &str) -> Result<()> {
    if app.is_empty()
        || !app
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '.' || c == '-')
    {
        return Err(CliError::InvalidInput(format!(
            "invalid Rhino app name {app:?}; use letters, numbers, spaces, dots, and hyphens"
        )));
    }
    Ok(())
}

fn default_screenshot_path() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    PathBuf::from(format!("rhino-screenshot-{timestamp}.png"))
}

#[cfg(target_os = "macos")]
fn launch_app(app: &str, script: Option<&str>) -> Result<()> {
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

#[cfg(target_os = "macos")]
fn capture_window(
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

#[cfg(not(target_os = "macos"))]
fn capture_window(
    _app: &str,
    _out: &std::path::Path,
    _window_id: Option<u64>,
    _activate: bool,
    _no_shadow: bool,
) -> Result<()> {
    Err(CliError::Other(
        "Rhino window screenshot is currently only supported on macOS.".to_string(),
    ))
}

#[cfg(target_os = "macos")]
fn ensure_screen_capture_access() -> Result<()> {
    let access = core_graphics::access::ScreenCaptureAccess::default();
    if access.preflight() || access.request() {
        return Ok(());
    }

    Err(CliError::Other(
        "macOS Screen Recording permission is required to capture the Rhino window. Grant permission to the terminal running rhino-cli, then retry.".to_string(),
    ))
}

#[cfg(target_os = "macos")]
fn app_window_id(app: &str, activate: bool) -> Result<u64> {
    if activate {
        activate_app(app)?;
        std::thread::sleep(Duration::from_millis(500));
    }

    visible_app_window_id(app)
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
fn owner_matches_app(owner: &str, app: &str) -> bool {
    owner == app
        || (app.starts_with("Rhino") && (owner == "Rhinoceros" || owner.starts_with("Rhino")))
}

#[cfg(not(target_os = "macos"))]
fn launch_app(_app: &str, _script: Option<&str>) -> Result<()> {
    Err(CliError::Other(
        "Rhino launch is currently only supported on macOS.".to_string(),
    ))
}

#[cfg(target_os = "macos")]
fn request_quit(app: &str) -> Result<()> {
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

#[cfg(not(target_os = "macos"))]
fn request_quit(_app: &str) -> Result<()> {
    Err(CliError::Other(
        "Rhino shutdown is currently only supported on macOS.".to_string(),
    ))
}

#[cfg(target_os = "macos")]
fn is_app_running(app: &str) -> Result<bool> {
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

#[cfg(not(target_os = "macos"))]
fn is_app_running(_app: &str) -> Result<bool> {
    Err(CliError::Other(
        "Rhino app state query is currently only supported on macOS.".to_string(),
    ))
}

#[cfg(target_os = "macos")]
fn run_osascript(script: &str) -> std::io::Result<std::process::Output> {
    let mut command = std::process::Command::new("osascript");
    command.args(["-e", script]);
    run_command_with_timeout(
        &mut command,
        Duration::from_secs(MACOS_COMMAND_TIMEOUT_SECS),
    )
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
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

#[cfg(test)]
mod tests {
    use super::{default_screenshot_path, validate_app_name};

    #[test]
    fn app_name_allows_expected_rhino_names() {
        validate_app_name("Rhino 8").unwrap();
        validate_app_name("RhinoWIP").unwrap();
        validate_app_name("Rhino 8.0-Test").unwrap();
    }

    #[test]
    fn app_name_rejects_shell_metacharacters() {
        assert!(validate_app_name("").is_err());
        assert!(validate_app_name("Rhino 8; rm -rf /").is_err());
        assert!(validate_app_name("Rhino \"8\"").is_err());
    }

    #[test]
    fn default_screenshot_path_is_png() {
        let path = default_screenshot_path();
        assert_eq!(path.extension().and_then(|ext| ext.to_str()), Some("png"));
        assert!(path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap()
            .starts_with("rhino-screenshot-"));
    }
}
