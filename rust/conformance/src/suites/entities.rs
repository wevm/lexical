use crate::case::{Case, Outcome, same};
use lexical::hashtag::{HashtagConfig, get_hashtag_match, hashtag_nodes, register_lexical_hashtag};
use lexical::json::{Object, Value};
use lexical::link::{LinkMatcherResult, auto_link_email_matcher, auto_link_url_matcher};
use lexical::{
    CreateEditorArgs, Editor, NodeRef, UpdateOptions, create_editor, create_paragraph_node,
    create_text_node, get_root, get_selection,
};

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

/// `entities/hashtag-match`: what `HashtagExtension`'s default
/// `getHashtagMatch` returns for each text, as `JSON.stringify` writes it.
pub fn check_hashtag_match(case: &Value, _file: &Value) -> Result<Outcome, String> {
    let text = case.str("text")?;
    let got = match get_hashtag_match(text) {
        None => Value::Null,
        Some(found) => {
            let mut object = Object::new();
            object.insert("end", (found.end as f64).into());
            object.insert("start", (found.start as f64).into());
            Value::Object(object)
        }
    };
    same("match", got.to_string(), case.field("match")?.to_string())?;
    Ok(Outcome::Pass)
}

/// The editor of `entities.test.ts`'s `build()`,
/// `buildEditorFromExtensions` with `HashtagExtension`: its nodes and
/// `registerLexicalHashtag` with the default config.
fn build() -> Editor {
    let editor = create_editor(CreateEditorArgs {
        nodes: hashtag_nodes(),
        ..Default::default()
    });
    let _unregister = register_lexical_hashtag(&editor, HashtagConfig::default());
    editor
}

/// A discrete `editor.update(f)`.
fn update(
    editor: &Editor,
    f: impl FnOnce() -> Result<(), lexical::LexicalError> + 'static,
) -> Result<(), String> {
    editor
        .update(
            f,
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .map_err(|e| format!("update: {e}"))
}

/// `entities/hashtag-documents`: a paragraph of the case's text pieces (in
/// their formats), set in one update, leaves the JSON JS's transforms left.
pub fn check_hashtag_documents(case: &Value, _file: &Value) -> Result<Outcome, String> {
    let pieces = case
        .arr("pieces")?
        .iter()
        .map(|piece| {
            let format = piece.get("format").and_then(Value::as_f64).unwrap_or(0.0);
            Ok((piece.str("text")?.to_owned(), format))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let editor = build();
    update(&editor, move || {
        let root = get_root();
        root.clear()?;
        let paragraph = create_paragraph_node();
        let texts: Vec<NodeRef> = pieces
            .iter()
            .map(|(text, format)| {
                let node = create_text_node(text);
                node.set_format(*format);
                node
            })
            .collect();
        paragraph.append(&texts)?;
        root.append(&[paragraph])
    })?;
    same(
        "json",
        editor
            .get_editor_state()
            .to_json(false)
            .to_string()
            .as_str(),
        case.str("json")?,
    )?;
    Ok(Outcome::Pass)
}

/// `root.getAllTextNodes()`: the text nodes under `node`, in document
/// order.
fn all_text_nodes(node: &NodeRef, out: &mut Vec<NodeRef>) {
    for child in node.get_children() {
        if child.is_text() {
            out.push(child);
        } else if child.is_element() {
            all_text_nodes(&child, out);
        }
    }
}

/// `entities/hashtag-typing`: the case's text typed a character (code
/// point) at a time at the end of an empty paragraph, one discrete update
/// each, leaves the JSON and the selection JS's transforms left (each point
/// as the index of its node among the text nodes, and its offset).
pub fn check_hashtag_typing(case: &Value, _file: &Value) -> Result<Outcome, String> {
    let text = case.str("text")?;
    let editor = build();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        root.append(&[create_paragraph_node()])?;
        root.select_end()?;
        Ok(())
    })?;
    for ch in text.chars() {
        update(&editor, move || {
            if let Some(selection) = get_selection().filter(|s| s.is_range()) {
                selection.insert_text(&ch.to_string())?;
            }
            Ok(())
        })?;
    }
    let selection = editor
        .read(|| {
            let Some(range) = get_selection().and_then(|s| s.range()) else {
                return Vec::new();
            };
            let mut texts = Vec::new();
            all_text_nodes(&get_root(), &mut texts);
            let index = |key: &str| {
                texts
                    .iter()
                    .position(|n| n.get_key() == key)
                    .map_or(-1.0, |i| i as f64)
            };
            vec![
                index(&range.anchor.key),
                range.anchor.offset as f64,
                index(&range.focus.key),
                range.focus.offset as f64,
            ]
        })
        .map_err(|e| format!("read: {e}"))?;
    same(
        "json",
        editor
            .get_editor_state()
            .to_json(false)
            .to_string()
            .as_str(),
        case.str("json")?,
    )?;
    let want = case
        .arr("selection")?
        .iter()
        .map(|v| v.as_f64().ok_or("selection is not a list of numbers"))
        .collect::<Result<Vec<f64>, _>>()?;
    same("selection", selection, want)?;
    Ok(Outcome::Pass)
}
