//! JSON as JS reads and writes it: `JSON.parse`, `JSON.stringify` with JS's
//! property order and number formatting, and `undefined`. Lexical's
//! serialized documents must be byte-identical to what JS writes, which
//! `serde_json` doesn't promise (key order, `1e+21`, `-0`), and the crate has
//! no third-party dependencies (rust/DESIGN.md).

mod number;
mod parse;
mod stringify;
mod value;

pub use number::number_to_string;
pub use parse::{ParseError, parse};
pub use stringify::stringify;
pub use value::{Object, Value};
