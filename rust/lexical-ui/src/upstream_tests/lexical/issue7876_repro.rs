//! Ports of `packages/lexical/src/__tests__/unit/Issue7876Repro.test.ts`.

#![cfg(all(feature = "extension", feature = "rich-text"))]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::{
    LexicalExtension, NodeRef, UpdateOptions, create_paragraph_node, create_text_node,
    define_extension, get_root,
};
use std::cell::Cell;
use std::rc::Rc;

const PARSED_TEXT: &str = "hello {{name}}";
const TRANSFORMED_TEXT: &str = "VARIABLE";

/// What the test's text transform also calls.
type OnTransform = Box<dyn Fn(&NodeRef)>;

fn build_editor(on_transform: Option<OnTransform>) -> LexicalEditorWithDispose {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![crate::rich_text::rich_text_extension().into()],
        ..LexicalExtension::<(), (), ()>::new("issue-7876-repro", ())
    })
    .into()])
    .unwrap();
    if let Some(on_transform) = on_transform {
        let _ = editor.register_node_transform("text", move |node| {
            on_transform(node);
            Ok(())
        });
    }
    editor
}

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

/// `JSON.stringify(src.getEditorState())` of an editor whose only
/// paragraphs hold `texts`.
fn make_parsed_json(texts: Vec<String>) -> String {
    let src = build_editor(None);
    src.update(
        move || {
            let root = get_root();
            root.clear()?;
            for text in &texts {
                let paragraph = create_paragraph_node();
                paragraph.append(&[create_text_node(text)])?;
                root.append(&[paragraph])?;
            }
            Ok(())
        },
        discrete(),
    )
    .unwrap();
    let json = src.get_editor_state().to_json(false).to_string();
    src.dispose();
    json
}

fn variable_transform(node: &NodeRef) {
    let text = node.get_text_content();
    if text.contains("{{") && text != TRANSFORMED_TEXT {
        node.set_text_content(TRANSFORMED_TEXT);
    }
}

fn text_content(editor: &LexicalEditorWithDispose) -> String {
    editor.read(|| get_root().get_text_content()).unwrap()
}

// upstream: lexical/src/__tests__/unit/Issue7876Repro.test.ts::Issue #7876: setEditorState triggers transforms on parsed state > text-node transform fires for a JSON state passed to setEditorState
#[test]
fn text_node_transform_fires_for_a_json_state_passed_to_set_editor_state() {
    let editor = build_editor(Some(Box::new(variable_transform)));
    let parsed = editor
        .parse_editor_state(&make_parsed_json(vec![PARSED_TEXT.to_owned()]))
        .unwrap();
    assert!(parsed.0.parsed.get());
    editor.set_editor_state(&parsed, None).unwrap();
    assert_eq!(text_content(&editor), TRANSFORMED_TEXT);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue7876Repro.test.ts::Issue #7876: setEditorState triggers transforms on parsed state > text-node transform fires for a cloned parsed state (the documented no-focus form)
#[test]
fn text_node_transform_fires_for_a_cloned_parsed_state() {
    let editor = build_editor(Some(Box::new(variable_transform)));
    let parsed = editor
        .parse_editor_state(&make_parsed_json(vec![PARSED_TEXT.to_owned()]))
        .unwrap();
    assert!(parsed.0.parsed.get());
    let cloned = parsed.clone_with_selection(Some(None));
    assert!(cloned.0.parsed.get());
    editor.set_editor_state(&cloned, None).unwrap();
    assert_eq!(text_content(&editor), TRANSFORMED_TEXT);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue7876Repro.test.ts::Issue #7876: setEditorState triggers transforms on parsed state > setEditorState consumes the `_parsed` flag so re-applying the resulting state does not re-trigger transforms
#[test]
fn set_editor_state_consumes_the_parsed_flag() {
    let calls = Rc::new(Cell::new(0));
    let c = calls.clone();
    let editor = build_editor(Some(Box::new(move |node| {
        c.set(c.get() + 1);
        variable_transform(node);
    })));
    let parsed = editor
        .parse_editor_state(&make_parsed_json(vec![PARSED_TEXT.to_owned()]))
        .unwrap();
    editor.set_editor_state(&parsed, None).unwrap();
    assert!(calls.get() >= 1);
    let current_state = editor.get_editor_state();
    assert!(!current_state.0.parsed.get());
    let before = calls.get();
    editor.set_editor_state(&current_state, None).unwrap();
    assert_eq!(calls.get() - before, 0);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue7876Repro.test.ts::Issue #7876: setEditorState triggers transforms on parsed state > a non-idempotent transform stabilises in two passes on a parsed state
#[test]
fn a_non_idempotent_transform_stabilises_in_two_passes_on_a_parsed_state() {
    const SENTINEL: &str = "#done:";
    let calls = Rc::new(Cell::new(0));
    let c = calls.clone();
    let editor = build_editor(Some(Box::new(move |node| {
        c.set(c.get() + 1);
        let text = node.get_text_content();
        if !text.starts_with(SENTINEL) {
            node.set_text_content(&format!("{SENTINEL}{text}"));
        }
    })));
    let parsed = editor
        .parse_editor_state(&make_parsed_json(vec![PARSED_TEXT.to_owned()]))
        .unwrap();
    editor.set_editor_state(&parsed, None).unwrap();
    assert_eq!(text_content(&editor), format!("{SENTINEL}{PARSED_TEXT}"));
    assert!(calls.get() <= 2);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue7876Repro.test.ts::Issue #7876: setEditorState triggers transforms on parsed state > setEditorState completes within bound on a 1000-paragraph parsed state
#[test]
fn set_editor_state_completes_within_bound_on_a_1000_paragraph_parsed_state() {
    let calls = Rc::new(Cell::new(0));
    let c = calls.clone();
    let editor = build_editor(Some(Box::new(move |_| c.set(c.get() + 1))));
    const NUM_PARAS: usize = 1000;
    let json = make_parsed_json((0..NUM_PARAS).map(|i| format!("line {i}")).collect());
    let parsed = editor.parse_editor_state(&json).unwrap();
    let start = std::time::Instant::now();
    editor.set_editor_state(&parsed, None).unwrap();
    let elapsed = start.elapsed();
    assert!(elapsed.as_millis() < 500, "{elapsed:?}");
    assert!(calls.get() >= NUM_PARAS);
    editor.dispose();
}
