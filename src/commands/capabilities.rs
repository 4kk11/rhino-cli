use clap::ValueEnum;
use serde_json::{json, Value};

use crate::commands::{print_json, CommandContext};
use crate::error::{CliError, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum CapabilitiesFormat {
    Text,
    Json,
    Markdown,
    Agent,
}

#[derive(Clone, Debug)]
pub struct CapabilitiesArgs {
    pub method: Option<String>,
    pub format: CapabilitiesFormat,
}

pub fn run(ctx: &CommandContext, args: CapabilitiesArgs) -> Result<()> {
    let params = args
        .method
        .as_ref()
        .map(|method| json!({ "method": method }))
        .unwrap_or_else(|| json!(null));
    let result = ctx.client().call("rpc.capabilities", params)?;

    if args.format == CapabilitiesFormat::Json || ctx.raw {
        print_json(&result, ctx.pretty)?;
        return Ok(());
    }

    match args.format {
        CapabilitiesFormat::Text => print_text(ctx, &result, args.method.as_deref()),
        CapabilitiesFormat::Markdown => print_markdown(ctx, &result, args.method.as_deref()),
        CapabilitiesFormat::Agent => print_agent(ctx, &result),
        CapabilitiesFormat::Json => unreachable!(),
    }
}

fn print_text(ctx: &CommandContext, result: &Value, requested_method: Option<&str>) -> Result<()> {
    println!("Capabilities from {}", server_title(result));
    println!("Connection: {}:{}", ctx.host, ctx.port);
    println!();

    if requested_method.is_some() {
        print_method_text(method_value(result)?, true)?;
        return Ok(());
    }

    for method in methods_value(result)? {
        print_method_text(method, false)?;
    }

    Ok(())
}

fn print_markdown(
    ctx: &CommandContext,
    result: &Value,
    requested_method: Option<&str>,
) -> Result<()> {
    println!("# {}", server_title(result));
    println!();
    println!("Connection: `{}:{}`", ctx.host, ctx.port);
    println!();

    if requested_method.is_some() {
        print_method_markdown(method_value(result)?)?;
        return Ok(());
    }

    for method in methods_value(result)? {
        print_method_markdown(method)?;
    }

    Ok(())
}

fn print_agent(ctx: &CommandContext, result: &Value) -> Result<()> {
    println!("# rhino-cli Agent Context");
    println!();
    println!("Connection: `{}:{}`", ctx.host, ctx.port);
    println!("Server: `{}`", server_title(result));
    println!();
    println!("Core workflow:");
    println!(
        "- Check environment: `rhino-cli doctor --port {}`",
        ctx.port
    );
    println!(
        "- Open Rhino to a modeling window: `rhino-cli launch` then `rhino-cli wait-ready --port {} --timeout 120`",
        ctx.port
    );
    println!(
        "- Inspect capabilities: `rhino-cli capabilities --port {}`",
        ctx.port
    );
    println!(
        "- Execute arbitrary handlers: `rhino-cli call <method> '<json>' --port {}`",
        ctx.port
    );
    println!("- Prefer dedicated commands when a handler lists one.");
    println!();
    println!("Available handlers:");
    println!();

    for method in methods_value(result)? {
        print_method_markdown(method)?;
    }

    Ok(())
}

fn print_method_text(method: &Value, detailed: bool) -> Result<()> {
    println!("{}", field(method, "method"));
    if !field(method, "description").is_empty() {
        println!("  {}", field(method, "description"));
    }
    if detailed || !field(method, "dedicatedCommand").is_empty() {
        println!("  Params: {}", field(method, "paramsSchema"));
        println!("  Result: {}", field(method, "resultSchema"));
    }
    if !field(method, "dedicatedCommand").is_empty() {
        println!("  CLI: {}", field(method, "dedicatedCommand"));
    }
    if !field(method, "sideEffects").is_empty() {
        println!("  Side effects: {}", field(method, "sideEffects"));
    }
    for example in examples(method) {
        println!("  Example: {example}");
    }
    println!();
    Ok(())
}

fn print_method_markdown(method: &Value) -> Result<()> {
    println!("## `{}`", field(method, "method"));
    println!();
    if !field(method, "description").is_empty() {
        println!("{}", field(method, "description"));
        println!();
    }
    println!("- Params: `{}`", field(method, "paramsSchema"));
    println!("- Result: `{}`", field(method, "resultSchema"));
    if !field(method, "dedicatedCommand").is_empty() {
        println!("- Dedicated CLI: `{}`", field(method, "dedicatedCommand"));
    }
    if !field(method, "sideEffects").is_empty() {
        println!("- Side effects: {}", field(method, "sideEffects"));
    }
    let examples = examples(method);
    if !examples.is_empty() {
        println!();
        println!("Examples:");
        for example in examples {
            println!("- `{example}`");
        }
    }
    println!();
    Ok(())
}

fn methods_value(result: &Value) -> Result<&Vec<Value>> {
    result
        .get("methods")
        .and_then(|value| value.as_array())
        .ok_or_else(|| {
            CliError::InvalidResponse("rpc.capabilities result must contain methods[]".to_string())
        })
}

fn method_value(result: &Value) -> Result<&Value> {
    result.get("method").ok_or_else(|| {
        CliError::InvalidResponse("rpc.capabilities result must contain method".to_string())
    })
}

fn server_title(result: &Value) -> String {
    let server = result.get("server");
    let plugin = server
        .and_then(|value| value.get("pluginId").or_else(|| value.get("plugin_id")))
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let version = server
        .and_then(|value| {
            value
                .get("serverVersion")
                .or_else(|| value.get("server_version"))
        })
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    format!("{plugin} {version}")
}

fn field<'a>(method: &'a Value, name: &str) -> &'a str {
    method
        .get(name)
        .and_then(|value| value.as_str())
        .unwrap_or("")
}

fn examples(method: &Value) -> Vec<&str> {
    method
        .get("examples")
        .and_then(|value| value.as_array())
        .map(|values| values.iter().filter_map(|value| value.as_str()).collect())
        .unwrap_or_default()
}
