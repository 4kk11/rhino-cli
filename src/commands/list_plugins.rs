use serde_json::json;

use crate::commands::{print_json, CommandContext};
use crate::error::{CliError, Result};

pub fn run(ctx: &CommandContext) -> Result<()> {
    let result = ctx.client().call("rpc.list_plugins", json!(null))?;

    if ctx.raw {
        return print_json(&result, ctx.pretty);
    }

    let plugins = result
        .get("plugins")
        .and_then(|value| value.as_array())
        .ok_or_else(|| {
            CliError::InvalidResponse(
                "rpc.list_plugins result must contain plugins[]".to_string(),
            )
        })?;

    for plugin in plugins {
        let id = plugin.get("id").and_then(|value| value.as_str()).ok_or_else(|| {
            CliError::InvalidResponse(
                "rpc.list_plugins entries must contain id (string)".to_string(),
            )
        })?;
        let port = plugin
            .get("port")
            .and_then(|value| value.as_u64())
            .ok_or_else(|| {
                CliError::InvalidResponse(
                    "rpc.list_plugins entries must contain port (number)".to_string(),
                )
            })?;
        println!("{id}\t{port}");
    }

    Ok(())
}
