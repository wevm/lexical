use crate::case::{Case, Outcome, same};
use lexical::dom::{DomNode, is_legacy_entity_name, named_entity, parse_from_string};
use lexical::json::Value;

/// `html/parse`: `DOMParser` builds the same document as jsdom's, compared
/// by the `innerHTML` of its head and body.
pub fn check_parse(case: &Value, _file: &Value) -> Result<Outcome, String> {
    let document = parse_from_string(case.str("html")?);
    let inner = |e: Option<DomNode>| e.map(|e| e.inner_html());
    same(
        "head",
        inner(document.head()).as_deref(),
        Some(case.str("head")?),
    )?;
    same(
        "body",
        inner(document.body()).as_deref(),
        Some(case.str("body")?),
    )?;
    Ok(Outcome::Pass)
}

/// `html/entities`: each named character reference decodes to the text
/// jsdom gives, with and (for a legacy name) without its semicolon, and the
/// generated table (`rust/scripts/generate-dom-tables.mjs`) agrees.
pub fn check_entity(case: &Value, file: &Value) -> Result<Outcome, String> {
    let name = case.str("name")?;
    let value = case.str("value")?;
    let legacy = case.field("legacy")?.as_bool().ok_or("legacy")?;
    same("table value", named_entity(name), Some(value))?;
    same("table legacy", is_legacy_entity_name(name), legacy)?;
    let count = file.arr("cases")?.len();
    let table = lexical::dom::named_entity_count();
    same("names in the table", table, count)?;
    // Without its semicolon, a name that isn't legacy may still start with
    // one that is (`&centerdot` is `&cent` + "erdot").
    let (html, expected) = if legacy {
        (
            format!("<p>&{name};|&{name}</p>"),
            format!("{value}|{value}"),
        )
    } else {
        (format!("<p>&{name};</p>"), value.to_owned())
    };
    let body = parse_from_string(&html).body().ok_or("no body")?;
    same("textContent", body.text_content(), expected)?;
    Ok(Outcome::Pass)
}
