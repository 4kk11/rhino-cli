use std::path::PathBuf;

use serde_json::{json, Value};

use crate::commands::{print_json, CommandContext};
use crate::error::{CliError, Result};

#[derive(Clone, Debug)]
pub struct RunScriptArgs {
    pub script: String,
    pub echo: bool,
    pub mru_display_string: Option<String>,
    pub fail_on_false: bool,
}

#[derive(Clone, Debug)]
pub struct HistoryArgs {
    pub tail: Option<u32>,
    pub clear: bool,
    pub json: bool,
}

#[derive(Clone, Debug)]
pub struct NewModelArgs {
    pub template: Option<PathBuf>,
}

pub fn run_script(ctx: &CommandContext, args: RunScriptArgs) -> Result<()> {
    let mut params = json!({
        "script": args.script,
        "echo": args.echo
    });
    if let Some(mru_display_string) = args.mru_display_string {
        params["mru_display_string"] = json!(mru_display_string);
    }

    let result = ctx.client().call("rhino.run_script", params)?;
    print_json(&result, ctx.pretty)?;

    if args.fail_on_false && result.get("success").and_then(Value::as_bool) == Some(false) {
        return Err(CliError::Other(
            "Rhino script returned success=false".to_string(),
        ));
    }

    Ok(())
}

pub fn history(ctx: &CommandContext, args: HistoryArgs) -> Result<()> {
    if args.clear {
        let result = ctx
            .client()
            .call("rhino.clear_command_history", json!(null))?;
        if args.json || ctx.pretty {
            print_json(&result, ctx.pretty)?;
        }
        return Ok(());
    }

    let params = match args.tail {
        Some(tail) => json!({ "tail": tail }),
        None => json!(null),
    };
    let result = ctx.client().call("rhino.command_history", params)?;

    if args.json || ctx.pretty {
        print_json(&result, ctx.pretty)?;
        return Ok(());
    }

    let text = result
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| CliError::InvalidResponse("history response is missing text".to_string()))?;

    print!("{text}");
    if !text.is_empty() && !text.ends_with('\n') {
        println!();
    }
    Ok(())
}

pub fn new_model(ctx: &CommandContext, args: NewModelArgs) -> Result<()> {
    let params = match args.template {
        Some(template) => json!({ "template": template.to_string_lossy() }),
        None => json!(null),
    };
    let result = ctx.client().call("rhino.new_model", params)?;
    print_json(&result, ctx.pretty)?;
    Ok(())
}
