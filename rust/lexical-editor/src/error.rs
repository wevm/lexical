use std::fmt;

/// A JS `invariant` failure (or another error JS throws), carrying the same
/// message. Invariants that only a bug in a node implementation can break
/// panic instead, as `RefCell` does; the ones a document can break return
/// this.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexicalError {
    message: String,
}

impl LexicalError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for LexicalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LexicalError {}

/// `invariant(cond, message, ...args)`: returns a [`LexicalError`] from the
/// enclosing function unless `cond` holds.
macro_rules! invariant {
    ($cond:expr, $($message:tt)+) => {
        if !$cond {
            return Err($crate::LexicalError::new(format!($($message)+)));
        }
    };
}
pub(crate) use invariant;
