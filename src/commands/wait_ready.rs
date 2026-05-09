use std::thread;
use std::time::{Duration, Instant};

use serde_json::json;

use crate::commands::CommandContext;
use crate::error::{CliError, Result};

pub fn run(ctx: &CommandContext, ready_timeout: Duration) -> Result<()> {
    let started = Instant::now();
    let interval = Duration::from_millis(100);

    loop {
        match ctx.client().call("system.ping", json!(null)) {
            Ok(result) if result.get("pong").and_then(|value| value.as_bool()) == Some(true) => {
                return Ok(())
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
