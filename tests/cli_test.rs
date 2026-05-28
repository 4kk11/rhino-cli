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

fn spawn_rpc_server_n<F>(connections: usize, handler: F) -> (u16, thread::JoinHandle<()>)
where
    F: Fn(Value) -> Value + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = thread::spawn(move || {
        for _ in 0..connections {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&stream);
            let response = handler(request);
            let mut line = serde_json::to_string(&response).unwrap();
            line.push('\n');
            stream.write_all(line.as_bytes()).unwrap();
        }
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

fn unused_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
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
            "result": {"methods": ["system.ping", "rpc.list_methods", "rhino_cli.hello"]}
        })
    });

    bin()
        .args(["list-methods", "--port", &port.to_string()])
        .assert()
        .success()
        .stdout("system.ping\nrpc.list_methods\nrhino_cli.hello\n");
    handle.join().unwrap();
}

#[test]
fn list_plugins_prints_id_and_port_per_line() {
    let (port, handle) = spawn_rpc_server(|request| {
        assert_eq!(request["method"], "rpc.list_plugins");
        assert!(request["params"].is_null());
        json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {"plugins": [{"id": "RhinoCliPlugin", "port": 50061}]}
        })
    });

    bin()
        .args(["list-plugins", "--port", &port.to_string()])
        .assert()
        .success()
        .stdout("RhinoCliPlugin\t50061\n");
    handle.join().unwrap();
}

#[test]
fn list_plugins_raw_pretty_prints_json() {
    let (port, handle) = spawn_rpc_server(|request| {
        assert_eq!(request["method"], "rpc.list_plugins");
        json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {"plugins": [{"id": "RhinoCliPlugin", "port": 50061}]}
        })
    });

    bin()
        .args([
            "list-plugins",
            "--port",
            &port.to_string(),
            "--raw",
            "--pretty",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"plugins\""))
        .stdout(predicate::str::contains("\"RhinoCliPlugin\""))
        .stdout(predicate::str::contains("50061"));
    handle.join().unwrap();
}

#[test]
fn capabilities_prints_handler_metadata() {
    let (port, handle) = spawn_rpc_server(|request| {
        assert_eq!(request["method"], "rpc.capabilities");
        assert!(request["params"].is_null());
        json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {
                "server": {"pluginId": "RhinoCliPlugin", "port": 50061, "serverVersion": "0.1.0"},
                "methods": [{
                    "method": "rhino.run_script",
                    "description": "Run a Rhino command script.",
                    "paramsSchema": "{ script: string }",
                    "resultSchema": "{ success: boolean }",
                    "examples": ["rhino-cli run-script \"_Zoom _Extents\""],
                    "dedicatedCommand": "rhino-cli run-script <SCRIPT>",
                    "sideEffects": "May modify the active document.",
                    "category": "rhino"
                }]
            }
        })
    });

    bin()
        .args(["capabilities", "--port", &port.to_string()])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("rhino.run_script")
                .and(predicate::str::contains("Run a Rhino command script."))
                .and(predicate::str::contains("rhino-cli run-script <SCRIPT>")),
        );
    handle.join().unwrap();
}

#[test]
fn capabilities_method_sends_method_param() {
    let (port, handle) = spawn_rpc_server(|request| {
        assert_eq!(request["method"], "rpc.capabilities");
        assert_eq!(request["params"], json!({"method": "rhino.new_model"}));
        json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {
                "server": {"pluginId": "RhinoCliPlugin", "port": 50061, "serverVersion": "0.1.0"},
                "method": {
                    "method": "rhino.new_model",
                    "description": "Create a new Rhino model.",
                    "paramsSchema": "null | { template?: string }",
                    "resultSchema": "{ status: string }",
                    "examples": ["rhino-cli new-model"],
                    "dedicatedCommand": "rhino-cli new-model [--template <3dm>]",
                    "sideEffects": "Creates a new unsaved Rhino document.",
                    "category": "rhino"
                }
            }
        })
    });

    bin()
        .args([
            "capabilities",
            "--method",
            "rhino.new_model",
            "--format",
            "json",
            "--port",
            &port.to_string(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"method\":\"rhino.new_model\""));
    handle.join().unwrap();
}

#[test]
fn doctor_reports_reachable_rpc() {
    let (port, handle) = spawn_rpc_server(|request| {
        assert_eq!(request["method"], "system.ping");
        json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {"pong": true, "server": "RhinoCliPlugin", "version": "0.1.0"}
        })
    });

    bin()
        .args(["doctor", "--port", &port.to_string()])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("RhinoCliPlugin RPC: reachable")
                .and(predicate::str::contains("Server: RhinoCliPlugin 0.1.0")),
        );
    handle.join().unwrap();
}

#[test]
fn call_sends_positional_json_params_and_prints_result() {
    let (port, handle) = spawn_rpc_server(|request| {
        assert_eq!(request["method"], "rhino_cli.echo");
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
            "rhino_cli.echo",
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
            "rhino_cli.echo",
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
fn wait_ready_warns_when_active_doc_missing() {
    let (port, handle) = spawn_rpc_server_n(2, |request| match request["method"].as_str() {
        Some("system.ping") => json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {"pong": true}
        }),
        Some("rhino.run_python") => json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {
                "status": "ok",
                "success": true,
                "stdout": "",
                "result": "{\"active_doc\": false, \"open_count\": 0}",
                "result_repr": ""
            }
        }),
        other => panic!("unexpected method {other:?}"),
    });

    bin()
        .args(["wait-ready", "--port", &port.to_string(), "--timeout", "2"])
        .assert()
        .success()
        .stdout("")
        .stderr(predicate::str::contains("Rhino.RhinoDoc.ActiveDoc is None"));
    handle.join().unwrap();
}

#[test]
fn wait_ready_silent_when_active_doc_present() {
    let (port, handle) = spawn_rpc_server_n(2, |request| match request["method"].as_str() {
        Some("system.ping") => json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {"pong": true}
        }),
        Some("rhino.run_python") => json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {
                "status": "ok",
                "success": true,
                "stdout": "",
                "result": "{\"active_doc\": true, \"open_count\": 1}",
                "result_repr": ""
            }
        }),
        other => panic!("unexpected method {other:?}"),
    });

    bin()
        .args(["wait-ready", "--port", &port.to_string(), "--timeout", "2"])
        .assert()
        .success()
        .stdout("")
        .stderr("");
    handle.join().unwrap();
}

#[test]
fn doctor_reports_warning_when_active_doc_missing() {
    let (port, handle) = spawn_rpc_server_n(2, |request| match request["method"].as_str() {
        Some("system.ping") => json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {"pong": true, "server": "RhinoCliPlugin", "version": "0.1.0"}
        }),
        Some("rhino.run_python") => json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {
                "status": "ok",
                "success": true,
                "stdout": "",
                "result": "{\"active_doc\": false, \"open_count\": 0}",
                "result_repr": ""
            }
        }),
        other => panic!("unexpected method {other:?}"),
    });

    bin()
        .args(["doctor", "--port", &port.to_string()])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Document: active_doc=false open_count=0")
                .and(predicate::str::contains(
                    "Rhino.RhinoDoc.ActiveDoc is None",
                )),
        );
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
fn connection_refused_prints_rhino_start_hint() {
    let port = unused_port();

    bin()
        .args([
            "list-methods",
            "--port",
            &port.to_string(),
            "--connect-timeout",
            "1",
        ])
        .assert()
        .code(2)
        .stderr(
            predicate::str::contains("Rhino is not reachable")
                .and(predicate::str::contains(format!(
                    "rhino-cli plugin set-port {port}"
                )))
                .and(predicate::str::contains("rhino-cli launch"))
                .and(predicate::str::contains(format!(
                    "rhino-cli wait-ready --port {port} --timeout 120"
                ))),
        );
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
fn execute_panel_js_sends_panel_and_script() {
    let (port, handle) = spawn_rpc_server(|request| {
        assert_eq!(request["method"], "rhino.execute_in_panel_webview");
        assert_eq!(
            request["params"],
            json!({
                "panel": "F2A3B4C5-D6E7-8901-ABCD-EF0123456789",
                "script": "return document.readyState"
            })
        );
        json!({
            "jsonrpc": "2.0",
            "id": request["id"].clone(),
            "result": {
                "status": "ok",
                "value": "complete",
                "panel_type": "AICmdHub.Panels.MainPanel"
            }
        })
    });

    bin()
        .args([
            "execute-panel-js",
            "F2A3B4C5-D6E7-8901-ABCD-EF0123456789",
            "return document.readyState",
            "--port",
            &port.to_string(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"value\":\"complete\""));
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
