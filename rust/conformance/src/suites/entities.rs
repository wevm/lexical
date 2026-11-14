use crate::case::{Case, Outcome, same};
use lexical::json::{Object, Value};
use lexical::link::{LinkMatcherResult, auto_link_email_matcher, auto_link_url_matcher};

/// A `LinkMatcherResult` as `JSON.stringify` writes it (the built-in
/// matchers set no `attributes`).
fn result_json(result: Option<LinkMatcherResult>) -> Value {
    let Some(result) = result else {
        return Value::Null;
    };
    let mut object = Object::new();
    object.insert("index", (result.index as f64).into());
    object.insert("length", (result.length as f64).into());
    object.insert("text", result.text.as_str().into());
    object.insert("url", result.url.as_str().into());
    Value::Object(object)
}

/// `entities/autolink-match`: what `autoLinkUrlMatcher` and
/// `autoLinkEmailMatcher` return for each text.
pub fn check_autolink_match(case: &Value, _file: &Value) -> Result<Outcome, String> {
    let text = case.str("text")?;
    for (field, result) in [
        ("url", auto_link_url_matcher(text)),
        ("email", auto_link_email_matcher(text)),
    ] {
        same(
            field,
            result_json(result).to_string(),
            case.field(field)?.to_string(),
        )?;
    }
    Ok(Outcome::Pass)
}
