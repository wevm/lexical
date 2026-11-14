use crate::case::{Case, Outcome, same};
use lexical::dom::{CssPropertyTraits, css_property_names, css_property_traits, get_document};
use lexical::json::Value;

/// What `element.style.setProperty(name, value)` reads back, as
/// `css.test.ts`'s `probe` asks jsdom.
fn probe(name: &str, value: &str) -> String {
    let element = get_document().create_element("div");
    element.style().set_property(name, value, "");
    element.style().get_property_value(name)
}

fn bool_field(case: &Value, key: &str) -> Result<bool, String> {
    case.field(key)?
        .as_bool()
        .ok_or_else(|| format!("`{key}` is not a boolean"))
}

/// `css/properties`: each property an inline style accepts in jsdom is
/// accepted, and normalizes hex colors, a bare `0`, an identifier's case and
/// an unparsed value as jsdom's does; the generated table
/// (`rust/scripts/generate-dom-tables.mjs`) holds the same properties.
pub fn check_property(case: &Value, file: &Value) -> Result<Outcome, String> {
    let name = case.str("name")?;
    let color = bool_field(case, "color")?;
    let zero_length = bool_field(case, "zeroLength")?;
    let preserve_case = bool_field(case, "preserveCase")?;
    let verbatim = bool_field(case, "verbatim")?;
    let bits = u8::from(color)
        | u8::from(zero_length) << 1
        | u8::from(preserve_case) << 2
        | u8::from(verbatim) << 3;
    same(
        "table traits",
        css_property_traits(name),
        Some(CssPropertyTraits(bits)),
    )?;
    same(
        "properties in the table",
        css_property_names().count(),
        file.arr("cases")?.len(),
    )?;
    same(
        "accepts `initial`",
        probe(name, "initial").is_empty(),
        false,
    )?;
    same(
        "color",
        probe(name, "#f00") == "rgb(255, 0, 0)"
            || probe(name, "1px solid #f00") == "1px solid rgb(255, 0, 0)",
        color,
    )?;
    same("preserveCase", probe(name, "Foo") == "Foo", preserve_case)?;
    same(
        "verbatim",
        probe(name, "1PX 1px RED") == "1PX 1px RED",
        verbatim,
    )?;
    same("zeroLength", probe(name, "0") == "0px", zero_length)?;
    Ok(Outcome::Pass)
}

/// `css/values`: inline CSS text serializes back as the `cssText` jsdom
/// gives, in the declaration and in the `style` attribute.
pub fn check_value(case: &Value, _file: &Value) -> Result<Outcome, String> {
    let css = case.str("css")?;
    let css_text = case.str("cssText")?;
    let element = get_document().create_element("span");
    element.style().set_css_text(css);
    same("cssText", element.style().css_text().as_str(), css_text)?;
    same(
        "style attribute",
        element.get_attribute("style").as_deref(),
        Some(css_text),
    )?;
    Ok(Outcome::Pass)
}
