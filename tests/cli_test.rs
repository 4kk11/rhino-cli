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

#[test]
fn run_script_calls_rhino_run_script() {
    let (port, handle) = spawn_rpc_server(|request| {
        assert_eq!(request["method"], "rhino.run_script");
        assert_eq!(
            request["params"],
            json!({
                "script": "_SelNone",
                "echo": true,
                "mru_display_string": "Select none"
            })
        );
        json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {"status": "ok", "success": true}
        })
    });

    bin()
        .args([
            "run-script",
            "_SelNone",
            "--echo",
            "--mru",
            "Select none",
            "--port",
            &port.to_string(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"success\":true"));
    handle.join().unwrap();
}

#[test]
fn run_script_can_fail_on_false_result() {
    let (port, handle) = spawn_rpc_server(|request| {
        assert_eq!(request["method"], "rhino.run_script");
        json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {"status": "failed", "success": false}
        })
    });

    bin()
        .args([
            "run-script",
            "_SelNone",
            "--fail-on-false",
            "--port",
            &port.to_string(),
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("\"success\":false"))
        .stderr(predicate::str::contains("success=false"));
    handle.join().unwrap();
}

#[test]
fn history_prints_text_by_default() {
    let (port, handle) = spawn_rpc_server(|request| {
        assert_eq!(request["method"], "rhino.command_history");
        assert_eq!(request["params"], json!({"tail": 2}));
        json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {
                "status": "ok",
                "text": "line one\nline two",
                "line_count": 2,
                "total_line_count": 10,
                "truncated": true
            }
        })
    });

    bin()
        .args(["history", "--tail", "2", "--port", &port.to_string()])
        .assert()
        .success()
        .stdout("line one\nline two\n");
    handle.join().unwrap();
}

#[test]
fn history_clear_calls_clear_method() {
    let (port, handle) = spawn_rpc_server(|request| {
        assert_eq!(request["method"], "rhino.clear_command_history");
        assert!(request["params"].is_null());
        json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {"status": "ok"}
        })
    });

    bin()
        .args(["history", "--clear", "--port", &port.to_string()])
        .assert()
        .success()
        .stdout("");
    handle.join().unwrap();
}

#[test]
fn new_model_calls_rhino_new_model_without_template() {
    let (port, handle) = spawn_rpc_server(|request| {
        assert_eq!(request["method"], "rhino.new_model");
        assert!(request["params"].is_null());
        json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {
                "status": "ok",
                "document": {"runtime_serial_number": 42, "name": "", "path": ""}
            }
        })
    });

    bin()
        .args(["new-model", "--port", &port.to_string()])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"runtime_serial_number\":42"));
    handle.join().unwrap();
}

#[test]
fn new_model_sends_template_path() {
    let (port, handle) = spawn_rpc_server(|request| {
        assert_eq!(request["method"], "rhino.new_model");
        assert_eq!(request["params"], json!({"template": "/tmp/template.3dm"}));
        json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {"status": "ok"}
        })
    });

    bin()
        .args([
            "new-model",
            "--template",
            "/tmp/template.3dm",
            "--port",
            &port.to_string(),
        ])
        .assert()
        .success()
        .stdout("{\"status\":\"ok\"}\n");
    handle.join().unwrap();
}

#[test]
fn screenshot_rejects_invalid_app_name_before_os_capture() {
    bin()
        .args(["screenshot", "--app", "Rhino 8; rm -rf /"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("invalid Rhino app name"));
}
