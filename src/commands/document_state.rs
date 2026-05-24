use serde::Deserialize;
use serde_json::{json, Value};

use crate::client::Client;
use crate::error::Result;

const PROBE_SOURCE: &str = "import Rhino, json\n\
opens = list(Rhino.RhinoDoc.OpenDocuments(True))\n\
active = Rhino.RhinoDoc.ActiveDoc\n\
state = {\n\
  'active_doc': active is not None,\n\
  'open_count': len(opens),\n\
}";

const PROBE_RESULT_EXPRESSION: &str = "json.dumps(state)";

#[derive(Debug, Clone, Copy)]
pub struct DocumentState {
    pub active_doc: bool,
    pub open_count: u32,
}

#[derive(Debug, Deserialize)]
struct ProbePayload {
    active_doc: bool,
    open_count: u32,
}

impl DocumentState {
    /// True when Rhino has no active document. The usual cause is Rhino's
    /// start window (recent/template picker) still being up — panel/python
    /// operations from the plugin will silently fail until ActiveDoc is set.
    /// OpenDocuments may be 0 or 1 in this state depending on the Rhino
    /// build and template configuration; the load-bearing signal is
    /// ActiveDoc itself, not the count.
    pub fn active_doc_missing(&self) -> bool {
        !self.active_doc
    }
}

/// Probe ActiveDoc / OpenDocuments via `rhino.run_python`. Returns `Ok(None)`
/// when the call succeeds but the payload is not parseable (e.g. older
/// RhinoCliPlugin that lacks run_python), or `Err` when the RPC itself fails.
pub fn probe(client: &Client) -> Result<Option<DocumentState>> {
    let params = json!({
        "source": PROBE_SOURCE,
        "result_expression": PROBE_RESULT_EXPRESSION,
    });
    let result = client.call("rhino.run_python", params)?;
    Ok(parse(&result))
}

fn parse(result: &Value) -> Option<DocumentState> {
    let raw = result.get("result")?.as_str()?;
    let payload: ProbePayload = serde_json::from_str(raw).ok()?;
    Some(DocumentState {
        active_doc: payload.active_doc,
        open_count: payload.open_count,
    })
}

pub const ACTIVE_DOC_MISSING_WARNING: &str =
    "warning: Rhino.RhinoDoc.ActiveDoc is None. \
The start window (recent/template picker) is likely still up; plugin panel/python operations will silently fail until a document is active. \
Dismiss it in Rhino, or relaunch with `rhino-cli launch --restart` (the default opens a new model and avoids this state).";

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_payload_string() {
        let response = json!({
            "status": "ok",
            "result": "{\"active_doc\": false, \"open_count\": 1}",
        });
        let state = parse(&response).expect("payload");
        assert!(!state.active_doc);
        assert_eq!(state.open_count, 1);
        assert!(state.active_doc_missing());
    }

    #[test]
    fn returns_none_when_result_missing() {
        let response = json!({ "status": "ok" });
        assert!(parse(&response).is_none());
    }

    #[test]
    fn active_doc_missing_when_no_active_regardless_of_open_count() {
        let state = DocumentState {
            active_doc: false,
            open_count: 0,
        };
        assert!(state.active_doc_missing());
    }

    #[test]
    fn active_doc_present_is_not_missing() {
        let state = DocumentState {
            active_doc: true,
            open_count: 1,
        };
        assert!(!state.active_doc_missing());
    }
}
