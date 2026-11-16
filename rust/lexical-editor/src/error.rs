use crate::json::Value;
use std::fmt;
use std::rc::Rc;

/// A JS `invariant` failure (or another error JS throws), carrying the same
/// message. Invariants that only a bug in a node implementation can break
/// panic instead, as `RefCell` does; the ones a document can break return
/// this.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexicalError {
    message: String,
    warning: Option<Rc<LexicalWarning>>,
}

impl LexicalError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            warning: None,
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    /// `warning` as the error `onWarn` receives, with its message.
    pub fn from_warning(warning: LexicalWarning) -> Self {
        Self {
            message: warning.message.clone(),
            warning: Some(Rc::new(warning)),
        }
    }

    /// The [`LexicalWarning`] this error is, as JS's
    /// `error instanceof LexicalWarning`.
    pub fn warning(&self) -> Option<&LexicalWarning> {
        self.warning.as_deref()
    }
}

/// `LexicalWarning`: a warning with a stable code, passed to the editor's
/// `onWarn` (as the [`LexicalError`] [`LexicalError::from_warning`] makes,
/// whose [`warning`](LexicalError::warning) it is) by packages that report
/// recoverable, warn-level conditions. The code is namespaced by the package that reports it
/// (`"@example/pkg/some-condition"`); a `match` over codes needs a default
/// arm, since packages may add codes in any release.
#[derive(Clone, Debug, PartialEq)]
pub struct LexicalWarning {
    /// What happened, namespaced by the package that reports it.
    pub code: String,
    /// For people; it may change.
    pub message: String,
    /// Details for programs, documented with each code.
    pub data: Option<Value>,
}

impl Eq for LexicalWarning {}

impl LexicalWarning {
    /// `name`.
    pub const NAME: &'static str = "LexicalWarning";

    /// `new LexicalWarning(code, message, data)`.
    pub fn new(code: impl Into<String>, message: impl Into<String>, data: Option<Value>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            data,
        }
    }
}

impl fmt::Display for LexicalWarning {
    /// `String(warning)`: `LexicalWarning: <message>`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", Self::NAME, self.message)
    }
}

impl std::error::Error for LexicalWarning {}

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
