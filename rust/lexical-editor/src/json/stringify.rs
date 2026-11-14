use super::number::number_to_string;
use super::value::Value;

/// `JSON.stringify(value)`: `None` where JS returns `undefined`, which is for
/// [`Value::Undefined`] alone.
pub fn stringify(value: &Value) -> Option<String> {
    if value.is_undefined() {
        return None;
    }
    let mut out = String::new();
    write_value(&mut out, value);
    Some(out)
}

fn write_value(out: &mut String, value: &Value) {
    match value {
        // In an array; an object skips the property before getting here.
        Value::Undefined | Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) if n.is_finite() => out.push_str(&number_to_string(*n)),
        Value::Number(_) => out.push_str("null"),
        Value::String(s) => write_string(out, s),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(out, item);
            }
            out.push(']');
        }
        Value::Object(object) => {
            out.push('{');
            let mut first = true;
            for (key, item) in object.iter() {
                if item.is_undefined() {
                    continue;
                }
                if !first {
                    out.push(',');
                }
                first = false;
                write_string(out, key);
                out.push(':');
                write_value(out, item);
            }
            out.push('}');
        }
    }
}

/// QuoteJSONString. A Rust string has no lone surrogates, so the escapes JS
/// writes for them never apply (rust/PARITY.md).
fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}
