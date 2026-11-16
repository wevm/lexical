//! Ports of `packages/lexical/src/__tests__/unit/LexicalNormalization.test.tsx`.

use crate::selection_insert_nodes::normalize_selection;
use crate::upstream_tests::support::{
    create_test_decorator_node, create_test_element_node, initialize_unit_test, update,
};
use crate::{
    COLLABORATION_TAG, CreateEditorArgs, DEFER_COMPOSITION_MERGE_TAG, Editor, NodeRef, PointType,
    UpdateOptions, Which, create_paragraph_node, create_text_node, get_root, set_composition_key,
};

fn test_editor() -> Editor {
    initialize_unit_test(CreateEditorArgs {
        namespace: "test".to_owned(),
        ..CreateEditorArgs::default()
    })
}

// text nodes next to a composition

/// `$paragraph()`.
fn paragraph() -> NodeRef {
    get_root().get_first_child().expect("a paragraph")
}

/// `texts()`.
fn texts(editor: &Editor) -> Vec<String> {
    editor.get_editor_state().read(|| {
        paragraph()
            .get_children()
            .iter()
            .map(NodeRef::get_text_content)
            .collect()
    })
}

fn last_child_key(editor: &Editor) -> String {
    editor.get_editor_state().read(|| {
        paragraph()
            .get_last_child_or_throw()
            .unwrap()
            .get_key()
            .to_owned()
    })
}

/// `startComposition()`.
fn start_composition(editor: &Editor) -> String {
    crate::upstream_tests::support::update_with(editor, || {
        let composed = create_text_node("にj2z");
        let p = create_paragraph_node();
        p.append(std::slice::from_ref(&composed))?;
        get_root().clear()?;
        get_root().append(&[p])?;
        let composed_key = composed.get_key().to_owned();
        composed.select(Some(1), Some(1))?;
        set_composition_key(Some(&composed_key));
        Ok(composed_key)
    })
}

/// `insertBefore(tag)`: text from another source lands right before the
/// composition. Upstream awaits the update, which commits it.
fn insert_before(editor: &Editor, tag: Option<&str>) {
    editor
        .update(
            || {
                paragraph()
                    .get_last_child_or_throw()?
                    .insert_before(&create_text_node("j1z"), true)?;
                Ok(())
            },
            UpdateOptions {
                discrete: true,
                tags: tag.into_iter().map(str::to_owned).collect(),
                ..UpdateOptions::default()
            },
        )
        .unwrap();
}

/// `compositionStep()`.
fn composition_step(editor: &Editor) {
    update(editor, || {
        let node = paragraph().get_last_child_or_throw()?;
        node.set_text_content(&node.get_text_content().replacen('に', "にほ", 1));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > text nodes next to a composition > text tagged DEFER_COMPOSITION_MERGE_TAG is not merged into the composed node until it ends
#[test]
fn text_tagged_defer_composition_merge_tag_is_not_merged_into_the_composed_node_until_it_ends() {
    let editor = test_editor();
    let composed_key = start_composition(&editor);
    insert_before(&editor, Some(DEFER_COMPOSITION_MERGE_TAG));
    assert_eq!(texts(&editor), ["j1z", "にj2z"]);
    assert_eq!(last_child_key(&editor), composed_key);
    // Later steps of the same composition do not merge either.
    composition_step(&editor);
    assert_eq!(texts(&editor), ["j1z", "にほj2z"]);
    assert_eq!(last_child_key(&editor), composed_key);
    // The composition ends: the nodes merge.
    update(&editor, || {
        set_composition_key(None);
        Ok(())
    });
    assert_eq!(texts(&editor), ["j1zにほj2z"]);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > text nodes next to a composition > text inserted without the tag merges at once
#[test]
fn text_inserted_without_the_tag_merges_at_once() {
    let editor = test_editor();
    start_composition(&editor);
    insert_before(&editor, None);
    assert_eq!(texts(&editor), ["j1zにj2z"]);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > text nodes next to a composition > COLLABORATION_TAG alone merges at once
#[test]
fn collaboration_tag_alone_merges_at_once() {
    let editor = test_editor();
    start_composition(&editor);
    insert_before(&editor, Some(COLLABORATION_TAG));
    assert_eq!(texts(&editor), ["j1zにj2z"]);
    composition_step(&editor);
    assert_eq!(texts(&editor), ["j1zにほj2z"]);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > text nodes next to a composition > a new composition of the same node merges as usual
#[test]
fn a_new_composition_of_the_same_node_merges_as_usual() {
    let editor = test_editor();
    let composed_key = start_composition(&editor);
    insert_before(&editor, Some(DEFER_COMPOSITION_MERGE_TAG));
    assert_eq!(texts(&editor), ["j1z", "にj2z"]);
    // The composition is reset without normalizing (as setEditorState
    // does), then a new one starts on the same node: nothing is held.
    let e = editor.clone();
    update(&editor, move || {
        *e.0.composition_key.borrow_mut() = None;
        Ok(())
    });
    update(&editor, move || {
        set_composition_key(Some(&composed_key));
        Ok(())
    });
    assert_eq!(texts(&editor), ["j1zにj2z"]);
    insert_before(&editor, None);
    assert_eq!(texts(&editor), ["j1zj1zにj2z"]);
}

// $normalizeSelection

/// A point as `(key, offset, type)`.
type P = (String, usize, PointType);

/// Builds the root's paragraph with `build`, which returns the points to set
/// (`getAnchor`, `getFocus`: swapped when `reversed`) and the expected
/// normalized points; sets them, normalizes, and compares.
fn check_normalize_selection(reversed: bool, build: fn(&NodeRef) -> (P, P, P, P)) {
    let editor = test_editor();
    update(&editor, move || {
        let root = get_root();
        let paragraph = create_paragraph_node();
        let (anchor, focus, want_anchor, want_focus) = build(&paragraph);
        root.append(std::slice::from_ref(&paragraph))?;
        let (a, f) = if reversed {
            (Which::Focus, Which::Anchor)
        } else {
            (Which::Anchor, Which::Focus)
        };
        let selection = paragraph.select(None, None)?;
        selection.point_set(a, &anchor.0, anchor.1, anchor.2)?;
        selection.point_set(f, &focus.0, focus.1, focus.2)?;
        normalize_selection(&selection)?;
        for (which, want) in [(a, want_anchor), (f, want_focus)] {
            let point = selection.point(which).unwrap();
            assert_eq!(point.point_type, want.2);
            assert_eq!(point.get_node()?.get_key(), want.0);
            assert_eq!(point.offset, want.1);
        }
        Ok(())
    });
}

fn key(node: &NodeRef) -> String {
    node.get_key().to_owned()
}

fn paragraph_to_text_nodes(paragraph: &NodeRef) -> (P, P, P, P) {
    let text1 = create_text_node("a");
    let text2 = create_text_node("b");
    paragraph.append(&[text1.clone(), text2.clone()]).unwrap();
    (
        (key(paragraph), 0, PointType::Element),
        (key(paragraph), 2, PointType::Element),
        (key(&text1), 0, PointType::Text),
        (key(&text2), 1, PointType::Text),
    )
}

fn paragraph_to_text_node_and_element(paragraph: &NodeRef) -> (P, P, P, P) {
    let text1 = create_text_node("a");
    let element_node = create_test_element_node();
    paragraph
        .append(&[text1.clone(), element_node.clone()])
        .unwrap();
    (
        (key(paragraph), 0, PointType::Element),
        (key(paragraph), 2, PointType::Element),
        (key(&text1), 0, PointType::Text),
        (key(&element_node), 0, PointType::Element),
    )
}

fn paragraph_to_text_node_and_decorator(paragraph: &NodeRef) -> (P, P, P, P) {
    let text1 = create_text_node("a");
    let decorator_node = create_test_decorator_node();
    paragraph.append(&[text1.clone(), decorator_node]).unwrap();
    (
        (key(paragraph), 0, PointType::Element),
        (key(paragraph), 2, PointType::Element),
        (key(&text1), 0, PointType::Text),
        (key(paragraph), 2, PointType::Element),
    )
}

fn text_and_text_node(paragraph: &NodeRef) -> (P, P, P, P) {
    let text1 = create_text_node("a");
    let text2 = create_text_node("b");
    paragraph.append(&[text1.clone(), text2.clone()]).unwrap();
    (
        (key(&text1), 0, PointType::Text),
        (key(&text2), 1, PointType::Text),
        (key(&text1), 0, PointType::Text),
        (key(&text2), 1, PointType::Text),
    )
}

fn paragraph_to_test_element_to_text_and_text(paragraph: &NodeRef) -> (P, P, P, P) {
    let element_node = create_test_element_node();
    let text1 = create_text_node("a");
    let text2 = create_text_node("b");
    element_node
        .append(&[text1.clone(), text2.clone()])
        .unwrap();
    paragraph.append(&[element_node]).unwrap();
    (
        (key(&text1), 0, PointType::Text),
        (key(&text2), 1, PointType::Text),
        (key(&text1), 0, PointType::Text),
        (key(&text2), 1, PointType::Text),
    )
}

// upstream: lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > $normalizeSelection > paragraph to text nodes
#[test]
fn paragraph_to_text_nodes_forward() {
    check_normalize_selection(false, paragraph_to_text_nodes);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > $normalizeSelection > paragraph to text node + element
#[test]
fn paragraph_to_text_node_element_forward() {
    check_normalize_selection(false, paragraph_to_text_node_and_element);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > $normalizeSelection > paragraph to text node + decorator
#[test]
fn paragraph_to_text_node_decorator_forward() {
    check_normalize_selection(false, paragraph_to_text_node_and_decorator);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > $normalizeSelection > text + text node
#[test]
fn text_text_node_forward() {
    check_normalize_selection(false, text_and_text_node);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > $normalizeSelection > paragraph to test element to text + text
#[test]
fn paragraph_to_test_element_to_text_text_forward() {
    check_normalize_selection(false, paragraph_to_test_element_to_text_and_text);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > $normalizeSelection > paragraph to text nodes (reversed)
#[test]
fn paragraph_to_text_nodes_reversed() {
    check_normalize_selection(true, paragraph_to_text_nodes);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > $normalizeSelection > paragraph to text node + element (reversed)
#[test]
fn paragraph_to_text_node_element_reversed() {
    check_normalize_selection(true, paragraph_to_text_node_and_element);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > $normalizeSelection > paragraph to text node + decorator (reversed)
#[test]
fn paragraph_to_text_node_decorator_reversed() {
    check_normalize_selection(true, paragraph_to_text_node_and_decorator);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > $normalizeSelection > text + text node (reversed)
#[test]
fn text_text_node_reversed() {
    check_normalize_selection(true, text_and_text_node);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalization.test.tsx::LexicalNormalization tests > $normalizeSelection > paragraph to test element to text + text (reversed)
#[test]
fn paragraph_to_test_element_to_text_text_reversed() {
    check_normalize_selection(true, paragraph_to_test_element_to_text_and_text);
}
