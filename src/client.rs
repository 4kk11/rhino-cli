use std::io;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::Value;

use crate::error::{CliError, Result};
use crate::protocol::{Id, Request, Response};

pub const DEFAULT_HOST: &str = "127.0.0.1";
pub const DEFAULT_PORT: u16 = 50061;
pub const DEFAULT_TIMEOUT_SECS: u64 = 60;
pub const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 5;

#[derive(Debug)]
pub struct Client {
    host: String,
    port: u16,
    connect_timeout: Duration,
    read_timeout: Duration,
    next_id: AtomicU64,
    verbose: bool,
}

impl Client {
    pub fn new(
        host: impl Into<String>,
        port: u16,
        connect_timeout: Duration,
        read_timeout: Duration,
    ) -> Self {
        Self {
            host: host.into(),
            port,
            connect_timeout,
            read_timeout,
            next_id: AtomicU64::new(1),
            verbose: std::env::var("RHINO_CLI_DEBUG").is_ok_and(|value| value == "1"),
        }
    }

    pub fn from_env() -> Result<Self> {
        let host = std::env::var("RHINO_CLI_HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string());
        let port = read_env_u16("RHINO_CLI_PORT", DEFAULT_PORT)?;
        let timeout_secs = read_env_u64("RHINO_CLI_TIMEOUT", DEFAULT_TIMEOUT_SECS)?;

        Ok(Self::new(
            host,
            port,
            Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS),
            Duration::from_secs(timeout_secs),
        ))
    }

    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    pub fn call(&self, method: &str, params: Value) -> Result<Value> {
        let response = self.call_response(method, params)?;

        if let Some(error) = response.error {
            return Err(CliError::RpcError(error));
        }

        response
            .result
            .ok_or_else(|| CliError::InvalidResponse("response is missing result".to_string()))
    }

    pub fn call_response(&self, method: &str, params: Value) -> Result<Response> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let request = Request::new(id, method, Some(params));
        let request_line = request.to_json_line()?;

        if self.verbose {
            eprint!("--> {request_line}");
        }

        let addr = (self.host.as_str(), self.port)
            .to_socket_addrs()
            .map_err(|error| CliError::Connect(error.to_string()))?
            .next()
            .ok_or_else(|| {
                CliError::Connect(format!("could not resolve {}:{}", self.host, self.port))
            })?;

        let mut stream = TcpStream::connect_timeout(&addr, self.connect_timeout)?;
        stream.set_read_timeout(Some(self.read_timeout))?;
        stream.set_write_timeout(Some(self.read_timeout))?;
        stream.write_all(request_line.as_bytes())?;
        stream.flush()?;

        let mut response_line = String::new();
        let bytes = match BufReader::new(stream).read_line(&mut response_line) {
            Ok(bytes) => bytes,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) =>
            {
                return Err(CliError::Timeout(error.to_string()));
            }
            Err(error) => {
                return Err(CliError::Parse(format!("failed to read response: {error}")));
            }
        };
        if bytes == 0 {
            return Err(CliError::Parse(
                "connection closed before response".to_string(),
            ));
        }

        if self.verbose {
            eprint!("<-- {response_line}");
        }

        let response = Response::from_json_line(&response_line)?;
        response.validate_id(&Id::Number(id))?;

        Ok(response)
    }
}

fn read_env_u16(name: &str, default: u16) -> Result<u16> {
    match std::env::var(name) {
        Ok(value) => value
            .parse()
            .map_err(|_| CliError::InvalidInput(format!("{name} must be a u16"))),
        Err(_) => Ok(default),
    }
}

fn read_env_u64(name: &str, default: u64) -> Result<u64> {
    match std::env::var(name) {
        Ok(value) => value
            .parse()
            .map_err(|_| CliError::InvalidInput(format!("{name} must be a positive integer"))),
        Err(_) => Ok(default),
    }
}
