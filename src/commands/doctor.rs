use serde_json::json;

use crate::commands::{rhino, CommandContext};
use crate::error::{CliError, Result};

#[derive(Clone, Debug)]
pub struct DoctorArgs {
    pub app: String,
}

pub fn run(ctx: &CommandContext, args: DoctorArgs) -> Result<()> {
    let app_status = match rhino::app_running(&args.app) {
        Ok(true) => format!("running ({})", args.app),
        Ok(false) => format!("not running ({})", args.app),
        Err(error) => format!("unknown ({error})"),
    };

    println!("Rhino app: {app_status}");

    match ctx.client().call("system.ping", json!(null)) {
        Ok(result) => {
            let server = result
                .get("server")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown");
            let version = result
                .get("version")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown");
            println!("RhinoCliPlugin RPC: reachable at {}:{}", ctx.host, ctx.port);
            println!("Server: {server} {version}");
            println!();
            println!("Next:");
            println!("  rhino-cli capabilities --port {}", ctx.port);
            Ok(())
        }
        Err(error) => {
            println!(
                "RhinoCliPlugin RPC: not reachable at {}:{}",
                ctx.host, ctx.port
            );
            println!("Reason: {error}");
            println!();
            println!("Suggested:");
            println!(
                "  rhino-cli launch --new-model --port {} --timeout 120",
                ctx.port
            );
            println!("  dotnet build plugin/RhinoCliPlugin/RhinoCliPlugin.csproj");
            println!();
            println!("If Rhino is already running, RhinoCliPlugin may not be installed, loaded, or listening on this port.");
            match error {
                CliError::Connect(_) | CliError::Timeout(_) => Ok(()),
                other => Err(other),
            }
        }
    }
}
