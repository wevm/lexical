//! `LexicalTableCommands.ts`: `INSERT_TABLE_COMMAND` and its payload.

use crate::LexicalCommand;
use crate::json::{Object, Value};

/// `InsertTableCommandPayloadHeaders`: `true` or `false` for a header row
/// and column both, or each on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertTableCommandPayloadHeaders {
    Both(bool),
    Each { rows: bool, columns: bool },
}

impl Default for InsertTableCommandPayloadHeaders {
    /// `includeHeaders = true`.
    fn default() -> Self {
        InsertTableCommandPayloadHeaders::Both(true)
    }
}

/// `InsertTableCommandPayload`: the dimensions as strings, as JS's
/// `Number(rows)` reads them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InsertTableCommandPayload {
    pub columns: String,
    pub rows: String,
    pub include_headers: Option<InsertTableCommandPayloadHeaders>,
}

impl InsertTableCommandPayload {
    /// The payload of its JSON form, `{columns, rows, includeHeaders}`,
    /// as a script dispatches it.
    pub fn from_json(json: &Object) -> Self {
        let string = |key: &str| match json.get(key) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) => crate::json::number_to_string(*n),
            _ => String::new(),
        };
        let include_headers = match json.get("includeHeaders") {
            Some(Value::Bool(b)) => Some(InsertTableCommandPayloadHeaders::Both(*b)),
            Some(Value::Object(o)) => Some(InsertTableCommandPayloadHeaders::Each {
                rows: o.get("rows").and_then(Value::as_bool).unwrap_or(false),
                columns: o.get("columns").and_then(Value::as_bool).unwrap_or(false),
            }),
            _ => None,
        };
        Self {
            columns: string("columns"),
            rows: string("rows"),
            include_headers,
        }
    }
}

/// `Number(value)` for a dimension: a whole number of rows or columns, 0
/// for anything a loop `i < Number(value)` never enters.
pub(crate) fn dimension(value: &str) -> usize {
    let trimmed = value.trim();
    let n: f64 = if trimmed.is_empty() {
        0.0
    } else {
        trimmed.parse().unwrap_or(f64::NAN)
    };
    if n.is_nan() || n <= 0.0 {
        0
    } else {
        n.ceil() as usize
    }
}

/// `INSERT_TABLE_COMMAND`.
pub const INSERT_TABLE_COMMAND: LexicalCommand<InsertTableCommandPayload> =
    LexicalCommand::constant(u64::MAX - 60, "INSERT_TABLE_COMMAND");
