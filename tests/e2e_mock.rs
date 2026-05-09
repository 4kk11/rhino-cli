use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::process::{Child, Command as StdCommand, Stdio};
use std::sync::Once;
use std::time::Duration;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;

struct TestRunner {
    child: Child,
    port: u16,
}

static BUILD_TEST_RUNNER: Once = Once::new();

impl TestRunner {
    fn start() -> Self {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        BUILD_TEST_RUNNER.call_once(|| {
            let status = StdCommand::new("dotnet")
                .args([
                    "build",
                    "server/RhinoCli.TestRunner/RhinoCli.TestRunner.csproj",
                ])
                .current_dir(manifest_dir)
                .status()
                .unwrap();
            assert!(status.success(), "failed to build RhinoCli.TestRunner");
        });

        let mut child = StdCommand::new("dotnet")
            .args([
                "run",
                "--no-build",
                "--project",
                "server/RhinoCli.TestRunner/RhinoCli.TestRunner.csproj",
                "--",
                "--port",
                "0",
            ])
            .current_dir(manifest_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        let stdout = child.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        let port = loop {
            line.clear();
            let bytes = reader.read_line(&mut line).unwrap();
            assert!(bytes > 0, "test runner exited before READY");
            if let Some(port) = line.strip_prefix("READY ") {
                break port.trim().parse::<u16>().unwrap();
            }
        };

        Self { child, port }
    }
}

impl Drop for TestRunner {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn bin() -> Command {
    Command::cargo_bin("rhino-cli").unwrap()
}

#[test]
fn cli_can_ping_csharp_tcp_server() {
    let runner = TestRunner::start();

    bin()
        .args(["ping", "--port", &runner.port.to_string()])
        .assert()
        .success()
        .stdout("")
        .stderr("");
}

#[test]
fn cli_can_list_methods_from_csharp_tcp_server() {
    let runner = TestRunner::start();

    bin()
        .args(["list-methods", "--port", &runner.port.to_string()])
        .assert()
        .success()
        .stdout(predicate::str::contains("system.ping"))
        .stdout(predicate::str::contains("rpc.list_methods"))
        .stdout(predicate::str::contains("test.echo"));
}

#[test]
fn cli_surfaces_csharp_method_not_found_error() {
    let runner = TestRunner::start();

    bin()
        .args(["call", "missing.method", "--port", &runner.port.to_string()])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("\"code\":-32601"));
}

#[test]
fn raw_tcp_parse_error_returns_json_rpc_error() {
    let runner = TestRunner::start();
    let mut stream = TcpStream::connect(("127.0.0.1", runner.port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":3,\"meth\n")
        .unwrap();

    let mut response_line = String::new();
    BufReader::new(stream)
        .read_line(&mut response_line)
        .unwrap();
    let response: Value = serde_json::from_str(&response_line).unwrap();

    assert_eq!(response["error"]["code"], -32700);
}
