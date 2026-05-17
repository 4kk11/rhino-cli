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

#[derive(Clone, Debug)]
pub struct ListCommandsArgs {
    pub pattern: Option<String>,
    pub include_unloaded: bool,
}

#[derive(Clone, Debug)]
pub struct ProbeCommandArgs {
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct InspectTypeArgs {
    pub name: String,
    pub binding: Option<String>,
    pub include_inherited: bool,
}

#[derive(Clone, Debug)]
pub struct SearchTypesArgs {
    pub pattern: String,
    pub scope: Option<String>,
    pub assembly: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct DecompileMethodArgs {
    pub type_name: String,
    pub method: String,
    pub signature: Option<String>,
}

#[derive(Clone, Debug)]
pub struct InspectTypeWithBodyArgs {
    pub inspect: InspectTypeArgs,
    pub with_body: Vec<String>,
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

pub fn list_commands(ctx: &CommandContext, args: ListCommandsArgs) -> Result<()> {
    let mut params = json!({
        "include_unloaded": args.include_unloaded
    });
    if let Some(pattern) = args.pattern {
        params["pattern"] = json!(pattern);
    }
    let result = ctx.client().call("rhino.list_commands", params)?;
    print_json(&result, ctx.pretty)?;
    Ok(())
}

pub fn probe_command(ctx: &CommandContext, args: ProbeCommandArgs) -> Result<()> {
    let params = json!({ "name": args.name });
    let result = ctx.client().call("rhino.probe_command", params)?;
    print_json(&result, ctx.pretty)?;
    Ok(())
}

pub fn inspect_type(ctx: &CommandContext, args: InspectTypeArgs) -> Result<()> {
    let mut params = json!({
        "name": args.name,
        "include_inherited": args.include_inherited,
    });
    if let Some(binding) = args.binding {
        params["binding"] = json!(binding);
    }
    let result = ctx.client().call("rhino.inspect_type", params)?;
    print_json(&result, ctx.pretty)?;
    Ok(())
}

pub fn search_types(ctx: &CommandContext, args: SearchTypesArgs) -> Result<()> {
    let mut params = json!({ "pattern": args.pattern });
    if let Some(scope) = args.scope {
        params["scope"] = json!(scope);
    }
    if let Some(assembly) = args.assembly {
        params["assembly"] = json!(assembly);
    }
    if let Some(limit) = args.limit {
        params["limit"] = json!(limit);
    }
    let result = ctx.client().call("rhino.search_types", params)?;
    print_json(&result, ctx.pretty)?;
    Ok(())
}

pub fn decompile_method(ctx: &CommandContext, args: DecompileMethodArgs) -> Result<()> {
    let mut params = json!({
        "type": args.type_name,
        "method": args.method,
    });
    if let Some(signature) = args.signature {
        params["signature"] = json!(signature);
    }
    let result = ctx.client().call("rhino.decompile_method", params)?;
    print_json(&result, ctx.pretty)?;
    Ok(())
}

pub fn inspect_type_with_body(ctx: &CommandContext, args: InspectTypeWithBodyArgs) -> Result<()> {
    let mut inspect_params = json!({
        "name": args.inspect.name,
        "include_inherited": args.inspect.include_inherited,
    });
    if let Some(binding) = args.inspect.binding {
        inspect_params["binding"] = json!(binding);
    }
    let client = ctx.client();
    let mut inspect_result = client.call("rhino.inspect_type", inspect_params)?;

    if let Some(methods) = inspect_result
        .get_mut("methods")
        .and_then(Value::as_array_mut)
    {
        for target_name in &args.with_body {
            // Collect overload count for this method group.
            let group_idx = methods
                .iter()
                .position(|m| m.get("name").and_then(Value::as_str) == Some(target_name.as_str()));
            let Some(idx) = group_idx else { continue };
            let overload_count = methods[idx]
                .get("overloads")
                .and_then(Value::as_array)
                .map(|a| a.len())
                .unwrap_or(0);

            for ov in 0..overload_count {
                // Build signature filter from overload params types.
                let sig = methods[idx]["overloads"][ov]
                    .get("params")
                    .and_then(Value::as_array)
                    .map(|ps| {
                        ps.iter()
                            .filter_map(|p| p.get("type").and_then(Value::as_str))
                            .collect::<Vec<_>>()
                            .join(",")
                    })
                    .unwrap_or_default();

                let mut params = json!({
                    "type": args.inspect.name,
                    "method": target_name,
                });
                if !sig.is_empty() {
                    params["signature"] = json!(sig);
                }
                match client.call("rhino.decompile_method", params) {
                    Ok(body) => {
                        if let Some(csharp) = body.get("csharp").and_then(Value::as_str) {
                            methods[idx]["overloads"][ov]["body"] = json!(csharp);
                        }
                    }
                    Err(err) => {
                        methods[idx]["overloads"][ov]["body_error"] = json!(err.to_string());
                    }
                }
            }
        }
    }

    print_json(&inspect_result, ctx.pretty)?;
    Ok(())
}
