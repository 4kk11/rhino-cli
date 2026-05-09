use std::process;
use std::time::Duration;

use clap::{Parser, Subcommand};
use rhino_cli::client::{
    DEFAULT_CONNECT_TIMEOUT_SECS, DEFAULT_HOST, DEFAULT_PORT, DEFAULT_TIMEOUT_SECS,
};
use rhino_cli::commands::call::CallArgs;
use rhino_cli::commands::rhino::{LaunchArgs, ShutdownArgs};
use rhino_cli::commands::CommandContext;
use rhino_cli::error::{CliError, Result};
use serde_json::json;

#[derive(Debug, Parser)]
#[command(author, version, about = "JSON-RPC 2.0 client for Rhino plugins")]
struct Cli {
    #[arg(long, env = "RHINO_CLI_PORT", default_value_t = DEFAULT_PORT, global = true)]
    port: u16,

    #[arg(long, env = "RHINO_CLI_HOST", default_value = DEFAULT_HOST, global = true)]
    host: String,

    #[arg(long, env = "RHINO_CLI_TIMEOUT", global = true)]
    timeout: Option<u64>,

    #[arg(long, default_value_t = DEFAULT_CONNECT_TIMEOUT_SECS, global = true)]
    connect_timeout: u64,

    #[arg(long, global = true)]
    pretty: bool,

    #[arg(long, global = true)]
    raw: bool,

    #[arg(short, long, global = true)]
    quiet: bool,

    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Call system.ping.
    Ping,
    /// List registered RPC methods.
    ListMethods,
    /// Call an arbitrary RPC method.
    Call {
        method: String,
        params_json: Option<String>,
        #[arg(long)]
        params_file: Option<std::path::PathBuf>,
        #[arg(long = "param")]
        params: Vec<String>,
    },
    /// Wait until system.ping succeeds.
    WaitReady,
    /// Launch Rhino and wait until the plugin answers system.ping.
    Launch {
        /// Rhino application name.
        #[arg(long, default_value = "Rhino 8")]
        app: String,
        /// Ask Rhino to quit before launching.
        #[arg(long)]
        restart: bool,
        /// Return immediately after launching Rhino.
        #[arg(long)]
        no_wait: bool,
        /// Optional Rhino command script passed via -runscript.
        #[arg(long)]
        script: Option<String>,
    },
    /// Ask Rhino to quit and wait until it exits.
    Shutdown {
        /// Rhino application name.
        #[arg(long, default_value = "Rhino 8")]
        app: String,
    },
}

fn main() {
    let cli = Cli::parse();
    if let Err(error) = run(cli) {
        print_error(&error);
        process::exit(error.exit_code());
    }
}

fn run(cli: Cli) -> Result<()> {
    let timeout_secs = cli.timeout.unwrap_or(DEFAULT_TIMEOUT_SECS);
    let ctx = CommandContext {
        host: cli.host,
        port: cli.port,
        timeout: Duration::from_secs(timeout_secs),
        connect_timeout: Duration::from_secs(cli.connect_timeout),
        pretty: cli.pretty,
        raw: cli.raw,
        quiet: cli.quiet,
        verbose: cli.verbose,
    };

    match cli.command {
        Commands::Ping => rhino_cli::commands::ping::run(&ctx),
        Commands::ListMethods => rhino_cli::commands::list_methods::run(&ctx),
        Commands::Call {
            method,
            params_json,
            params_file,
            params,
        } => rhino_cli::commands::call::run(
            &ctx,
            CallArgs {
                method,
                params_json,
                params_file,
                params,
            },
        ),
        Commands::WaitReady => {
            let ready_timeout = Duration::from_secs(cli.timeout.unwrap_or(30));
            rhino_cli::commands::wait_ready::run(&ctx, ready_timeout)
        }
        Commands::Launch {
            app,
            restart,
            no_wait,
            script,
        } => rhino_cli::commands::rhino::launch(
            &ctx,
            LaunchArgs {
                app,
                timeout: Duration::from_secs(cli.timeout.unwrap_or(120)),
                restart,
                no_wait,
                script,
            },
        ),
        Commands::Shutdown { app } => rhino_cli::commands::rhino::shutdown(
            &ctx,
            ShutdownArgs {
                app,
                timeout: Duration::from_secs(cli.timeout.unwrap_or(30)),
            },
        ),
    }
}

fn print_error(error: &CliError) {
    match error {
        CliError::RpcError(rpc_error) => {
            eprintln!(
                "{}",
                serde_json::to_string(&json!({ "error": rpc_error })).unwrap()
            );
        }
        _ => eprintln!("{error}"),
    }
}
