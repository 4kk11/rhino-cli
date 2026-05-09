use std::path::PathBuf;

use serde_json::json;

use crate::commands::CommandContext;
use crate::error::{CliError, Result};

pub fn set_port(ctx: &CommandContext, port: u16) -> Result<()> {
    if port == 0 {
        return Err(CliError::InvalidInput(
            "port must be in 1-65535".to_string(),
        ));
    }

    let path = config_path()?;
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
    })?;

    if !ctx.quiet {
        println!(
            "RhinoCliPlugin port set to {port}. Restart Rhino to apply ({})",
            path.display()
        );
    }
    Ok(())
}

pub fn show_config(ctx: &CommandContext) -> Result<()> {
    let path = config_path()?;
    if !path.exists() {
        if !ctx.quiet {
            println!("no RhinoCliPlugin config at {}", path.display());
            println!("plugin falls back to RHINO_CLI_PORT env var or the default port");
        }
        return Ok(());
    }

    let content = std::fs::read_to_string(&path).map_err(|error| {
        CliError::Other(format!(
            "failed to read RhinoCliPlugin config {}: {error}",
            path.display()
        ))
    })?;

    if !ctx.quiet {
        println!("{}", path.display());
        print!("{content}");
        if !content.ends_with('\n') {
            println!();
        }
    }
    Ok(())
}

fn config_path() -> Result<PathBuf> {
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
