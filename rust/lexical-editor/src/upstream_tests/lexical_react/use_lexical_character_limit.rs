//! Ports of `packages/lexical-react/src/__tests__/unit/useLexicalCharacterLimit.test.ts`
//! (`shared/useCharacterLimit.ts`'s `$mergePrevious` and
//! `$wrapOverflowedNodes`, which the `overflow` feature has).

#![cfg(all(feature = "overflow", feature = "extension"))]

use super::super::support::{
    TestBlockableDecoratorNode, create_test_decorator_node_inline, set_test_root_element, update,
};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::overflow::{
    create_overflow_node, is_overflow_node, merge_previous, overflow_extension,
    wrap_overflowed_nodes,
};
use crate::{
    LexicalExtension, NodeClass, NodeRef, PointType, Which, create_paragraph_node,
    create_text_node, define_extension, get_root, get_selection, set_slot,
};

fn make_editor() -> LexicalEditorWithDispose {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![overflow_extension().into()],
        nodes: vec![NodeClass::of::<TestBlockableDecoratorNode>()],
        ..LexicalExtension::<(), (), ()>::new("character-limit-test", ())
    })
    .into()])
    .unwrap();
    set_test_root_element(&editor);
    editor
}

/// `$setupLeftRightOverflow()`: `(overflowLeft, overflowRight, paragraph)`.
fn setup_left_right_overflow() -> (NodeRef, NodeRef, NodeRef) {
    let root = get_root();
    root.clear().unwrap();
    let paragraph = create_paragraph_node();
    let overflow_left = create_overflow_node();
    let overflow_right = create_overflow_node();
    root.append(std::slice::from_ref(&paragraph)).unwrap();
    paragraph
        .append(&[overflow_left.clone(), overflow_right.clone()])
        .unwrap();
    (overflow_left, overflow_right, paragraph)
}

/// The texts `1` to `4`, `1` and `2` in `overflow_left`, `3` and `4` in
/// `overflow_right`, `2` and `4` bold.
fn four_texts(overflow_left: &NodeRef, overflow_right: &NodeRef) -> [NodeRef; 4] {
    let texts = ["1", "2", "3", "4"].map(create_text_node);
    overflow_left
        .append(&[texts[0].clone(), texts[1].clone()])
        .unwrap();
    texts[1].toggle_format("bold");
    overflow_right
        .append(&[texts[2].clone(), texts[3].clone()])
        .unwrap();
    texts[3].toggle_format("bold");
    texts
}

fn point(which: Which) -> (String, usize) {
    let selection = get_selection().expect("Lost selection");
    assert!(selection.is_range(), "Lost selection");
    let point = selection.point(which).unwrap();
    (point.key.to_string(), point.offset)
}

// upstream: lexical-react/src/__tests__/unit/useLexicalCharacterLimit.test.ts::useCharacterLimit > $mergePrevious > merges left overflow into right with selection in left
#[test]
fn merges_left_overflow_into_right_with_selection_in_left() {
    let editor = make_editor();
    update(&editor, || {
        let (overflow_left, overflow_right, paragraph) = setup_left_right_overflow();
        let [text1, ..] = four_texts(&overflow_left, &overflow_right);
        text1.select(Some(1), Some(1))?;
        merge_previous(&overflow_right)?;
        assert_eq!(paragraph.get_children_size(), 1);
        assert!(is_overflow_node(&paragraph.get_first_child().unwrap()));
        let key = text1.get_key().to_owned();
        assert_eq!(point(Which::Anchor), (key.clone(), 1));
        assert_eq!(point(Which::Focus), (key, 1));
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical-react/src/__tests__/unit/useLexicalCharacterLimit.test.ts::useCharacterLimit > $mergePrevious > merges with selection spanning both overflows
#[test]
fn merges_with_selection_spanning_both_overflows() {
    let editor = make_editor();
    update(&editor, || {
        let (overflow_left, overflow_right, paragraph) = setup_left_right_overflow();
        let [_, text2, text3, _] = four_texts(&overflow_left, &overflow_right);
        let selection = text2.select(Some(0), Some(0))?;
        selection.point_set(Which::Focus, text3.get_key(), 1, PointType::Text)?;
        merge_previous(&overflow_right)?;
        assert_eq!(paragraph.get_children_size(), 1);
        assert_eq!(point(Which::Anchor), (text2.get_key().to_owned(), 0));
        assert_eq!(point(Which::Focus), (text3.get_key().to_owned(), 1));
        Ok(())
    });
    editor.dispose();
}

fn paragraph_at(index: usize) -> NodeRef {
    let paragraph = get_root().get_children()[index].clone();
    assert_eq!(paragraph.get_type(), "paragraph");
    paragraph
}

/// Root paragraphs of one text node each.
fn seed_paragraphs(editor: &crate::Editor, texts: &'static [&'static str]) {
    update(editor, move || {
        let root = get_root();
        root.clear()?;
        for text in texts {
            let p = create_paragraph_node();
            p.append(&[create_text_node(text)])?;
            root.append(&[p])?;
        }
        Ok(())
    });
}

// upstream: lexical-react/src/__tests__/unit/useLexicalCharacterLimit.test.ts::useCharacterLimit > $wrapOverflowedNodes > merges adjacent overflow nodes when wrapping before existing overflow
#[test]
fn merges_adjacent_overflow_nodes_when_wrapping_before_existing_overflow() {
    let editor = make_editor();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        let paragraph = create_paragraph_node();
        let overflow = create_overflow_node();
        overflow.append(&[create_text_node("f")])?;
        paragraph.append(&[create_text_node("abcde"), overflow])?;
        root.append(&[paragraph])
    });
    update(&editor, || {
        let text = paragraph_at(0).get_first_child().unwrap();
        if text.is_text() {
            text.set_text_content("abcdge");
        }
        Ok(())
    });
    update(&editor, || wrap_overflowed_nodes(5));
    editor
        .read(|| {
            let children = paragraph_at(0).get_children();
            assert_eq!(children.len(), 2);
            assert_eq!(children[0].get_text_content(), "abcdg");
            assert!(is_overflow_node(&children[1]));
            assert_eq!(children[1].get_text_content(), "ef");
        })
        .unwrap();
    editor.dispose();
}

// upstream: "lexical-react/src/__tests__/unit/useLexicalCharacterLimit.test.ts::useCharacterLimit > $wrapOverflowedNodes > counts \\n\\n separators between paragraphs (#6329)"
#[test]
fn counts_separators_between_paragraphs() {
    let editor = make_editor();
    seed_paragraphs(&editor, &["123", "456"]);
    // "123\n\n456": offset 5 is "123" (3) + separator (2), so all of P2
    // overflows.
    update(&editor, || wrap_overflowed_nodes(5));
    editor
        .read(|| {
            let (p1, p2) = (paragraph_at(0), paragraph_at(1));
            assert_eq!(p1.get_text_content(), "123");
            assert!(!is_overflow_node(&p1.get_first_child().unwrap()));
            let overflow = p2.get_first_child().unwrap();
            assert!(is_overflow_node(&overflow));
            assert_eq!(overflow.get_text_content(), "456");
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-react/src/__tests__/unit/useLexicalCharacterLimit.test.ts::useCharacterLimit > $wrapOverflowedNodes > splits text correctly with separator budget (#6329)
#[test]
fn splits_text_correctly_with_separator_budget() {
    let editor = make_editor();
    seed_paragraphs(&editor, &["12", "3456"]);
    // "12\n\n3456": offset 5 is "12" (2) + separator (2) + "3" (1), so
    // "456" in P2 overflows.
    update(&editor, || wrap_overflowed_nodes(5));
    editor
        .read(|| {
            let children = paragraph_at(1).get_children();
            assert_eq!(children[0].get_text_content(), "3");
            assert!(is_overflow_node(&children[1]));
            assert_eq!(children[1].get_text_content(), "456");
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-react/src/__tests__/unit/useLexicalCharacterLimit.test.ts::useCharacterLimit > $wrapOverflowedNodes > counts multiple separators across three paragraphs (#6329)
#[test]
fn counts_multiple_separators_across_three_paragraphs() {
    let editor = make_editor();
    seed_paragraphs(&editor, &["abc", "de", "fgh"]);
    // "abc\n\nde\n\nfgh": offset 9 is "abc"(3)+sep(2)+"de"(2)+sep(2), so
    // all of P3 overflows.
    update(&editor, || wrap_overflowed_nodes(9));
    editor
        .read(|| {
            for (i, text) in [(0, "abc"), (1, "de")] {
                let p = paragraph_at(i);
                assert_eq!(p.get_text_content(), text);
                assert!(!is_overflow_node(&p.get_first_child().unwrap()));
            }
            let overflow = paragraph_at(2).get_first_child().unwrap();
            assert!(is_overflow_node(&overflow));
            assert_eq!(overflow.get_text_content(), "fgh");
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-react/src/__tests__/unit/useLexicalCharacterLimit.test.ts::useCharacterLimit > $wrapOverflowedNodes > advances the budget past a slotted decorator so the wrap boundary is exact
#[test]
fn advances_the_budget_past_a_slotted_decorator() {
    let editor = make_editor();
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        let host = create_paragraph_node();
        host.append(&[create_text_node("12345")])?;
        root.append(std::slice::from_ref(&host))?;
        set_slot(&host, "media", &create_test_decorator_node_inline(false))?;
        Ok(())
    });
    update(&editor, || wrap_overflowed_nodes(13));
    editor
        .read(|| {
            let children = paragraph_at(0).get_children();
            assert_eq!(children[0].get_text_content(), "12");
            assert!(is_overflow_node(&children[1]));
            assert_eq!(children[1].get_text_content(), "345");
        })
        .unwrap();
    editor.dispose();
}
