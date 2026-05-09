use rhino_cli::protocol::{Id, Request, Response};
use serde_json::json;

#[test]
fn request_serializes_to_json_line() {
    let request = Request::new(1, "system.ping", None);

    assert_eq!(
        request.to_json_line().unwrap(),
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"system.ping\",\"params\":null}\n"
    );
}

#[test]
fn request_serializes_empty_params() {
    let request = Request::new(99, "rpc.list_methods", Some(json!({})));

    assert_eq!(
        request.to_json_line().unwrap(),
        "{\"jsonrpc\":\"2.0\",\"id\":99,\"method\":\"rpc.list_methods\",\"params\":{}}\n"
    );
}

#[test]
fn request_serializes_large_numeric_id() {
    let request = Request::new(u64::MAX, "system.version", None);

    assert_eq!(
        request.to_json_line().unwrap(),
        format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":{},\"method\":\"system.version\",\"params\":null}}\n",
            u64::MAX
        )
    );
}

#[test]
fn success_response_parses() {
    let response =
        Response::from_json_line("{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"pong\":true}}\n")
            .unwrap();

    assert_eq!(response.id, Some(Id::Number(1)));
    assert_eq!(response.result, Some(json!({"pong": true})));
    assert!(response.error.is_none());
}

#[test]
fn error_response_parses() {
    let response = Response::from_json_line(
        "{\"jsonrpc\":\"2.0\",\"id\":\"abc\",\"error\":{\"code\":-32601,\"message\":\"Method not found\",\"data\":{\"method\":\"x.y\"}}}\n",
    )
    .unwrap();

    let error = response.error.unwrap();
    assert_eq!(response.id, Some(Id::String("abc".to_string())));
    assert_eq!(error.code, -32601);
    assert_eq!(error.message, "Method not found");
    assert_eq!(error.data, Some(json!({"method": "x.y"})));
}

#[test]
fn invalid_json_returns_parse_error() {
    let error = Response::from_json_line("{\"jsonrpc\":\"2.0\",\"id\":3,\"meth\n").unwrap_err();

    assert_eq!(error.exit_code(), 5);
    assert!(error.to_string().contains("parse"));
}

#[test]
fn response_with_result_and_error_is_invalid() {
    let error = Response::from_json_line(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{},\"error\":{\"code\":-32603,\"message\":\"boom\"}}\n",
    )
    .unwrap_err();

    assert_eq!(error.exit_code(), 5);
    assert!(error.to_string().contains("result"));
}

#[test]
fn response_id_mismatch_is_detected() {
    let response =
        Response::from_json_line("{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":true}\n").unwrap();
    let error = response.validate_id(&Id::Number(1)).unwrap_err();

    assert_eq!(error.exit_code(), 5);
    assert!(error.to_string().contains("id mismatch"));
}
