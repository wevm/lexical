use crate::case::{Case, Outcome, same};
use lexical::json::Value;
use lexical::{CreateEditorArgs, EditorState, NodeRef, create_editor, get_root};

/// `serialization/*.json`: the document parses from both forms and writes
/// both back byte for byte, and each node's generated JSON is what the
/// walk of its class's schema writes ([`generated_agrees_with_schema`]). The editor has the list, link, code, overflow,
/// hashtag and mark nodes and `TestDecoratorBlockNode`, for the `serialization/*` file of each package.
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
            crate::scripts::test_decorator_block_nodes(),
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
        generated_agrees_with_schema(&state).map_err(|e| format!("from {form}: {e}"))?;
    }
    Ok(Outcome::Pass)
}

/// Whether the generated JSON of every node of `state` (the class's own
/// `exportJSON` code, `LexicalGeneratedJSON.ts` and its counterparts) is
/// what the walk of its schema writes, in both forms, as
/// `LexicalGeneratedJSON.test.ts` asserts in JS.
pub fn generated_agrees_with_schema(state: &EditorState) -> Result<(), String> {
    fn visit(node: &NodeRef) -> Result<(), String> {
        let latest = node.get_latest();
        for compact in [false, true] {
            if let Some(generated) = latest.generated_export_json(compact) {
                let walked = lexical::schema::walk_export_json(&*latest, compact);
                same(
                    format_args!(
                        "the {} schema of {} (compact: {compact})",
                        latest.get_type(),
                        node.get_key()
                    ),
                    Value::Object(walked).to_string(),
                    Value::Object(generated).to_string(),
                )?;
            }
        }
        node.get_children().iter().try_for_each(visit)
    }
    state.read(|| visit(&get_root()))
}
