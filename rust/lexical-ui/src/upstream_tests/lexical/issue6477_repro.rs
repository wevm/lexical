//! Ports of `packages/lexical/src/__tests__/unit/Issue6477Repro.test.ts`.

#![cfg(all(
    feature = "html",
    feature = "extension",
    feature = "rich-text",
    feature = "link"
))]

use super::super::support::{
    TestDecoratorNode, TestInlineElementNode, create_test_decorator_node,
    create_test_inline_element_node,
};
use crate::clipboard::insert_data_transfer_for_rich_text;
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::link::{LinkAttributes, LinkNode, create_link_node};
use crate::{
    DataTransfer, Editor, LexicalError, LexicalExtension, NodeClass, NodeRef, UpdateOptions,
    create_paragraph_node, create_text_node, define_extension, get_root, get_selection,
};

fn build() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![crate::rich_text::rich_text_extension().into()],
        nodes: vec![
            NodeClass::of::<LinkNode>(),
            NodeClass::of::<TestDecoratorNode>(),
            NodeClass::of::<TestInlineElementNode>(),
        ],
        ..LexicalExtension::<(), (), ()>::new("[6477]", ())
    })
    .into()])
    .unwrap()
}

fn describe_tree(node: &NodeRef) -> String {
    if !node.is_element() {
        return format!("{}(\"{}\")", node.get_type(), node.get_text_content());
    }
    let children: Vec<String> = node.get_children().iter().map(describe_tree).collect();
    format!("{}({})", node.get_type(), children.join(" "))
}

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

fn with(node: NodeRef, children: &[NodeRef]) -> Result<NodeRef, LexicalError> {
    node.append(children)?;
    Ok(node)
}

/// `$pasteHTML(editor, html)`.
fn paste_html(editor: &Editor, html: &str) -> Result<(), LexicalError> {
    let selection = get_selection().filter(|s| s.is_range());
    let selection = selection.expect("Expected a RangeSelection");
    let data_transfer = DataTransfer::new();
    data_transfer.set_data("text/html", html);
    insert_data_transfer_for_rich_text(&data_transfer, &selection, editor)
}

/// The document `before`/`after` text around `inline(abcd)` (either side
/// optional), with the caret at `ab|cd`.
fn set_up(editor: &Editor, wrapper: fn() -> NodeRef, sides: bool) {
    editor
        .update(
            move || {
                let inner = create_text_node("abcd");
                let mut children = Vec::new();
                if sides {
                    children.push(create_text_node("before"));
                }
                children.push(with(wrapper(), std::slice::from_ref(&inner))?);
                if sides {
                    children.push(create_text_node("after"));
                }
                let root = get_root();
                root.clear()?;
                root.append(&[with(create_paragraph_node(), &children)?])?;
                inner.select(Some(2), Some(2))?;
                Ok(())
            },
            discrete(),
        )
        .unwrap();
}

fn paste(editor: &Editor, html: &'static str) {
    let e = editor.clone();
    editor
        .update(move || paste_html(&e, html), discrete())
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/Issue6477Repro.test.ts::Insertion into a custom inline ElementNode (#6477) > pasted text is inserted at the caret inside the node
#[test]
fn pasted_text_is_inserted_at_the_caret_inside_the_node() {
    let editor = build();
    set_up(&editor, create_test_inline_element_node, true);
    paste(&editor, "<b>XYZ</b>");
    editor
        .read(|| {
            assert_eq!(
                describe_tree(&get_root()),
                r#"root(paragraph(text("before") test_inline_block(text("ab") text("XYZ") text("cd")) text("after")))"#
            );
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue6477Repro.test.ts::Insertion into a custom inline ElementNode (#6477) > an inline node inserted at the caret goes inside the node
#[test]
fn an_inline_node_inserted_at_the_caret_goes_inside_the_node() {
    let editor = build();
    set_up(&editor, create_test_inline_element_node, false);
    editor
        .update(
            || {
                let selection = get_selection().filter(|s| s.is_range());
                let selection = selection.expect("Expected a RangeSelection");
                selection.insert_nodes(vec![create_test_decorator_node()])
            },
            discrete(),
        )
        .unwrap();
    editor
        .read(|| {
            assert_eq!(
                describe_tree(&get_root()),
                r#"root(paragraph(test_inline_block(text("ab") test_decorator("Hello world") text("cd"))))"#
            );
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/Issue6477Repro.test.ts::Insertion into a custom inline ElementNode (#6477) > control: a splittable inline ElementNode is still split
#[test]
fn control_a_splittable_inline_element_node_is_still_split() {
    let editor = build();
    set_up(
        &editor,
        || create_link_node("https://lexical.dev", &LinkAttributes::default()),
        false,
    );
    paste(&editor, "<b>XYZ</b>");
    editor
        .read(|| {
            assert_eq!(
                describe_tree(&get_root()),
                r#"root(paragraph(link(text("ab")) text("XYZ") link(text("cd"))))"#
            );
        })
        .unwrap();
    editor.dispose();
}
