//! Ports of `packages/lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts`.
//!
//! Upstream runs each test for `CustomSizedParagraphNode` and
//! `CustomUnsizedParagraphNode` (`#2`), which both register the type
//! `custom-paragraph`, so `$create` of either builds the one registered
//! last. JS gives the unsized class `LexicalNode.getTextContentSize`
//! (`getTextContent().length`) since it overrides only `getTextContent`;
//! Rust can't tell an override, so the node overrides
//! [`LexicalNode::get_text_content_size`] with what JS injects.

#![cfg(feature = "extension")]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::nodes::element::{self, ElementData};
use crate::upstream_tests::support::{
    TestElementNode, create_test_element_node, set_test_root_element, update,
};
use crate::{
    Editor, LexicalExtension, LexicalNode, NodeBase, NodeClass, NodeRef, create,
    create_paragraph_node, create_text_node, define_extension, get_root, remove_slot, set_slot,
    utf16,
};
use std::cell::RefCell;
use std::rc::Rc;

macro_rules! custom_paragraph {
    ($name:ident, $text:expr, $size:expr) => {
        #[derive(Clone, Debug, Default)]
        struct $name {
            base: NodeBase,
            element: ElementData,
        }

        impl LexicalNode for $name {
            fn get_type(&self) -> &'static str {
                "custom-paragraph"
            }
            fn super_types(&self) -> Vec<&'static str> {
                vec!["paragraph"]
            }
            fn base(&self) -> &NodeBase {
                &self.base
            }
            fn base_mut(&mut self) -> &mut NodeBase {
                &mut self.base
            }
            fn element(&self) -> Option<&ElementData> {
                Some(&self.element)
            }
            fn element_mut(&mut self) -> Option<&mut ElementData> {
                Some(&mut self.element)
            }
            fn get_text_content(&self) -> String {
                let f: fn(String) -> String = $text;
                f(element::get_text_content(self))
            }
            fn get_text_content_size(&self) -> usize {
                let f: fn(&Self) -> usize = $size;
                f(self)
            }
        }
    };
}

// `CustomUnsizedParagraphNode`, with the `getTextContentSize` JS injects.
custom_paragraph!(
    CustomUnsizedParagraphNode,
    |text| format!("~\"{text}\""),
    |node| utf16::len(&node.get_text_content())
);

// `CustomSizedParagraphNode`.
custom_paragraph!(
    CustomSizedParagraphNode,
    |text| format!("--\"{text}\""),
    |node| 4 + element::get_text_content_size(node)
);

/// `createEditor()`.
fn create_editor() -> LexicalEditorWithDispose {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        nodes: vec![
            NodeClass::of::<CustomSizedParagraphNode>(),
            NodeClass::of::<CustomUnsizedParagraphNode>(),
            NodeClass::of::<TestElementNode>(),
        ],
        ..LexicalExtension::<(), (), ()>::new("CustomTextContent", ())
    })
    .into()])
    .unwrap();
    set_test_root_element(&editor);
    editor
}

fn root_text(editor: &Editor) -> String {
    editor.read(|| get_root().get_text_content()).unwrap()
}

/// What an update gives back to the test, as JS's `let` bindings.
fn keep<T: Clone + 'static>() -> (Rc<RefCell<Option<T>>>, impl Fn() -> T) {
    let cell = Rc::new(RefCell::new(None::<T>));
    let read = cell.clone();
    (cell, move || {
        read.borrow().clone().expect("set by an update")
    })
}

fn root_text_includes_a_custom_element_containing<T: LexicalNode + Default>(text: &'static str) {
    let editor = create_editor();
    update(&editor, move || {
        let paragraph = create::<T>();
        if !text.is_empty() {
            paragraph.append(&[create_text_node(text)])?;
        }
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&paragraph))?;
        assert_eq!(
            paragraph.get_text_content_size(),
            utf16::len(&paragraph.get_text_content())
        );
        assert_eq!(get_root().get_text_content(), format!("~\"{text}\""));
        Ok(())
    });
    editor
        .read(|| {
            assert_eq!(get_root().get_text_content(), format!("~\"{text}\""));
            assert_eq!(
                get_root().get_text_content_size(),
                utf16::len(&format!("~\"{text}\""))
            );
        })
        .unwrap();
    editor.dispose();
}

fn updates_custom_text_when_only_a_descendant_is_dirty<T: LexicalNode + Default>() {
    let editor = create_editor();
    let (set, text) = keep::<NodeRef>();
    update(&editor, move || {
        let text = create_text_node("Hello");
        let paragraph = create::<T>();
        paragraph.append(std::slice::from_ref(&text))?;
        get_root().clear()?;
        get_root().append(&[paragraph])?;
        *set.borrow_mut() = Some(text);
        Ok(())
    });
    let changes = Rc::new(RefCell::new(Vec::<String>::new()));
    let seen = changes.clone();
    let unregister = editor
        .register_text_content_listener(move |value| seen.borrow_mut().push(value.to_owned()));
    let t = text();
    update(&editor, move || {
        t.set_text_content("World");
        Ok(())
    });
    assert_eq!(root_text(&editor), "~\"World\"");
    assert_eq!(*changes.borrow(), ["~\"World\""]);
    unregister();
    editor.dispose();
}

fn preserves_clean_custom_text_when_a_following_sibling_changes<T: LexicalNode + Default>() {
    let editor = create_editor();
    let (set, text) = keep::<NodeRef>();
    update(&editor, move || {
        let text = create_text_node("Last");
        let paragraph = |child: NodeRef| -> Result<NodeRef, crate::LexicalError> {
            let p = create_paragraph_node();
            p.append(&[child])?;
            Ok(p)
        };
        let custom = create::<T>();
        custom.append(&[create_text_node("Custom")])?;
        get_root().clear()?;
        get_root().append(&[
            paragraph(create_text_node("First"))?,
            custom,
            paragraph(create_text_node("Third"))?,
            paragraph(text.clone())?,
        ])?;
        *set.borrow_mut() = Some(text);
        Ok(())
    });
    let t = text();
    update(&editor, move || {
        t.set_text_content("Updated");
        Ok(())
    });
    assert_eq!(
        root_text(&editor),
        "First\n\n~\"Custom\"\n\nThird\n\nUpdated"
    );
    editor.dispose();
}

fn recomputes_a_custom_parent_when_its_child_suffix_changes<T: LexicalNode + Default>() {
    let editor = create_editor();
    let (set, nodes) = keep::<(NodeRef, NodeRef)>();
    update(&editor, move || {
        let paragraph = create::<T>();
        let last = create_text_node("d");
        last.toggle_format("underline");
        let b = create_text_node("b");
        b.toggle_format("bold");
        let c = create_text_node("c");
        c.toggle_format("italic");
        paragraph.append(&[create_text_node("a"), b, c, last.clone()])?;
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&paragraph))?;
        *set.borrow_mut() = Some((paragraph, last));
        Ok(())
    });
    let (paragraph, last) = nodes();
    update(&editor, move || {
        last.set_text_content("D");
        Ok(())
    });
    assert_eq!(root_text(&editor), "~\"abcD\"");
    let p = paragraph.clone();
    update(&editor, move || p.append(&[create_text_node("e")]));
    assert_eq!(root_text(&editor), "~\"abcDe\"");
    update(&editor, move || {
        paragraph.get_last_child_or_throw()?.remove(false)
    });
    assert_eq!(root_text(&editor), "~\"abcD\"");
    editor.dispose();
}

fn restores_custom_text_when_an_editor_state_is_replaced<T: LexicalNode + Default>() {
    let editor = create_editor();
    update(&editor, || {
        let custom = create::<T>();
        custom.append(&[create_text_node("Original")])?;
        get_root().clear()?;
        get_root().append(&[custom])
    });
    let original = editor.get_editor_state();
    update(&editor, || {
        get_root().clear()?;
        get_root().append(&[create_paragraph_node()])
    });
    editor.set_editor_state(&original, None).unwrap();
    assert_eq!(root_text(&editor), "~\"Original\"");
    editor.dispose();
}

fn preserves_the_source_text_when_an_ancestor_of_a_custom_node_moves<T: LexicalNode + Default>() {
    let editor = create_editor();
    let (set, nodes) = keep::<(NodeRef, NodeRef, NodeRef)>();
    update(&editor, move || {
        let target = create_test_element_node();
        let text = create_text_node("Original");
        let custom = create::<T>();
        custom.append(std::slice::from_ref(&text))?;
        let moved = create_test_element_node();
        moved.append(&[custom])?;
        let paragraph = |s: &str| -> Result<NodeRef, crate::LexicalError> {
            let p = create_paragraph_node();
            p.append(&[create_text_node(s)])?;
            Ok(p)
        };
        let source = create_test_element_node();
        source.append(&[
            paragraph("First")?,
            paragraph("Second")?,
            paragraph("Third")?,
            moved.clone(),
        ])?;
        get_root().clear()?;
        get_root().append(&[target.clone(), source])?;
        *set.borrow_mut() = Some((target, moved, text));
        Ok(())
    });
    let (target, moved, text) = nodes();
    let (tg, mv) = (target.clone(), moved.clone());
    update(&editor, move || {
        assert_eq!(
            mv.get_text_content_size(),
            utf16::len(&mv.get_text_content())
        );
        tg.append(std::slice::from_ref(&mv))?;
        text.set_text_content("Changed text");
        Ok(())
    });
    editor
        .read(|| {
            assert_eq!(
                moved.get_text_content_size(),
                utf16::len(&moved.get_text_content())
            );
            assert_eq!(
                target.get_text_content_size(),
                utf16::len(&target.get_text_content())
            );
            assert_eq!(
                get_root().get_text_content(),
                "~\"Changed text\"\n\nFirst\n\nSecond\n\nThird"
            );
            assert_eq!(
                get_root().get_text_content_size(),
                utf16::len(&get_root().get_text_content())
            );
        })
        .unwrap();
    editor.dispose();
}

fn includes_slots_in_custom_text_when_they_change_or_are_removed<T: LexicalNode + Default>() {
    let editor = create_editor();
    let (set, nodes) = keep::<(NodeRef, NodeRef)>();
    update(&editor, move || {
        let paragraph = create::<T>();
        paragraph.append(&[create_text_node("Body")])?;
        let title = create_text_node("Title");
        let slot = create_paragraph_node();
        slot.append(std::slice::from_ref(&title))?;
        set_slot(&paragraph, "title", &slot)?;
        get_root().clear()?;
        get_root().append(std::slice::from_ref(&paragraph))?;
        *set.borrow_mut() = Some((paragraph, title));
        Ok(())
    });
    let (paragraph, title) = nodes();
    assert_eq!(root_text(&editor), "~\"TitleBody\"");
    update(&editor, move || {
        title.set_text_content("Updated");
        Ok(())
    });
    editor
        .read(|| {
            assert_eq!(get_root().get_text_content(), "~\"UpdatedBody\"");
            assert_eq!(
                paragraph.get_text_content_size(),
                utf16::len(&paragraph.get_text_content())
            );
            assert_eq!(
                get_root().get_text_content_size(),
                utf16::len(&get_root().get_text_content())
            );
        })
        .unwrap();
    let p = paragraph.clone();
    update(&editor, move || remove_slot(&p, "title").map(drop));
    assert_eq!(root_text(&editor), "~\"Body\"");
    editor.dispose();
}

// upstream: "lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > root text includes a custom element containing \"\" after reconciliation"
#[test]
fn root_text_includes_a_custom_element_containing_empty_after_reconciliation() {
    root_text_includes_a_custom_element_containing::<CustomSizedParagraphNode>("");
}

// upstream: "lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > root text includes a custom element containing \"Hello\" after reconciliation"
#[test]
fn root_text_includes_a_custom_element_containing_hello_after_reconciliation() {
    root_text_includes_a_custom_element_containing::<CustomSizedParagraphNode>("Hello");
}

// upstream: lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > updates custom text when only a descendant is dirty
#[test]
fn updates_custom_text_when_only_a_descendant_is_dirty_sized() {
    updates_custom_text_when_only_a_descendant_is_dirty::<CustomSizedParagraphNode>();
}

// upstream: lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > preserves clean custom text when a following sibling changes
#[test]
fn preserves_clean_custom_text_when_a_following_sibling_changes_sized() {
    preserves_clean_custom_text_when_a_following_sibling_changes::<CustomSizedParagraphNode>();
}

// upstream: lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > recomputes a custom parent when its child suffix changes
#[test]
fn recomputes_a_custom_parent_when_its_child_suffix_changes_sized() {
    recomputes_a_custom_parent_when_its_child_suffix_changes::<CustomSizedParagraphNode>();
}

// upstream: lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > restores custom text when an editor state is replaced
#[test]
fn restores_custom_text_when_an_editor_state_is_replaced_sized() {
    restores_custom_text_when_an_editor_state_is_replaced::<CustomSizedParagraphNode>();
}

// upstream: lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > preserves the source text when an ancestor of a custom node moves
#[test]
fn preserves_the_source_text_when_an_ancestor_of_a_custom_node_moves_sized() {
    preserves_the_source_text_when_an_ancestor_of_a_custom_node_moves::<CustomSizedParagraphNode>();
}

// upstream: lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > includes slots in custom text when they change or are removed
#[test]
fn includes_slots_in_custom_text_when_they_change_or_are_removed_sized() {
    includes_slots_in_custom_text_when_they_change_or_are_removed::<CustomSizedParagraphNode>();
}

// upstream: "lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > root text includes a custom element containing \"\" after reconciliation #2"
#[test]
fn root_text_includes_a_custom_element_containing_empty_after_reconciliation_unsized() {
    root_text_includes_a_custom_element_containing::<CustomUnsizedParagraphNode>("");
}

// upstream: "lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > root text includes a custom element containing \"Hello\" after reconciliation #2"
#[test]
fn root_text_includes_a_custom_element_containing_hello_after_reconciliation_unsized() {
    root_text_includes_a_custom_element_containing::<CustomUnsizedParagraphNode>("Hello");
}

// upstream: lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > updates custom text when only a descendant is dirty #2
#[test]
fn updates_custom_text_when_only_a_descendant_is_dirty_unsized() {
    updates_custom_text_when_only_a_descendant_is_dirty::<CustomUnsizedParagraphNode>();
}

// upstream: lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > preserves clean custom text when a following sibling changes #2
#[test]
fn preserves_clean_custom_text_when_a_following_sibling_changes_unsized() {
    preserves_clean_custom_text_when_a_following_sibling_changes::<CustomUnsizedParagraphNode>();
}

// upstream: lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > recomputes a custom parent when its child suffix changes #2
#[test]
fn recomputes_a_custom_parent_when_its_child_suffix_changes_unsized() {
    recomputes_a_custom_parent_when_its_child_suffix_changes::<CustomUnsizedParagraphNode>();
}

// upstream: lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > restores custom text when an editor state is replaced #2
#[test]
fn restores_custom_text_when_an_editor_state_is_replaced_unsized() {
    restores_custom_text_when_an_editor_state_is_replaced::<CustomUnsizedParagraphNode>();
}

// upstream: lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > preserves the source text when an ancestor of a custom node moves #2
#[test]
fn preserves_the_source_text_when_an_ancestor_of_a_custom_node_moves_unsized() {
    preserves_the_source_text_when_an_ancestor_of_a_custom_node_moves::<CustomUnsizedParagraphNode>(
    );
}

// upstream: lexical/src/__tests__/unit/LexicalCustomTextContent.test.ts::custom element text content > includes slots in custom text when they change or are removed #2
#[test]
fn includes_slots_in_custom_text_when_they_change_or_are_removed_unsized() {
    includes_slots_in_custom_text_when_they_change_or_are_removed::<CustomUnsizedParagraphNode>();
}
