use super::package_of;
use crate::case::{Case, Outcome, same};
use crate::scripts::{known_command, selection_snapshot};
use lexical::code::code_extension;
use lexical::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use lexical::hashtag::hashtag_extension;
use lexical::history::{HistoryConfigOverrides, history_extension};
use lexical::json::{self, Object, Value};
use lexical::link::link_extension;
use lexical::list::{check_list_extension, list_extension};
use lexical::markdown::transformers;
use lexical::rich_text::{
    HeadingTagType, create_heading_node, create_quote_node, rich_text_extension,
};
use lexical::selection::set_blocks_type;
use lexical::{
    CreateEditorArgs, Editor, HISTORY_MERGE_TAG, LexicalError, LexicalExtension, NodeClass,
    NodeRef, Point, PointType, SelectionGranularity, SelectionMovement, UpdateOptions, Which,
    config_extension, create_editor, create_paragraph_node, create_range_selection,
    define_extension, get_root, get_selection, reset_random_key, set_selection,
};
use std::rc::Rc;

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
/// each form gives what JS's gave, and the text content, the Markdown export
/// and the HTML are the same.
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
            lexical::overflow::overflow_nodes(),
            lexical::hashtag::hashtag_nodes(),
        ]
        .concat(),
        ..CreateEditorArgs::default()
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

    // JS sets the state on the editor first and reads the text there. A
    // parsed state is all dirty, so normalization (merging adjacent text)
    // and transforms run on it: `OverflowNode`'s removes an empty one.
    editor
        .set_editor_state(&state, None)
        .map_err(|e| format!("setting the state: {e}"))?;
    let text = editor
        .read(|| get_root().get_text_content())
        .map_err(|e| format!("reading the text: {e}"))?;
    same("text", text.as_str(), case.str("text")?)?;

    // `$convertToMarkdownString(TRANSFORMERS)`, null when it threw.
    let markdown = editor
        .read(|| lexical::markdown::convert_to_markdown_string(&transformers(), None, false))
        .map_err(|e| format!("reading the markdown: {e}"))?;
    match (case.field("markdown")?.as_str(), markdown) {
        (Some(want), Ok(got)) => same("markdown", got.as_str(), want)?,
        (None, Err(_)) => {}
        (want, got) => return Err(format!("markdown: Rust {got:?}, JS {want:?}")),
    }

    // `$generateHtmlFromNodes`, or null when it throws.
    let html = editor
        .read(|| lexical::html::generate_html_from_nodes(&editor, None))
        .map_err(|e| format!("reading the HTML: {e}"))?
        .ok();
    let want_html = match case.field("html")? {
        Value::Null => None,
        _ => Some(case.str("html")?),
    };
    same("html", html.as_deref(), want_html)?;
    Ok(Outcome::Pass)
}

/// The jsdom window `sequenceEditor` gives its editor in property.ts. It
/// is installed after Lexical has loaded, so `CAN_USE_DOM` is false and
/// `getDOMSelection` gives no selection: the deletion commands resolve in
/// the model (Swift's runner gives its editor a movement that never lands,
/// to the same end).
struct WindowWithoutSelection;

impl SelectionMovement for WindowWithoutSelection {
    fn landed_point(
        &self,
        _from: &Point,
        _is_backward: bool,
        _granularity: SelectionGranularity,
    ) -> Result<Option<Point>, LexicalError> {
        Ok(None)
    }

    fn has_dom_selection(&self) -> bool {
        false
    }
}

/// `sequenceEditor()` in property.ts: `buildEditorFromExtensions` of a root
/// extension whose dependencies are `RichTextExtension`, `ListExtension`,
/// `CheckListExtension`, `LinkExtension`, `HashtagExtension`,
/// `CodeExtension` and `HistoryExtension` with `{delay: 0, disabled:
/// false}`, in that order, and whose nodes are `AutoLinkNode` and
/// `OverflowNode`. Its `namespace: ''` and `onError` are editor options a
/// Rust editor doesn't take (rust/PARITY.md); and the window JS gives it is
/// the editor's selection movement.
fn sequence_editor() -> LexicalEditorWithDispose {
    reset_random_key();
    let root = define_extension(LexicalExtension::<()> {
        dependencies: vec![
            rich_text_extension().into(),
            list_extension().into(),
            check_list_extension().into(),
            link_extension().into(),
            hashtag_extension().into(),
            code_extension().into(),
            // No time-based merging, so history doesn't depend on the clock.
            config_extension(
                &history_extension(),
                [HistoryConfigOverrides {
                    delay: Some(0.0),
                    disabled: Some(false),
                    ..HistoryConfigOverrides::default()
                }],
            ),
        ],
        nodes: vec![
            NodeClass::of::<lexical::link::AutoLinkNode>(),
            NodeClass::of::<lexical::overflow::OverflowNode>(),
        ],
        ..LexicalExtension::new("[root]", ())
    });
    let editor =
        build_editor_from_extensions(vec![root.into()]).expect("sequenceEditor's extensions build");
    editor.set_selection_movement(Some(Rc::new(WindowWithoutSelection)));
    editor
}

/// A step's `anchor` or `focus`: `[key, offset, type]`.
fn point_spec(step: &Value, which: &str) -> Result<(String, usize, PointType), String> {
    let spec = step.arr(which)?;
    let bad = || {
        format!(
            "bad {which} {}",
            step.field(which).map_or(String::new(), Value::to_string)
        )
    };
    let key = spec.first().and_then(Value::as_str).ok_or_else(bad)?;
    let offset = spec.get(1).and_then(Value::as_f64).ok_or_else(bad)?;
    let point_type = match spec.get(2).and_then(Value::as_str) {
        Some("text") => PointType::Text,
        Some("element") => PointType::Element,
        _ => return Err(bad()),
    };
    Ok((key.to_owned(), offset as usize, point_type))
}

/// `$blockCreator(blocks)` in property.ts.
fn block_creator(blocks: &str) -> Box<dyn Fn() -> NodeRef> {
    match blocks {
        "quote" => Box::new(|| create_quote_node(false)),
        "h1" => Box::new(|| create_heading_node(HeadingTagType::H1)),
        "h2" => Box::new(|| create_heading_node(HeadingTagType::H2)),
        _ => Box::new(create_paragraph_node),
    }
}

/// `runStep(editor, step)` in property.ts: selects the step's points in one
/// discrete update (running `$setBlocksType` for a `blocks` step), then
/// dispatches its command outside any update, and records the result in
/// JS's key order.
fn run_step(editor: &Editor, step: &Value) -> Result<Object, String> {
    let anchor = point_spec(step, "anchor")?;
    let focus = point_spec(step, "focus")?;
    let blocks = step
        .get("blocks")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let command = match step.get("command").and_then(Value::as_str) {
        None => None,
        Some(name) => Some(known_command(name).ok_or_else(|| format!("unknown command {name}"))?),
    };
    let mut handled = None;
    let outcome = editor
        .update(
            move || {
                let selection = create_range_selection();
                selection.point_set(Which::Anchor, &anchor.0, anchor.1, anchor.2)?;
                selection.point_set(Which::Focus, &focus.0, focus.1, focus.2)?;
                set_selection(Some(selection));
                if let Some(blocks) = &blocks {
                    set_blocks_type(get_selection().as_ref(), block_creator(blocks), None)?;
                }
                Ok(())
            },
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .and_then(|()| match &command {
            None => Ok(()),
            Some(command) => {
                let payload = step.get("payload").cloned().unwrap_or(Value::Null);
                handled = Some(command.dispatch(editor, payload)?);
                Ok(())
            }
        });
    // A dispatch outside an update commits at the next read.
    let (selection, text) = editor
        .read(|| (selection_snapshot(), get_root().get_text_content()))
        .map_err(|e| format!("committing: {e}"))?;
    let state = editor.get_editor_state();
    let mut result = Object::new();
    result.insert("compact", state.to_json(true).to_string().into());
    result.insert("selection", selection);
    result.insert("state", state.to_json(false).to_string().into());
    result.insert("text", text.into());
    if let Some(handled) = handled {
        result.insert("handled", handled.into());
    }
    if let Err(error) = outcome {
        result.insert("error", true.into());
        // Not a field JS records: why the step failed, for the report.
        result.insert("thrown", error.to_string().into());
    }
    Ok(result)
}

/// The fields of a step's result, in the order a mismatch is reported.
const STEP_FIELDS: [&str; 6] = ["error", "handled", "text", "selection", "state", "compact"];

/// `got` and `want` from the first place they differ, for long strings.
fn difference(got: &str, want: &str) -> String {
    const CONTEXT: usize = 60;
    let common = got
        .char_indices()
        .zip(want.chars())
        .find(|((_, a), b)| a != b)
        .map_or(got.len().min(want.len()), |((i, _), _)| i);
    let start = got[..common]
        .char_indices()
        .rev()
        .nth(CONTEXT)
        .map_or(0, |(i, _)| i);
    let window = |s: &str| -> String { s[start..].chars().take(3 * CONTEXT).collect() };
    format!(
        "differ at byte {common}: Rust …{:?}, JS …{:?}",
        window(got),
        window(want)
    )
}

/// `property/sequences.json`: `runSequence` in property.ts, from the
/// concrete steps JS recorded. After each step, whether it threw, what
/// `dispatchCommand` returned, the text, the selection and the state in both
/// forms match JS. The first step that differs is reported, with every
/// field that differs in it; later steps diverge with it.
pub fn check_sequence(case: &Value, _file: &Value) -> Result<Outcome, String> {
    let document = case.str("document")?;
    let steps = case.arr("steps")?;
    let results = case.arr("results")?;
    same("steps recorded", results.len(), steps.len())?;
    let editor = sequence_editor();
    let outcome = (|| {
        let state = editor
            .parse_editor_state(document)
            .map_err(|e| format!("parsing the document: {e}"))?;
        editor
            .set_editor_state(&state, Some(HISTORY_MERGE_TAG))
            .map_err(|e| format!("setting the document: {e}"))?;
        for (i, (step, expected)) in steps.iter().zip(results).enumerate() {
            let got = run_step(&editor, step).map_err(|e| format!("step {i}: {e}"))?;
            let mismatches: Vec<String> = STEP_FIELDS
                .iter()
                .filter(|field| got.get(field) != expected.get(field))
                .map(|field| {
                    let show = |v: Option<&Value>| v.map_or("absent".to_owned(), Value::to_string);
                    let detail = match (got.get(field), expected.get(field)) {
                        (Some(Value::String(a)), Some(Value::String(b)))
                            if a.len() + b.len() > 200 =>
                        {
                            difference(a, b)
                        }
                        (a, b) => format!("Rust {}, JS {}", show(a), show(b)),
                    };
                    let thrown = match (*field, got.get("thrown")) {
                        ("error", Some(thrown)) => format!(" (Rust threw {thrown})"),
                        _ => String::new(),
                    };
                    format!("{field}: {detail}{thrown}")
                })
                .collect();
            if !mismatches.is_empty() {
                return Err(format!("step {i} ({step}): {}", mismatches.join("; ")));
            }
        }
        Ok(Outcome::Pass)
    })();
    // The history's entries hold the editor until it is disposed.
    editor.dispose();
    outcome
}
