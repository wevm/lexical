use super::package_of;
use crate::case::{Case, Outcome, same};
use lexical::json::{self, Value};
use lexical::{CreateEditorArgs, create_editor, get_root};

/// The first node type in a serialized document that needs a package not
/// ported yet.
fn unported_package(node: &Value) -> Option<&'static str> {
    let own = node
        .get("type")
        .and_then(Value::as_str)
        .and_then(package_of);
    own.or_else(|| {
        node.get("children")
            .and_then(Value::as_array)?
            .iter()
            .find_map(unported_package)
    })
}

/// `property/documents.json`: a random document parses (or fails to, when
/// JS's did) to the same state, written in both forms, a second parse of
/// each form gives what JS's gave, and the text content is the same. The
/// Markdown and HTML JS recorded are checked once those packages are ported.
pub fn check_document(case: &Value, _file: &Value) -> Result<Outcome, String> {
    let input = case.str("input")?;
    let parsed_input = json::parse(input).map_err(|e| format!("input: {e}"))?;
    if let Some(package) = parsed_input.get("root").and_then(unported_package) {
        return Ok(Outcome::NotPorted(package));
    }
    let editor = create_editor(CreateEditorArgs {
        nodes: [
            lexical::rich_text::rich_text_nodes(),
            lexical::list::list_nodes(),
            lexical::link::link_nodes(),
            lexical::code::code_nodes(),
        ]
        .concat(),
    });
    let parsed = editor.parse_editor_state(input);
    if case.get("error").is_some() {
        return match parsed {
            Err(_) => Ok(Outcome::Pass),
            Ok(state) => Err(format!(
                "JS failed to parse, Rust parsed {}",
                state.to_json(false)
            )),
        };
    }
    let state = parsed.map_err(|e| format!("parsing the input: {e}"))?;
    let legacy = state.to_json(false).to_string();
    let compact = state.to_json(true).to_string();
    same("legacy", legacy.as_str(), case.str("legacy")?)?;
    same("compact", compact.as_str(), case.str("compact")?)?;

    // Parsing each form again gives `reparsed` when JS recorded it, and the
    // same two texts otherwise.
    let expected: Vec<(String, String)> = match case.get("reparsed") {
        Some(reparsed) => reparsed
            .as_array()
            .ok_or("reparsed is not an array")?
            .iter()
            .map(|pair| {
                let text = |i: usize| {
                    pair.as_array()
                        .and_then(|p| p.get(i))
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .ok_or("reparsed is not a list of pairs")
                };
                Ok((text(0)?, text(1)?))
            })
            .collect::<Result<_, &str>>()?,
        None => vec![(legacy.clone(), compact.clone()); 2],
    };
    for ((form, text), (want_legacy, want_compact)) in [("legacy", &legacy), ("compact", &compact)]
        .into_iter()
        .zip(&expected)
    {
        let again = editor
            .parse_editor_state(text)
            .map_err(|e| format!("parsing the {form} form again: {e}"))?;
        same(
            format_args!("legacy after parsing the {form} form"),
            &again.to_json(false).to_string(),
            want_legacy,
        )?;
        same(
            format_args!("compact after parsing the {form} form"),
            &again.to_json(true).to_string(),
            want_compact,
        )?;
    }

    // JS sets the state on the editor first, which may normalize it (merging
    // adjacent text) without changing its text.
    let text = state.read(|| get_root().get_text_content());
    same("text", text.as_str(), case.str("text")?)?;
    Ok(Outcome::Pass)
}
