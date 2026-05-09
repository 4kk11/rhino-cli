use serde_json::json;

use crate::commands::CommandContext;
use crate::error::{CliError, Result};

pub fn run(ctx: &CommandContext) -> Result<()> {
    let result = ctx.client().call("rpc.list_methods", json!(null))?;
    let methods = result
        .get("methods")
        .and_then(|value| value.as_array())
        .or_else(|| result.as_array())
        .ok_or_else(|| {
            CliError::InvalidResponse(
                "rpc.list_methods result must be an array or contain methods[]".to_string(),
            )
        })?;

    for method in methods {
        let method = method.as_str().ok_or_else(|| {
            CliError::InvalidResponse("rpc.list_methods entries must be strings".to_string())
        })?;
        println!("{method}");
    }

    Ok(())
}
