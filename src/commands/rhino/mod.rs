mod platform;

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::commands::CommandContext;
use crate::error::{CliError, Result};

const DEFAULT_APP: &str = "Rhino 8";
const DEFAULT_SHUTDOWN_TIMEOUT_SECS: u64 = 30;

#[derive(Clone, Debug)]
pub struct LaunchArgs {
    pub app: String,
    pub restart: bool,
    pub no_new_model: bool,
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
            restart: false,
            no_new_model: false,
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

    let app_running = platform::is_app_running(&args.app)?;

    if !args.restart && app_running {
        if args.script.is_some() {
            return Err(CliError::InvalidInput(
                "Rhino is already running; use `rhino-cli launch --restart --script ...` to apply launch-time scripts, or `rhino-cli run-script` inside an existing modeling session."
                    .to_string(),
            ));
        }
        if ctx.verbose && !ctx.quiet {
            eprintln!("{} is already running", args.app);
        }
        return Ok(());
    }

    if args.restart && app_running {
        shutdown(
            ctx,
            ShutdownArgs {
                app: args.app.clone(),
                timeout: Duration::from_secs(DEFAULT_SHUTDOWN_TIMEOUT_SECS),
            },
        )?;
    }

    let startup_script = args
        .script
        .as_deref()
        .or_else(|| (!args.no_new_model).then_some("_NoEcho"));
    platform::launch_app(&args.app, startup_script)
}

pub fn shutdown(ctx: &CommandContext, args: ShutdownArgs) -> Result<()> {
    validate_app_name(&args.app)?;

    if !platform::is_app_running(&args.app)? {
        if ctx.verbose && !ctx.quiet {
            eprintln!("{} is not running", args.app);
        }
        return Ok(());
    }

    platform::request_quit(&args.app)?;
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

    platform::capture_window(
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
    platform::is_app_running(app)
}

fn wait_until_not_running(app: &str, timeout: Duration) -> Result<()> {
    let started = Instant::now();
    let interval = Duration::from_millis(500);

    while started.elapsed() < timeout {
        if !platform::is_app_running(app)? {
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
