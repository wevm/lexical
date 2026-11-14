use crate::case::{Case, Outcome, same};
use lexical::dom::{DomNode, is_legacy_entity_name, named_entity, parse_from_string};
use lexical::html::{generate_html_from_nodes, generate_nodes_from_dom};
use lexical::json::Value;
use lexical::{CreateEditorArgs, NodeRef, UpdateOptions, create_editor};
use std::cell::RefCell;
use std::rc::Rc;

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

/// `html/export`: `$generateHtmlFromNodes` writes the HTML jsdom gave for
/// the document, with the case's theme.
pub fn check_export(case: &Value, _file: &Value) -> Result<Outcome, String> {
    let theme = match case.get("theme") {
        Some(Value::Object(theme)) => theme.clone(),
        _ => lexical::json::Object::new(),
    };
    let editor = create_editor(CreateEditorArgs {
        theme,
        ..Default::default()
    });
    let state = editor
        .parse_editor_state(case.str("editorState")?)
        .map_err(|e| format!("editorState: {e}"))?;
    editor
        .set_editor_state(&state, None)
        .map_err(|e| e.to_string())?;
    let html = editor
        .read(|| generate_html_from_nodes(&editor, None))
        .and_then(|html| html)
        .map_err(|e| e.to_string())?;
    same("html", html.as_str(), case.str("html")?)?;
    Ok(Outcome::Pass)
}

/// A node and its subtree as JSON, as `exportJSON` writes each node
/// (`$nodeToJSON`).
fn node_to_json(node: &NodeRef) -> Value {
    let mut json = node.export_json(false);
    if node.is_element() {
        json.insert(
            "children",
            Value::Array(node.get_children().iter().map(node_to_json).collect()),
        );
    }
    Value::Object(json)
}

/// `html/import`: `$generateNodesFromDOM` makes the nodes jsdom's import
/// made of the parsed HTML.
pub fn check_import(case: &Value, _file: &Value) -> Result<Outcome, String> {
    let editor = create_editor(CreateEditorArgs::default());
    let dom = parse_from_string(case.str("html")?);
    let nodes = Rc::new(RefCell::new(String::new()));
    let (out, update_editor) = (nodes.clone(), editor.clone());
    editor
        .update(
            move || {
                let nodes = generate_nodes_from_dom(&update_editor, &dom)?;
                *out.borrow_mut() =
                    Value::Array(nodes.iter().map(node_to_json).collect()).to_string();
                Ok(())
            },
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .map_err(|e| e.to_string())?;
    same("nodes", nodes.borrow().as_str(), case.str("nodes")?)?;
    Ok(Outcome::Pass)
}
