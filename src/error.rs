use std::fmt;
use std::io;

use crate::protocol::RpcError;

#[derive(Debug)]
pub enum CliError {
    Connect(String),
    Timeout(String),
    RpcError(RpcError),
    Parse(String),
    InvalidResponse(String),
    InvalidInput(String),
    Other(String),
}

impl CliError {
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::Connect(_) => 2,
            CliError::RpcError(_) => 3,
            CliError::Timeout(_) => 4,
            CliError::Parse(_) | CliError::InvalidResponse(_) => 5,
            CliError::InvalidInput(_) | CliError::Other(_) => 1,
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::Connect(message) => write!(f, "connect error: {message}"),
            CliError::Timeout(message) => write!(f, "timeout: {message}"),
            CliError::RpcError(error) => write!(f, "rpc error {}: {}", error.code, error.message),
            CliError::Parse(message) => write!(f, "parse error: {message}"),
            CliError::InvalidResponse(message) => write!(f, "invalid response: {message}"),
            CliError::InvalidInput(message) => write!(f, "invalid input: {message}"),
            CliError::Other(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<io::Error> for CliError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::AddrNotAvailable
            | io::ErrorKind::AddrInUse
            | io::ErrorKind::NotConnected => CliError::Connect(error.to_string()),
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => {
                CliError::Timeout(error.to_string())
            }
            _ => CliError::Other(error.to_string()),
        }
    }
}

impl From<serde_json::Error> for CliError {
    fn from(error: serde_json::Error) -> Self {
        CliError::Parse(error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, CliError>;
