use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{json, Value};

fn spawn_rpc_server<F>(handler: F) -> (u16, thread::JoinHandle<()>)
where
    F: FnOnce(Value) -> Value + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = read_request(&stream);
        let response = handler(request);
        let mut line = serde_json::to_string(&response).unwrap();
        line.push('\n');
        stream.write_all(line.as_bytes()).unwrap();
    });
    (port, handle)
}

fn read_request(stream: &TcpStream) -> Value {
    let mut line = String::new();
    BufReader::new(stream.try_clone().unwrap())
        .read_line(&mut line)
        .unwrap();
    serde_json::from_str(&line).unwrap()
}

fn bin() -> Command {
    Command::cargo_bin("rhino-cli").unwrap()
}

#[test]
fn ping_calls_system_ping_and_reports_verbose_latency() {
    let (port, handle) = spawn_rpc_server(|request| {
        assert_eq!(request["method"], "system.ping");
        json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {"pong": true, "server": "MockRhino", "version": "0.1.0"}
        })
    });

    bin()
        .args(["ping", "--port", &port.to_string(), "--verbose"])
        .assert()
        .success()
        .stdout("")
        .stderr(predicate::str::contains("pong from MockRhino 0.1.0"));
    handle.join().unwrap();
}

#[test]
fn list_methods_prints_one_method_per_line() {
    let (port, handle) = spawn_rpc_server(|request| {
        assert_eq!(request["method"], "rpc.list_methods");
        json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {"methods": ["system.ping", "rpc.list_methods", "minimal.hello"]}
        })
    });

    bin()
        .args(["list-methods", "--port", &port.to_string()])
        .assert()
        .success()
        .stdout("system.ping\nrpc.list_methods\nminimal.hello\n");
    handle.join().unwrap();
}

#[test]
fn call_sends_positional_json_params_and_prints_result() {
    let (port, handle) = spawn_rpc_server(|request| {
        assert_eq!(request["method"], "minimal.echo");
        assert_eq!(request["params"], json!({"message": "hello"}));
        json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {"ok": true}
        })
    });

    bin()
        .args([
            "call",
            "minimal.echo",
            "{\"message\":\"hello\"}",
            "--port",
            &port.to_string(),
        ])
        .assert()
        .success()
        .stdout("{\"ok\":true}\n");
    handle.join().unwrap();
}

#[test]
fn call_builds_object_params_from_key_value_flags() {
    let (port, handle) = spawn_rpc_server(|request| {
        assert_eq!(request["params"], json!({"count": 3, "name": "beam"}));
        json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": request["params"].clone()
        })
    });

    bin()
        .args([
            "call",
            "minimal.echo",
            "--param",
            "count=3",
            "--param",
            "name=beam",
            "--port",
            &port.to_string(),
        ])
        .assert()
        .success()
        .stdout("{\"count\":3,\"name\":\"beam\"}\n");
    handle.join().unwrap();
}

#[test]
fn wait_ready_returns_success_after_ping() {
    let (port, handle) = spawn_rpc_server(|request| {
        assert_eq!(request["method"], "system.ping");
        json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {"pong": true}
        })
    });

    bin()
        .args(["wait-ready", "--port", &port.to_string(), "--timeout", "1"])
        .assert()
        .success()
        .stdout("")
        .stderr("");
    handle.join().unwrap();
}

#[test]
fn call_rpc_error_exits_with_code_3_and_json_stderr() {
    let (port, handle) = spawn_rpc_server(|request| {
        json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "error": {"code": -32601, "message": "Method not found"}
        })
    });

    bin()
        .args(["call", "missing.method", "--port", &port.to_string()])
        .assert()
        .code(3)
        .stderr(predicate::str::contains(
            "{\"error\":{\"code\":-32601,\"message\":\"Method not found\"}}\n",
        ));
    handle.join().unwrap();
}

#[test]
fn call_raw_still_exits_with_code_3_on_rpc_error() {
    let (port, handle) = spawn_rpc_server(|request| {
        json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "error": {"code": -32601, "message": "Method not found"}
        })
    });

    bin()
        .args([
            "call",
            "missing.method",
            "--raw",
            "--port",
            &port.to_string(),
        ])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("\"code\":-32601"));
    handle.join().unwrap();
}
