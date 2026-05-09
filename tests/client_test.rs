use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use rhino_cli::client::Client;
use rhino_cli::error::CliError;
use serde_json::json;

fn bind_local() -> TcpListener {
    TcpListener::bind("127.0.0.1:0").unwrap()
}

fn listener_port(listener: &TcpListener) -> u16 {
    listener.local_addr().unwrap().port()
}

fn respond_once<F>(listener: TcpListener, handler: F) -> thread::JoinHandle<()>
where
    F: FnOnce(TcpStream) + Send + 'static,
{
    thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        handler(stream);
    })
}

#[test]
fn client_call_sends_one_line_and_reads_result() {
    let listener = bind_local();
    let port = listener_port(&listener);
    let handle = respond_once(listener, |mut stream| {
        let mut line = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut line)
            .unwrap();
        assert_eq!(
            line,
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"system.ping\",\"params\":null}\n"
        );
        stream
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"pong\":true}}\n")
            .unwrap();
    });

    let client = Client::new(
        "127.0.0.1",
        port,
        Duration::from_secs(1),
        Duration::from_secs(1),
    );
    let result = client.call("system.ping", json!(null)).unwrap();

    assert_eq!(result, json!({"pong": true}));
    handle.join().unwrap();
}

#[test]
fn client_call_maps_connection_refused_to_connect_error() {
    let listener = bind_local();
    let port = listener_port(&listener);
    drop(listener);

    let client = Client::new(
        "127.0.0.1",
        port,
        Duration::from_millis(50),
        Duration::from_millis(50),
    );
    let error = client.call("system.ping", json!(null)).unwrap_err();

    assert!(matches!(error, CliError::Connect(_)), "got {error:?}");
}

#[test]
fn client_call_maps_read_timeout_to_timeout_error() {
    let listener = bind_local();
    let port = listener_port(&listener);
    let handle = respond_once(listener, |_stream| {
        thread::sleep(Duration::from_millis(200));
    });

    let client = Client::new(
        "127.0.0.1",
        port,
        Duration::from_secs(1),
        Duration::from_millis(50),
    );
    let error = client.call("system.ping", json!(null)).unwrap_err();

    assert!(matches!(error, CliError::Timeout(_)), "got {error:?}");
    handle.join().unwrap();
}

#[test]
fn client_call_maps_server_disconnect_to_parse_error() {
    let listener = bind_local();
    let port = listener_port(&listener);
    let handle = respond_once(listener, |_stream| {});

    let client = Client::new(
        "127.0.0.1",
        port,
        Duration::from_secs(1),
        Duration::from_secs(1),
    );
    let error = client.call("system.ping", json!(null)).unwrap_err();

    assert!(matches!(error, CliError::Parse(_)), "got {error:?}");
    handle.join().unwrap();
}

#[test]
fn client_call_detects_response_id_mismatch() {
    let listener = bind_local();
    let port = listener_port(&listener);
    let handle = respond_once(listener, |mut stream| {
        let mut line = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut line)
            .unwrap();
        stream
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":99,\"result\":true}\n")
            .unwrap();
    });

    let client = Client::new(
        "127.0.0.1",
        port,
        Duration::from_secs(1),
        Duration::from_secs(1),
    );
    let error = client.call("system.ping", json!(null)).unwrap_err();

    assert!(
        matches!(error, CliError::InvalidResponse(_)),
        "got {error:?}"
    );
    handle.join().unwrap();
}
