//! Ports of `packages/lexical-text/src/__tests__/unit/canShowPlaceholder.test.ts`.

use super::support::update_discrete;
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::text::can_show_placeholder;
use crate::{
    InitialEditorStateType, LexicalExtension, LexicalNode, NodeBase, NodeClass, NodeRef, create,
    create_paragraph_node, create_text_node, define_extension, get_root,
};

/// An inline decorator that contributes no text, e.g. an inline image or
/// equation chip. Its emptiness keeps the root's text content empty while the
/// document plainly is not empty.
#[derive(Clone, Debug, Default)]
struct InlineDecoratorNode {
    base: NodeBase,
}

impl LexicalNode for InlineDecoratorNode {
    fn get_type(&self) -> &'static str {
        "inline_decorator_placeholder"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn is_decorator(&self) -> bool {
        true
    }
    fn is_inline(&self) -> bool {
        true
    }
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        _config: &crate::EditorConfig,
        _editor: &crate::Editor,
    ) -> Result<crate::dom::DomNode, crate::LexicalError> {
        Ok(crate::lexical_dom::get_document().create_element("span"))
    }
}

fn create_inline_decorator_node() -> NodeRef {
    create::<InlineDecoratorNode>()
}

fn build_editor() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        nodes: vec![NodeClass::of::<InlineDecoratorNode>()],
        ..LexicalExtension::<(), (), ()>::new("[can-show-placeholder]", ())
    })
    .into()])
    .unwrap()
}

fn text_with_mode(mode: &str) -> NodeRef {
    let text = create_text_node("");
    text.set_mode(mode).unwrap();
    text
}

/// `$getRoot().clear().append($createParagraphNode().append(...children))`.
fn set_paragraph(editor: &crate::Editor, children: impl FnOnce() -> Vec<NodeRef> + 'static) {
    update_discrete(editor, move || {
        let paragraph = create_paragraph_node();
        paragraph.append(&children())?;
        get_root().clear()?;
        get_root().append(&[paragraph])
    });
}

// A simple empty TextNode is removed during reconciliation, which would move
// the decorator to index 0. `token` and `segmented` nodes are not simple text,
// so they survive and keep a TextNode at index 0 — the shape that exercises
// the scan of the remaining children.
// upstream: lexical-text/src/__tests__/unit/canShowPlaceholder.test.ts::$canShowPlaceholder > is false when a decorator follows an empty token text node
// upstream: lexical-text/src/__tests__/unit/canShowPlaceholder.test.ts::$canShowPlaceholder > is false when a decorator follows an empty segmented text node
#[test]
fn is_false_when_a_decorator_follows_an_empty_non_simple_text_node() {
    for mode in ["token", "segmented"] {
        let editor = build_editor();
        set_paragraph(&editor, move || {
            vec![text_with_mode(mode), create_inline_decorator_node()]
        });
        editor
            .read(|| {
                let paragraph = get_root().get_first_child().unwrap();
                assert_eq!(paragraph.get_type(), "paragraph");
                // Guard the premise: the text node must have survived,
                // otherwise the decorator would sit at index 0 and the scan
                // would be trivial.
                assert_eq!(paragraph.get_children_size(), 2, "{mode}");
                assert_eq!(get_root().get_text_content(), "");
                assert!(!can_show_placeholder(false), "{mode}");
            })
            .unwrap();
        editor.dispose();
    }
}

// upstream: lexical-text/src/__tests__/unit/canShowPlaceholder.test.ts::$canShowPlaceholder > is false when the decorator is several positions along
#[test]
fn is_false_when_the_decorator_is_several_positions_along() {
    let editor = build_editor();
    set_paragraph(&editor, || {
        vec![
            text_with_mode("token"),
            text_with_mode("token"),
            create_inline_decorator_node(),
        ]
    });
    assert!(!editor.read(|| can_show_placeholder(false)).unwrap());
    editor.dispose();
}

// upstream: lexical-text/src/__tests__/unit/canShowPlaceholder.test.ts::$canShowPlaceholder > is still false when the decorator is first
#[test]
fn is_still_false_when_the_decorator_is_first() {
    let editor = build_editor();
    set_paragraph(&editor, || {
        vec![create_inline_decorator_node(), text_with_mode("token")]
    });
    assert!(!editor.read(|| can_show_placeholder(false)).unwrap());
    editor.dispose();
}

// upstream: lexical-text/src/__tests__/unit/canShowPlaceholder.test.ts::$canShowPlaceholder > is still true for an empty paragraph
#[test]
fn is_still_true_for_an_empty_paragraph() {
    let editor = build_editor();
    set_paragraph(&editor, Vec::new);
    assert!(editor.read(|| can_show_placeholder(false)).unwrap());
    editor.dispose();
}

// upstream: lexical-text/src/__tests__/unit/canShowPlaceholder.test.ts::$canShowPlaceholder > is still true for a paragraph of empty text nodes
#[test]
fn is_still_true_for_a_paragraph_of_empty_text_nodes() {
    let editor = build_editor();
    set_paragraph(&editor, || {
        vec![text_with_mode("token"), text_with_mode("token")]
    });
    assert!(editor.read(|| can_show_placeholder(false)).unwrap());
    editor.dispose();
}
