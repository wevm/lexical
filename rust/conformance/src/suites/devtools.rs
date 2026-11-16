use super::package_of;
use crate::case::{Case, Outcome, same};
use lexical::devtools::generate_content;
use lexical::json::{self, Value};
use lexical::{
    CreateEditorArgs, Editor, LexicalError, NodeRef, PointType, UpdateOptions, Which,
    create_editor, create_node_selection, get_root, set_selection,
};
use std::collections::HashMap;

/// The first node type in a serialized document that needs a package not
/// ported yet.
fn unported_package(node: &Value) -> Option<&'static str> {
    node.get("type")
        .and_then(Value::as_str)
        .and_then(package_of)
        .or_else(|| {
            node.get("children")
                .and_then(Value::as_array)?
                .iter()
                .find_map(unported_package)
        })
}

fn err(message: impl Into<String>) -> LexicalError {
    LexicalError::new(message.into())
}

/// The node at a path of child indexes from the root.
fn node_at(path: &Value) -> Result<NodeRef, LexicalError> {
    let mut node = get_root();
    for index in path.as_array().ok_or_else(|| err("a path is an array"))? {
        let index = index.as_f64().ok_or_else(|| err("a path index"))? as usize;
        node = node
            .get_children()
            .get(index)
            .cloned()
            .ok_or_else(|| err(format!("no child {index}")))?;
    }
    Ok(node)
}

fn point_type(point: &Value) -> Result<PointType, LexicalError> {
    match point.get("type").and_then(Value::as_str) {
        Some("text") => Ok(PointType::Text),
        Some("element") => Ok(PointType::Element),
        other => Err(err(format!("point type {other:?}"))),
    }
}

/// `treeView.test.ts`'s `$applySelection` for the selections it recorded
/// (each applies): none, a node selection of paths, or a range selection
/// made with `anchor.select(0, 0)` and its points set.
fn apply_selection(selection: &Value) -> Result<(), LexicalError> {
    if selection.is_null() {
        set_selection(None);
        return Ok(());
    }
    if selection.get("type").and_then(Value::as_str) == Some("node") {
        let nodes = create_node_selection();
        for path in selection
            .get("paths")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            nodes.add(node_at(path)?.get_key());
        }
        set_selection(Some(nodes));
        return Ok(());
    }
    let anchor = selection.get("anchor").ok_or_else(|| err("no anchor"))?;
    let focus = selection.get("focus").ok_or_else(|| err("no focus"))?;
    let anchor_node = node_at(anchor.get("path").unwrap_or(&Value::Null))?;
    let focus_node = node_at(focus.get("path").unwrap_or(&Value::Null))?;
    let range = anchor_node.select(Some(0), Some(0))?;
    for (which, point, node) in [
        (Which::Anchor, anchor, &anchor_node),
        (Which::Focus, focus, &focus_node),
    ] {
        let offset = point
            .get("offset")
            .and_then(Value::as_f64)
            .ok_or_else(|| err("no offset"))?;
        range.point_set(which, node.get_key(), offset as usize, point_type(point)?)?;
    }
    if let Some(format) = selection.get("format").and_then(Value::as_f64) {
        range.set_format(format);
    }
    if let Some(style) = selection.get("style").and_then(Value::as_str) {
        range.set_style(style);
    }
    Ok(())
}

/// The run of ASCII digits at the start of `s`.
fn digits(s: &str) -> &str {
    &s[..s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len())]
}

/// The key a node line prints, `^[> ] [ |├└]* \((\d+)\) `, if it is one.
fn line_key(line: &str) -> Option<&str> {
    let rest = line.strip_prefix(['>', ' '])?.strip_prefix(' ')?;
    let rest = rest.trim_start_matches([' ', '|', '├', '└']);
    let rest = rest.strip_prefix('(')?;
    let key = digits(rest);
    (!key.is_empty() && rest[key.len()..].starts_with(") ")).then_some(key)
}

/// Replaces each `<prefix><digits><suffix>` with the digits mapped.
fn map_keys(text: &str, prefix: &str, suffix: &str, key: &HashMap<&str, String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find(prefix) {
        out.push_str(&rest[..i + prefix.len()]);
        rest = &rest[i + prefix.len()..];
        let found = digits(rest);
        if !found.is_empty() && rest[found.len()..].starts_with(suffix) {
            out.push_str(key.get(found).map_or("?", String::as_str));
            rest = &rest[found.len()..];
        }
    }
    out.push_str(rest);
    out
}

/// The JS tree with its keys replaced by the port's and its version by the
/// port's, as Swift's `TreeViewTests` does.
fn expected(tree: &str, version: &str, key: &HashMap<&str, String>) -> String {
    // A `^` line is padded by its node's printed key, whose width differs
    // when the keys do.
    let mut lines: Vec<String> = Vec::new();
    let mut width_change: isize = 0;
    for line in tree.split('\n') {
        if let Some(js_key) = line_key(line) {
            let rust_len = key.get(js_key).map_or(js_key.len(), String::len);
            width_change = js_key.len() as isize - rust_len as isize;
            lines.push(line.to_owned());
        } else if let (true, Some(caret)) = (line.starts_with('>'), line.find(" ^")) {
            // The run of spaces before the first `^`.
            let start = line[..=caret].trim_end_matches(' ').len();
            let kept = start + width_change.max(0) as usize;
            lines.push(format!(
                "{}{}{}",
                &line[..start],
                " ".repeat((-width_change).max(0) as usize),
                &line[kept..]
            ));
        } else {
            lines.push(line.to_owned());
        }
    }
    let tree = lines
        .join("\n")
        .replace(&format!("(v{version})"), &format!("(v{})", Editor::VERSION));
    let tree = map_keys(&tree, "(", ")", key);
    let tree = map_keys(&tree, "key: ", "", key);
    // A node selection's `└ [k1, k2]`.
    match tree.find("└ [") {
        Some(i) => {
            let open = i + "└ [".len();
            let close = open + tree[open..].find(']').unwrap_or(0);
            let mapped: Vec<&str> = tree[open..close]
                .split(", ")
                .filter(|k| !k.is_empty())
                .map(|k| key.get(k).map_or("?", String::as_str))
                .collect();
            format!("{}{}{}", &tree[..open], mapped.join(", "), &tree[close..])
        }
        None => tree,
    }
}

/// `devtools/tree`: `generateContent(editor, [], false)` of the parsed
/// state, with the selection set after parsing and the editable flag,
/// matches JS's tree with JS's node keys mapped to the port's (both listed
/// in tree order) and the port's version.
pub fn check_tree(case: &Value, _file: &Value) -> Result<Outcome, String> {
    let editor_state = case.str("editorState")?;
    let parsed = json::parse(editor_state).map_err(|e| format!("editorState: {e}"))?;
    if let Some(package) = parsed.get("root").and_then(unported_package) {
        return Ok(Outcome::NotPorted(package));
    }
    let editable = case
        .field("editable")?
        .as_bool()
        .ok_or("vector field `editable` is not a boolean")?;
    let editor = create_editor(CreateEditorArgs {
        nodes: lexical::rich_text::rich_text_nodes(),
        namespace: "playground".to_owned(),
    });
    let state = editor
        .parse_editor_state(editor_state)
        .map_err(|e| format!("parsing: {e}"))?;
    editor
        .set_editor_state(&state, None)
        .map_err(|e| format!("setEditorState: {e}"))?;
    let selection = case.field("selection")?.clone();
    editor
        .update(
            move || apply_selection(&selection),
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .map_err(|e| format!("setting the selection: {e}"))?;
    editor.set_editable(editable);
    let keys = editor
        .read(|| {
            fn visit(node: &NodeRef, keys: &mut Vec<String>) {
                for child in node.get_children() {
                    keys.push(child.get_key().to_owned());
                    visit(&child, keys);
                }
            }
            let mut keys = Vec::new();
            visit(&get_root(), &mut keys);
            keys
        })
        .map_err(|e| format!("reading: {e}"))?;
    let js_keys = case
        .arr("keys")?
        .iter()
        .map(|k| k.as_str().ok_or("a key is not a string"))
        .collect::<Result<Vec<_>, _>>()?;
    same("nodes", keys.len(), js_keys.len())?;
    let key: HashMap<&str, String> = js_keys.into_iter().zip(keys).collect();
    let tree = generate_content(&editor, &[], false, None, false)
        .map_err(|e| format!("generateContent: {e}"))?;
    same(
        "tree",
        tree.as_str(),
        expected(case.str("tree")?, case.str("version")?, &key).as_str(),
    )?;
    Ok(Outcome::Pass)
}
