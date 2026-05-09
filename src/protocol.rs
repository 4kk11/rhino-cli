use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::error::{CliError, Result};

pub const JSONRPC_VERSION: &str = "2.0";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Id {
    Number(u64),
    String(String),
    Null,
}

impl Serialize for Id {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Id::Number(value) => serializer.serialize_u64(*value),
            Id::String(value) => serializer.serialize_str(value),
            Id::Null => serializer.serialize_none(),
        }
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        Id::from_value(&value).map_err(serde::de::Error::custom)
    }
}

impl Id {
    fn from_value(value: &Value) -> std::result::Result<Self, String> {
        match value {
            Value::Number(number) => number
                .as_u64()
                .map(Id::Number)
                .ok_or_else(|| "id must be an unsigned integer".to_string()),
            Value::String(value) => Ok(Id::String(value.clone())),
            Value::Null => Ok(Id::Null),
            _ => Err("id must be a number, string, or null".to_string()),
        }
    }
}

impl From<u64> for Id {
    fn from(value: u64) -> Self {
        Id::Number(value)
    }
}

impl From<&str> for Id {
    fn from(value: &str) -> Self {
        Id::String(value.to_string())
    }
}

impl From<String> for Id {
    fn from(value: String) -> Self {
        Id::String(value)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub jsonrpc: &'static str,
    pub id: Id,
    pub method: String,
    pub params: Value,
}

impl Request {
    pub fn new<I>(id: I, method: impl Into<String>, params: Option<Value>) -> Self
    where
        I: Into<Id>,
    {
        Self {
            jsonrpc: JSONRPC_VERSION,
            id: id.into(),
            method: method.into(),
            params: params.unwrap_or(Value::Null),
        }
    }

    pub fn to_json_line(&self) -> Result<String> {
        let mut line = serde_json::to_string(self)?;
        line.push('\n');
        Ok(line)
    }
}

impl Serialize for Request {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Request", 4)?;
        state.serialize_field("jsonrpc", self.jsonrpc)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("method", &self.method)?;
        state.serialize_field("params", &self.params)?;
        state.end()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Response {
    pub jsonrpc: String,
    pub id: Option<Id>,
    pub result: Option<Value>,
    pub error: Option<RpcError>,
}

impl Response {
    pub fn from_json_line(line: &str) -> Result<Self> {
        let value: Value = serde_json::from_str(line)?;
        let object = value.as_object().ok_or_else(|| {
            CliError::InvalidResponse("response must be a JSON object".to_string())
        })?;

        let jsonrpc = required_string(object, "jsonrpc")?;
        if jsonrpc != JSONRPC_VERSION {
            return Err(CliError::InvalidResponse(format!(
                "jsonrpc must be {JSONRPC_VERSION:?}"
            )));
        }

        let has_result = object.contains_key("result");
        let has_error = object.contains_key("error");
        match (has_result, has_error) {
            (true, true) => {
                return Err(CliError::InvalidResponse(
                    "response cannot contain both result and error".to_string(),
                ));
            }
            (false, false) => {
                return Err(CliError::InvalidResponse(
                    "response must contain result or error".to_string(),
                ));
            }
            _ => {}
        }

        let id = match object.get("id") {
            Some(value) => Some(Id::from_value(value).map_err(CliError::InvalidResponse)?),
            None => None,
        };
        let result = object.get("result").cloned();
        let error = object
            .get("error")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| CliError::InvalidResponse(error.to_string()))?;

        Ok(Self {
            jsonrpc: jsonrpc.to_string(),
            id,
            result,
            error,
        })
    }

    pub fn validate_id(&self, expected: &Id) -> Result<()> {
        match &self.id {
            Some(actual) if actual == expected => Ok(()),
            Some(actual) => Err(CliError::InvalidResponse(format!(
                "id mismatch: expected {expected:?}, got {actual:?}"
            ))),
            None => Err(CliError::InvalidResponse(
                "response is missing id".to_string(),
            )),
        }
    }

    pub fn to_value(&self) -> Result<Value> {
        let mut object = Map::new();
        object.insert("jsonrpc".to_string(), Value::String(self.jsonrpc.clone()));
        if let Some(id) = &self.id {
            object.insert("id".to_string(), serde_json::to_value(id)?);
        }
        if let Some(result) = &self.result {
            object.insert("result".to_string(), result.clone());
        }
        if let Some(error) = &self.error {
            object.insert("error".to_string(), serde_json::to_value(error)?);
        }
        Ok(Value::Object(object))
    }
}

fn required_string<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str> {
    object.get(field).and_then(Value::as_str).ok_or_else(|| {
        CliError::InvalidResponse(format!("response is missing string field {field:?}"))
    })
}
