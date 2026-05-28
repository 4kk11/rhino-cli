use std::thread;
use std::time::{Duration, Instant};

use serde_json::json;

use crate::commands::document_state::{self, ACTIVE_DOC_MISSING_WARNING};
use crate::commands::CommandContext;
use crate::error::{CliError, Result};

pub fn run(ctx: &CommandContext, ready_timeout: Duration) -> Result<()> {
    let started = Instant::now();
    let interval = Duration::from_millis(100);

    loop {
        match ctx.client().call("system.ping", json!(null)) {
            Ok(result) if result.get("pong").and_then(|value| value.as_bool()) == Some(true) => {
                warn_if_active_doc_missing(ctx);
                return Ok(());
            }
            _ if started.elapsed() >= ready_timeout => {
                return Err(CliError::Timeout(format!(
                    "server was not ready within {}s",
                    ready_timeout.as_secs()
                )))
            }
            _ => thread::sleep(interval),
        }
    }
}

fn warn_if_active_doc_missing(ctx: &CommandContext) {
    if ctx.quiet {
        return;
    }
    let client = ctx.client();
    if let Ok(Some(state)) = document_state::probe(&client) {
        if state.active_doc_missing() {
            eprintln!("{ACTIVE_DOC_MISSING_WARNING}");
        }
    }
}
