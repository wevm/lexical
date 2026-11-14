use crate::case::{Case, Outcome, same};
use lexical::json::Value;
use lexical::{CreateEditorArgs, create_editor};

/// `serialization/*.json`: the document parses from both forms and writes
/// both back byte for byte. The editor has the list, link, code, overflow,
/// hashtag and mark nodes, for the `serialization/*` file of each package.
pub fn check(case: &Value, _file: &Value) -> Result<Outcome, String> {
    let legacy = case.str("legacy")?;
    let compact = case.str("compact")?;
    let editor = create_editor(CreateEditorArgs {
        nodes: [
            lexical::list::list_nodes(),
            lexical::link::link_nodes(),
            lexical::code::code_nodes(),
            lexical::overflow::overflow_nodes(),
            lexical::hashtag::hashtag_nodes(),
            lexical::mark::mark_nodes(),
        ]
        .concat(),
        ..CreateEditorArgs::default()
    });
    for (form, input) in [("legacy", legacy), ("compact", compact)] {
        let state = editor
            .parse_editor_state(input)
            .map_err(|e| format!("parsing the {form} form: {e}"))?;
        same(
            format_args!("legacy from {form}"),
            state.to_json(false).to_string().as_str(),
            legacy,
        )?;
        same(
            format_args!("compact from {form}"),
            state.to_json(true).to_string().as_str(),
            compact,
        )?;
    }
    Ok(Outcome::Pass)
}
