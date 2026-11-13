use lexical::json::{Object, Value};

/// How a case matched JS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Exactly.
    Pass,
    /// Only under the named documented exception (rust/PARITY.md).
    Exception(&'static str),
    /// Not checked: the case needs the named upstream package, which isn't
    /// ported yet.
    NotPorted(&'static str),
}

/// Reading a case's fields, with errors that name the missing field.
pub trait Case {
    fn field(&self, key: &str) -> Result<&Value, String>;
    fn str(&self, key: &str) -> Result<&str, String>;
    fn arr(&self, key: &str) -> Result<&Vec<Value>, String>;
    fn obj(&self, key: &str) -> Result<&Object, String>;
}

fn want<T>(v: Option<T>, key: &str, kind: &str, case: &Value) -> Result<T, String> {
    v.ok_or_else(|| {
        let text: String = case.to_string().chars().take(200).collect();
        format!("vector field `{key}` is not {kind} in {text}")
    })
}

impl Case for Value {
    fn field(&self, key: &str) -> Result<&Value, String> {
        want(self.get(key), key, "present", self)
    }
    fn str(&self, key: &str) -> Result<&str, String> {
        want(self.get(key).and_then(Value::as_str), key, "a string", self)
    }
    fn arr(&self, key: &str) -> Result<&Vec<Value>, String> {
        want(
            self.get(key).and_then(Value::as_array),
            key,
            "an array",
            self,
        )
    }
    fn obj(&self, key: &str) -> Result<&Object, String> {
        want(
            self.get(key).and_then(Value::as_object),
            key,
            "an object",
            self,
        )
    }
}

/// Fails with "Rust <got>, JS <want>" unless they're equal.
pub fn same<T: PartialEq + std::fmt::Debug>(
    what: impl std::fmt::Display,
    got: T,
    want: T,
) -> Result<(), String> {
    if got == want {
        Ok(())
    } else {
        Err(format!("{what}: Rust {got:?}, JS {want:?}"))
    }
}
