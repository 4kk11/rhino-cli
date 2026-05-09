use std::time::Instant;

use serde_json::json;

use crate::commands::CommandContext;
use crate::error::{CliError, Result};

pub fn run(ctx: &CommandContext) -> Result<()> {
    let client = ctx.client();
    let started = Instant::now();
    let result = client.call("system.ping", json!(null))?;

    if result.get("pong").and_then(|value| value.as_bool()) != Some(true) {
        return Err(CliError::InvalidResponse(
            "system.ping result must contain pong=true".to_string(),
        ));
    }

    if ctx.verbose && !ctx.quiet {
        let server = result
            .get("server")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");
        let version = result
            .get("version")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");
        eprintln!(
            "pong from {server} {version} ({}ms)",
            started.elapsed().as_millis()
        );
    }

    Ok(())
}
