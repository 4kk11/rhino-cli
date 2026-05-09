use std::process;
use std::time::Duration;

use clap::{Parser, Subcommand};
use rhino_cli::client::{
    DEFAULT_CONNECT_TIMEOUT_SECS, DEFAULT_HOST, DEFAULT_PORT, DEFAULT_TIMEOUT_SECS,
};
use rhino_cli::commands::call::CallArgs;
use rhino_cli::commands::capabilities::{CapabilitiesArgs, CapabilitiesFormat};
use rhino_cli::commands::doctor::DoctorArgs;
use rhino_cli::commands::rhino::{LaunchArgs, ScreenshotArgs, ShutdownArgs};
use rhino_cli::commands::rhino_rpc::{HistoryArgs, NewModelArgs, RunScriptArgs};
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
    /// Diagnose Rhino and RhinoCliPlugin connectivity.
    Doctor {
        /// Rhino application name.
        #[arg(long, default_value = "Rhino 8")]
        app: String,
    },
    /// Show registered handler capabilities and call metadata.
    Capabilities {
        /// Show one method in detail.
        #[arg(long)]
        method: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value = "text")]
        format: CapabilitiesFormat,
    },
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
    /// Run a Rhino command script through the plugin.
    RunScript {
        /// Rhino command script, for example "_Zoom _Extents".
        script: String,
        /// Echo the script in Rhino's command UI.
        #[arg(long)]
        echo: bool,
        /// Optional MRU display string for RhinoApp.RunScript.
        #[arg(long)]
        mru: Option<String>,
        /// Exit non-zero when RhinoApp.RunScript returns false.
        #[arg(long)]
        fail_on_false: bool,
    },
    /// Print or clear Rhino command history.
    History {
        /// Print only the last N history lines.
        #[arg(long)]
        tail: Option<u32>,
        /// Clear Rhino command history instead of printing it.
        #[arg(long)]
        clear: bool,
        /// Print the full JSON result.
        #[arg(long)]
        json: bool,
    },
    /// Open a new Rhino model through the plugin.
    NewModel {
        /// Optional 3DM template path.
        #[arg(long)]
        template: Option<std::path::PathBuf>,
    },
    /// Launch Rhino. Use `rhino-cli wait-ready --port <PORT>` afterwards to wait for the plugin.
    Launch {
        /// Rhino application name.
        #[arg(long, default_value = "Rhino 8")]
        app: String,
        /// Ask Rhino to quit before launching.
        #[arg(long)]
        restart: bool,
        /// Open a new model at startup instead of leaving Rhino's start window active.
        #[arg(long)]
        new_model: bool,
        /// Optional Rhino command script passed via -runscript.
        #[arg(long)]
        script: Option<String>,
    },
    /// Manage the bundled RhinoCliPlugin's launch config.
    Plugin {
        #[command(subcommand)]
        command: PluginCommand,
    },
    /// Ask Rhino to quit and wait until it exits.
    Shutdown {
        /// Rhino application name.
        #[arg(long, default_value = "Rhino 8")]
        app: String,
    },
    /// Capture the Rhino window as a PNG screenshot.
    Screenshot {
        /// Rhino application name.
        #[arg(long, default_value = "Rhino 8")]
        app: String,
        /// Output PNG path. Defaults to rhino-screenshot-<unix>.png.
        #[arg(long)]
        out: Option<std::path::PathBuf>,
        /// Do not activate Rhino before capturing the front window.
        #[arg(long)]
        no_activate: bool,
        /// Capture a specific macOS window id instead of Rhino's front window.
        #[arg(long)]
        window_id: Option<u64>,
        /// Capture without the macOS window shadow.
        #[arg(long)]
        no_shadow: bool,
    },
}

#[derive(Debug, Subcommand)]
enum PluginCommand {
    /// Set the port the bundled RhinoCliPlugin listens on at startup.
    SetPort {
        /// TCP port (1-65535).
        port: u16,
    },
    /// Print the current bundled RhinoCliPlugin launch config.
    ShowConfig,
}

fn main() {
    let cli = Cli::parse();
    let host = cli.host.clone();
    let port = cli.port;
    if let Err(error) = run(cli) {
        print_error(&error, &host, port);
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
        Commands::Doctor { app } => rhino_cli::commands::doctor::run(&ctx, DoctorArgs { app }),
        Commands::Capabilities { method, format } => {
            rhino_cli::commands::capabilities::run(&ctx, CapabilitiesArgs { method, format })
        }
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
        Commands::RunScript {
            script,
            echo,
            mru,
            fail_on_false,
        } => rhino_cli::commands::rhino_rpc::run_script(
            &ctx,
            RunScriptArgs {
                script,
                echo,
                mru_display_string: mru,
                fail_on_false,
            },
        ),
        Commands::History { tail, clear, json } => {
            rhino_cli::commands::rhino_rpc::history(&ctx, HistoryArgs { tail, clear, json })
        }
        Commands::NewModel { template } => {
            rhino_cli::commands::rhino_rpc::new_model(&ctx, NewModelArgs { template })
        }
        Commands::Launch {
            app,
            restart,
            new_model,
            script,
        } => rhino_cli::commands::rhino::launch(
            &ctx,
            LaunchArgs {
                app,
                restart,
                new_model,
                script,
            },
        ),
        Commands::Plugin { command } => match command {
            PluginCommand::SetPort { port } => rhino_cli::commands::plugin::set_port(&ctx, port),
            PluginCommand::ShowConfig => rhino_cli::commands::plugin::show_config(&ctx),
        },
        Commands::Shutdown { app } => rhino_cli::commands::rhino::shutdown(
            &ctx,
            ShutdownArgs {
                app,
                timeout: Duration::from_secs(cli.timeout.unwrap_or(30)),
            },
        ),
        Commands::Screenshot {
            app,
            out,
            no_activate,
            window_id,
            no_shadow,
        } => rhino_cli::commands::rhino::screenshot(
            &ctx,
            ScreenshotArgs {
                app,
                out,
                no_activate,
                window_id,
                no_shadow,
            },
        ),
    }
}

fn print_error(error: &CliError, host: &str, port: u16) {
    match error {
        CliError::Connect(message) => {
            eprintln!("connect error: {message}");
            eprintln!();
            eprintln!(
                "Rhino is not reachable at {host}:{port}. Start Rhino and RhinoCliPlugin, then retry:"
            );
            eprintln!("  rhino-cli plugin set-port {port}");
            eprintln!("  rhino-cli launch --new-model");
            eprintln!("  rhino-cli wait-ready --port {port} --timeout 120");
            eprintln!();
            eprintln!(
                "If Rhino is already running, verify that RhinoCliPlugin is installed and listening on this port."
            );
        }
        CliError::RpcError(rpc_error) => {
            eprintln!(
                "{}",
                serde_json::to_string(&json!({ "error": rpc_error })).unwrap()
            );
        }
        _ => eprintln!("{error}"),
    }
}
