pub mod call;
pub mod capabilities;
pub mod doctor;
pub mod document_state;
pub mod list_methods;
pub mod list_plugins;
pub mod ping;
pub mod plugin;
pub mod rhino;
pub mod rhino_rpc;
pub mod wait_ready;

use std::time::Duration;

use serde_json::Value;

use crate::client::Client;
use crate::error::Result;

#[derive(Clone, Debug)]
pub struct CommandContext {
    pub host: String,
    pub port: u16,
    pub timeout: Duration,
    pub connect_timeout: Duration,
    pub pretty: bool,
    pub raw: bool,
    pub quiet: bool,
    pub verbose: bool,
}

impl CommandContext {
    pub fn client(&self) -> Client {
        Client::new(
            self.host.clone(),
            self.port,
            self.connect_timeout,
            self.timeout,
        )
        .with_verbose(self.verbose && !self.quiet)
    }
}

pub fn print_json(value: &Value, pretty: bool) -> Result<()> {
    if pretty {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        println!("{}", serde_json::to_string(value)?);
    }
    Ok(())
}
