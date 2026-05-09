use std::time::{Duration, Instant};

use serde_json::json;

use crate::commands::CommandContext;
use crate::error::{CliError, Result};

const DEFAULT_APP: &str = "Rhino 8";
const DEFAULT_LAUNCH_TIMEOUT_SECS: u64 = 120;
const DEFAULT_SHUTDOWN_TIMEOUT_SECS: u64 = 30;

#[derive(Clone, Debug)]
pub struct LaunchArgs {
    pub app: String,
    pub timeout: Duration,
    pub restart: bool,
    pub no_wait: bool,
    pub script: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ShutdownArgs {
    pub app: String,
    pub timeout: Duration,
}

impl Default for LaunchArgs {
    fn default() -> Self {
        Self {
            app: DEFAULT_APP.to_string(),
            timeout: Duration::from_secs(DEFAULT_LAUNCH_TIMEOUT_SECS),
            restart: false,
            no_wait: false,
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

pub fn launch(ctx: &CommandContext, args: LaunchArgs) -> Result<()> {
    validate_app_name(&args.app)?;

    if !args.restart && is_plugin_ready(ctx) {
        if ctx.verbose && !ctx.quiet {
            eprintln!("Plugin already ready on {}:{}", ctx.host, ctx.port);
        }
        return Ok(());
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

    launch_app(&args.app, args.script.as_deref())?;

    if args.no_wait {
        return Ok(());
    }

    wait_until_ready(ctx, args.timeout)
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
    std::process::Command::new("osascript")
        .args(["-e", script])
        .output()
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
    use super::validate_app_name;

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
}
