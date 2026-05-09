use std::fs;
use std::path::PathBuf;

use serde_json::{json, Map, Value};

use crate::commands::{print_json, CommandContext};
use crate::error::{CliError, Result};

#[derive(Clone, Debug)]
pub struct CallArgs {
    pub method: String,
    pub params_json: Option<String>,
    pub params_file: Option<PathBuf>,
    pub params: Vec<String>,
}

pub fn run(ctx: &CommandContext, args: CallArgs) -> Result<()> {
    let params = build_params(&args)?;
    let response = ctx.client().call_response(&args.method, params)?;

    if let Some(error) = response.error {
        return Err(CliError::RpcError(error));
    }

    if ctx.raw {
        print_json(&response.to_value()?, ctx.pretty)?;
        return Ok(());
    }

    print_json(
        response
            .result
            .as_ref()
            .ok_or_else(|| CliError::InvalidResponse("response is missing result".to_string()))?,
        ctx.pretty,
    )
}

fn build_params(args: &CallArgs) -> Result<Value> {
    if let Some(path) = &args.params_file {
        let content = fs::read_to_string(path).map_err(|error| {
            CliError::InvalidInput(format!(
                "failed to read params file {}: {error}",
                path.display()
            ))
        })?;
        return serde_json::from_str(&content).map_err(|error| {
            CliError::InvalidInput(format!(
                "failed to parse params file {}: {error}",
                path.display()
            ))
        });
    }

    if !args.params.is_empty() {
        let mut object = Map::new();
        for pair in &args.params {
            let (key, value) = pair.split_once('=').ok_or_else(|| {
                CliError::InvalidInput(format!("--param must be key=value, got {pair:?}"))
            })?;
            object.insert(key.to_string(), parse_param_value(value));
        }
        return Ok(Value::Object(object));
    }

    if let Some(params_json) = &args.params_json {
        return serde_json::from_str(params_json).map_err(|error| {
            CliError::InvalidInput(format!("failed to parse params JSON: {error}"))
        });
    }

    Ok(json!(null))
}

fn parse_param_value(value: &str) -> Value {
    serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.to_string()))
}
