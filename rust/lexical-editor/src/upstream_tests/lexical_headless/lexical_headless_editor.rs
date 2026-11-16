//! Ports of `packages/lexical-headless/src/__tests__/unit/LexicalHeadlessEditor.test.ts`.
//!
//! Upstream's `update` awaits a microtask after `editor.update`, which
//! commits it; these update discretely. Its headless globals, `withDOM` and
//! the throwing `getRootElement` and `registerRootListener` are skipped
//! (rust/parity/skips.json).

use crate::headless::create_headless_editor;
use crate::upstream_tests::support::{update, update_with};
use crate::{
    COMMAND_PRIORITY_NORMAL, CONTROLLED_TEXT_INSERTION_COMMAND, CreateEditorArgs, Editor,
    EditorState, PointType, Which, create_paragraph_node, create_text_node, get_node_by_key,
    get_root, get_selection,
};
use std::cell::RefCell;
use std::rc::Rc;

/// `beforeEach`: `createHeadlessEditor({namespace: '', onError: throw})`.
fn headless_editor() -> Editor {
    create_headless_editor(CreateEditorArgs {
        namespace: String::new(),
        ..CreateEditorArgs::default()
    })
}

/// `assertEditorState`: the node map's values, in its order, each matching
/// the expected fields (`__key: 'root'`, `__type`, and a text node's
/// `__format` and `__text`).
fn node_summary(editor_state: &EditorState) -> Vec<String> {
    let keys = editor_state.keys_in_creation_order();
    editor_state.read(|| {
        keys.iter()
            .map(|key| {
                let node = get_node_by_key(key).unwrap();
                if key == "root" {
                    "root".to_owned()
                } else if node.is_text() {
                    let text = node.text_data();
                    format!("text:{}:{}", text.get_format(), text.get_text())
                } else {
                    node.get_type().to_owned()
                }
            })
            .collect()
    })
}

/// A paragraph of a bold `Hello` and `world`.
fn append_hello_world() -> Result<(), crate::LexicalError> {
    // In JS's order: the paragraph is created before its children.
    let paragraph = create_paragraph_node();
    let hello = create_text_node("Hello");
    hello.toggle_format("bold");
    paragraph.append(&[hello, create_text_node("world")])?;
    get_root().append(&[paragraph])?;
    Ok(())
}

// upstream: lexical-headless/src/__tests__/unit/LexicalHeadlessEditor.test.ts::LexicalHeadlessEditor > can update editor
#[test]
fn can_update_editor() {
    let editor = headless_editor();
    update(&editor, append_hello_world);
    assert_eq!(
        node_summary(&editor.get_editor_state()),
        ["root", "paragraph", "text:1:Hello", "text:0:world"]
    );
}

// upstream: lexical-headless/src/__tests__/unit/LexicalHeadlessEditor.test.ts::LexicalHeadlessEditor > can set editor state from json
#[test]
fn can_set_editor_state_from_json() {
    let editor = headless_editor();
    editor
        .set_editor_state(
            &editor
                .parse_editor_state(r#"{"root":{"children":[{"children":[{"detail":0,"format":1,"mode":"normal","style":"","text":"Hello","type":"text","version":1},{"detail":0,"format":0,"mode":"normal","style":"","text":"world","type":"text","version":1}],"direction":"ltr","format":"","indent":0,"type":"paragraph","version":1}],"direction":"ltr","format":"","indent":0,"type":"root","version":1}}"#)
                .unwrap(),
            None,
        )
        .unwrap();
    assert_eq!(
        node_summary(&editor.get_editor_state()),
        ["root", "paragraph", "text:1:Hello", "text:0:world"]
    );
}

// upstream: lexical-headless/src/__tests__/unit/LexicalHeadlessEditor.test.ts::LexicalHeadlessEditor > can register listeners
#[test]
fn can_register_listeners() {
    let editor = headless_editor();
    let updates = Rc::new(RefCell::new(0));
    let commands = Rc::new(RefCell::new(Vec::<String>::new()));
    let transformed = Rc::new(RefCell::new(Vec::<String>::new()));
    let text_contents = Rc::new(RefCell::new(Vec::<String>::new()));

    let heard = updates.clone();
    let _ = editor.register_update_listener(move |_| *heard.borrow_mut() += 1);
    let heard = commands.clone();
    let _ = editor.register_command(
        &CONTROLLED_TEXT_INSERTION_COMMAND,
        move |payload, _editor| {
            heard.borrow_mut().push(payload.clone());
            Ok(false)
        },
        COMMAND_PRIORITY_NORMAL,
    );
    let heard = transformed.clone();
    let _ = editor.register_node_transform("paragraph", move |node| {
        heard.borrow_mut().push(node.get_type().to_owned());
        Ok(())
    });
    let heard = text_contents.clone();
    let _ = editor.register_text_content_listener(move |text: &str| {
        heard.borrow_mut().push(text.to_owned());
    });

    let inner = editor.clone();
    update(&editor, move || {
        append_hello_world()?;
        inner.dispatch_command(&CONTROLLED_TEXT_INSERTION_COMMAND, "foo".to_owned())?;
        Ok(())
    });

    assert!(*updates.borrow() > 0);
    assert_eq!(*commands.borrow(), ["foo"]);
    assert!(transformed.borrow().iter().any(|t| t == "paragraph"));
    assert!(text_contents.borrow().iter().any(|t| t == "Helloworld"));
}

// upstream: lexical-headless/src/__tests__/unit/LexicalHeadlessEditor.test.ts::LexicalHeadlessEditor > can preserve selection for pending editor state (within update loop)
#[test]
fn can_preserve_selection_for_pending_editor_state() {
    let editor = headless_editor();
    update(&editor, || {
        let text_node = create_text_node("Hello world");
        let paragraph = create_paragraph_node();
        paragraph.append(std::slice::from_ref(&text_node))?;
        get_root().append(&[paragraph])?;
        text_node.select(Some(1), Some(2))?;
        Ok(())
    });
    let points = update_with(&editor, || {
        let selection = get_selection().filter(|s| s.is_range()).unwrap();
        let anchor = selection.point(Which::Anchor).unwrap();
        let focus = selection.point(Which::Focus).unwrap();
        Ok([
            (anchor.offset, anchor.point_type),
            (focus.offset, focus.point_type),
        ])
    });
    assert_eq!(points, [(1, PointType::Text), (2, PointType::Text)]);
}

// upstream: lexical-headless/src/__tests__/unit/LexicalHeadlessEditor.test.ts::LexicalHeadlessEditor > can generate html from the nodes when dom is set
#[cfg(feature = "html")]
#[test]
fn can_generate_html_from_the_nodes_when_dom_is_set() {
    let editor = headless_editor();
    editor
        .set_editor_state(
            // "hello world"
            &editor
                .parse_editor_state(r#"{"root":{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"hello world","type":"text","version":1}],"direction":"ltr","format":"","indent":0,"type":"paragraph","version":1}],"direction":"ltr","format":"","indent":0,"type":"root","version":1}}"#)
                .unwrap(),
            None,
        )
        .unwrap();
    let inner = editor.clone();
    let html = editor
        .read_with_mode(crate::EditorReadMode::Latest, move || {
            crate::html::generate_html_from_nodes(&inner, None)
        })
        .unwrap()
        .unwrap();
    assert_eq!(
        html,
        r#"<p dir="ltr"><span style="white-space: pre-wrap;">hello world</span></p>"#
    );
}
