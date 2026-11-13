use super::value::{Object, Value};
use std::fmt;

/// Why `JSON.parse` would throw a `SyntaxError`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    /// The byte offset in the text.
    pub position: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} in JSON at position {}", self.message, self.position)
    }
}

impl std::error::Error for ParseError {}

/// `JSON.parse(text)`. A `\u` escape of half a surrogate pair becomes
/// U+FFFD, the Tier 1 exception for unpaired surrogates (rust/PARITY.md).
pub fn parse(text: &str) -> Result<Value, ParseError> {
    let mut parser = Parser {
        text,
        bytes: text.as_bytes(),
        pos: 0,
    };
    parser.skip_whitespace();
    let value = parser.value()?;
    parser.skip_whitespace();
    if parser.pos != parser.bytes.len() {
        return Err(parser.error("Unexpected non-whitespace character after JSON"));
    }
    Ok(value)
}

struct Parser<'a> {
    text: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn error(&self, message: &str) -> ParseError {
        ParseError {
            message: message.to_owned(),
            position: self.pos,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_whitespace(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.peek() {
            self.pos += 1;
        }
    }

    fn expect_literal(&mut self, literal: &str, value: Value) -> Result<Value, ParseError> {
        if self.text[self.pos..].starts_with(literal) {
            self.pos += literal.len();
            Ok(value)
        } else {
            Err(self.error("Unexpected token"))
        }
    }

    fn value(&mut self) -> Result<Value, ParseError> {
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(Value::String),
            Some(b't') => self.expect_literal("true", Value::Bool(true)),
            Some(b'f') => self.expect_literal("false", Value::Bool(false)),
            Some(b'n') => self.expect_literal("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.error("Unexpected token")),
            None => Err(self.error("Unexpected end of JSON input")),
        }
    }

    fn object(&mut self) -> Result<Value, ParseError> {
        self.pos += 1;
        let mut object = Object::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Object(object));
        }
        loop {
            self.skip_whitespace();
            if self.peek() != Some(b'"') {
                return Err(self.error("Expected property name"));
            }
            let key = self.string()?;
            self.skip_whitespace();
            if self.peek() != Some(b':') {
                return Err(self.error("Expected ':' after property name"));
            }
            self.pos += 1;
            self.skip_whitespace();
            let value = self.value()?;
            // A repeated key keeps its first position and its last value.
            object.insert(key, value);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(object));
                }
                _ => return Err(self.error("Expected ',' or '}' after property value")),
            }
        }
    }

    fn array(&mut self) -> Result<Value, ParseError> {
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.skip_whitespace();
            items.push(self.value()?);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.error("Expected ',' or ']' after array element")),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, ParseError> {
        let digits = self
            .text
            .get(self.pos..self.pos + 4)
            .filter(|d| d.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| self.error("Bad Unicode escape"))?;
        self.pos += 4;
        Ok(u32::from_str_radix(digits, 16).expect("hex digits"))
    }

    fn string(&mut self) -> Result<String, ParseError> {
        self.pos += 1;
        let mut out = String::new();
        loop {
            let start = self.pos;
            while let Some(b) = self.peek() {
                if b == b'"' || b == b'\\' || b < 0x20 {
                    break;
                }
                self.pos += 1;
            }
            out.push_str(&self.text[start..self.pos]);
            match self.peek() {
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    let escape = self.peek().ok_or_else(|| self.error("Bad escape"))?;
                    self.pos += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let unit = self.hex4()?;
                            out.push(self.code_point(unit)?);
                        }
                        _ => {
                            self.pos -= 1;
                            return Err(self.error("Bad escaped character"));
                        }
                    }
                }
                Some(_) => return Err(self.error("Bad control character in string literal")),
                None => return Err(self.error("Unterminated string")),
            }
        }
    }

    /// The character a `\u` escape of `unit` starts, reading the low half of
    /// a surrogate pair when one follows.
    fn code_point(&mut self, unit: u32) -> Result<char, ParseError> {
        if (0xD800..0xDC00).contains(&unit) && self.text[self.pos..].starts_with("\\u") {
            let save = self.pos;
            self.pos += 2;
            let low = self.hex4()?;
            if (0xDC00..0xE000).contains(&low) {
                let c = 0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00);
                return Ok(char::from_u32(c).expect("surrogate pair"));
            }
            self.pos = save;
        }
        Ok(char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER))
    }

    fn digits(&mut self) -> usize {
        let start = self.pos;
        while let Some(b'0'..=b'9') = self.peek() {
            self.pos += 1;
        }
        self.pos - start
    }

    fn number(&mut self) -> Result<Value, ParseError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                self.digits();
            }
            _ => return Err(self.error("No number after minus sign")),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            if self.digits() == 0 {
                return Err(self.error("Unterminated fractional number"));
            }
        }
        if let Some(b'e' | b'E') = self.peek() {
            self.pos += 1;
            if let Some(b'+' | b'-') = self.peek() {
                self.pos += 1;
            }
            if self.digits() == 0 {
                return Err(self.error("Exponent part is missing a number"));
            }
        }
        // Correctly rounded, and out of range to ±Infinity, as in JS.
        let n: f64 = self.text[start..self.pos].parse().expect("JSON number");
        Ok(Value::Number(n))
    }
}

#[cfg(test)]
mod tests {
    use super::super::stringify;
    use super::*;

    #[test]
    fn round_trips_what_js_writes() {
        // Each `JSON.stringify(JSON.parse(text))` in Node.
        let cases = [
            (
                r#"{"b":1,"a":[true,null,-0,1e21]}"#,
                r#"{"b":1,"a":[true,null,0,1e+21]}"#,
            ),
            (r#" {"a" : 1 , "a" : 2 } "#, r#"{"a":2}"#),
            (r#""\u00e9\ud83d\ude00\/\u001f""#, "\"é😀/\\u001f\""),
            (r#"[1E400, 0.1e-6]"#, "[null,1e-7]"),
            (r#"{"2":0,"x":0,"1":0}"#, r#"{"1":0,"2":0,"x":0}"#),
        ];
        for (text, want) in cases {
            assert_eq!(stringify(&parse(text).unwrap()).unwrap(), want, "{text}");
        }
    }

    #[test]
    fn rejects_what_js_rejects() {
        for text in [
            "",
            "01",
            "1.",
            ".5",
            "+1",
            "[1,]",
            "{\"a\":1,}",
            "'a'",
            "\"\t\"",
            "\"\\x\"",
            "nul",
            "1 2",
            "NaN",
        ] {
            assert!(parse(text).is_err(), "{text:?}");
        }
    }

    #[test]
    fn lone_surrogates_become_replacement_characters() {
        let v = parse(r#""\ud83d!\ude00""#).unwrap();
        assert_eq!(v.as_str(), Some("\u{FFFD}!\u{FFFD}"));
    }
}
