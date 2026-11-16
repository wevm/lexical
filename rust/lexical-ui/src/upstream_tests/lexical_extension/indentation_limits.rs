//! Ports of `packages/lexical-extension/src/__tests__/unit/IndentationLimits.test.ts`.

#![cfg(all(feature = "rich-text", feature = "list"))]

use super::super::support::update;
use crate::extension::{
    LexicalEditorWithDispose, TabIndentationConfigOverrides, build_editor_from_extensions,
    tab_indentation_extension,
};
use crate::list::{ListType, create_list_item_node, create_list_node};
use crate::{
    Editor, INDENT_CONTENT_COMMAND, LexicalExtension, NodeRef, OUTDENT_CONTENT_COMMAND,
    config_extension, create_paragraph_node, create_text_node, define_extension, get_root,
};

/// `createEditor()`.
fn create_editor() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            crate::list::list_extension().into(),
            config_extension(
                &tab_indentation_extension(),
                [TabIndentationConfigOverrides {
                    max_indent: Some(Some(7.0)),
                    ..Default::default()
                }],
            ),
        ],
        ..LexicalExtension::<(), (), ()>::new("IndentationLimits", ())
    })
    .into()])
    .unwrap()
}

/// `indent(editor, times)`.
fn indent(editor: &Editor, times: usize) {
    for _ in 0..times {
        let inner = editor.clone();
        update(editor, move || {
            inner.dispatch_command(&INDENT_CONTENT_COMMAND, ())?;
            Ok(())
        });
    }
}

/// `$items(node)`: `(indent, text)` of each list item that holds content.
fn items_of(node: &NodeRef) -> Vec<(f64, String)> {
    if node.get_type() == "listitem" && !node.get_first_child().is_some_and(|c| c.is_element()) {
        return vec![(node.get_indent().unwrap(), node.get_text_content())];
    }
    if node.is_element() {
        node.get_children().iter().flat_map(items_of).collect()
    } else {
        vec![]
    }
}

fn items(editor: &Editor) -> Vec<(f64, String)> {
    editor.read(|| items_of(&get_root())).unwrap()
}

/// `$getParagraphIndent()`.
fn paragraph_indent(editor: &Editor) -> f64 {
    editor
        .read(|| {
            let paragraph = get_root().get_first_child_or_throw().unwrap();
            assert_eq!(paragraph.get_type(), "paragraph");
            paragraph.get_indent().unwrap()
        })
        .unwrap()
}

fn caps_paragraph(text: &'static str) {
    let editor = create_editor();
    update(&editor, move || {
        let paragraph = create_paragraph_node();
        if !text.is_empty() {
            paragraph.append(&[create_text_node(text)])?;
        }
        let root = get_root();
        root.clear()?;
        root.append(std::slice::from_ref(&paragraph))?;
        paragraph.select_end()?;
        Ok(())
    });
    for i in 1..=14 {
        indent(&editor, 1);
        assert_eq!(
            paragraph_indent(&editor),
            f64::from(i.min(6)),
            "{text:?} {i}"
        );
    }
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::caps paragraph indentation for ""
#[test]
fn caps_paragraph_indentation_empty() {
    caps_paragraph("");
}

// upstream: lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::caps paragraph indentation for "World"
#[test]
fn caps_paragraph_indentation_world() {
    caps_paragraph("World");
}

fn caps_list(text: &'static str) {
    let editor = create_editor();
    update(&editor, move || {
        let item = create_list_item_node(None);
        if !text.is_empty() {
            item.append(&[create_text_node(text)])?;
        }
        let list = create_list_node(ListType::Bullet, 1.0);
        list.append(std::slice::from_ref(&item))?;
        let root = get_root();
        root.clear()?;
        root.append(&[list])?;
        item.select_end()?;
        Ok(())
    });
    for i in 1..=14 {
        indent(&editor, 1);
        assert_eq!(
            items(&editor),
            [(f64::from(i.min(6)), text.to_owned())],
            "{text:?} {i}"
        );
    }
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::caps list indentation for ""
#[test]
fn caps_list_indentation_empty() {
    caps_list("");
}

// upstream: lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::caps list indentation for "World"
#[test]
fn caps_list_indentation_world() {
    caps_list("World");
}

// upstream: lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::caps each selected item independently in a nested list
#[test]
fn caps_each_selected_item_independently() {
    let editor = create_editor();
    update(&editor, || {
        let list = create_list_node(ListType::Bullet, 1.0);
        let root = get_root();
        root.clear()?;
        root.append(std::slice::from_ref(&list))?;
        let mut parent = list;
        for text in ["Hello", "from", "the", "other", "side"] {
            let item = create_list_item_node(None);
            item.append(&[create_text_node(text)])?;
            parent.append(&[item])?;
            if text != "side" {
                let nested = create_list_node(ListType::Bullet, 1.0);
                let wrapper = create_list_item_node(None);
                wrapper.append(std::slice::from_ref(&nested))?;
                parent.append(&[wrapper])?;
                parent = nested;
            }
        }
        parent.append(&[create_list_item_node(None)])?;
        crate::select_all()?;
        Ok(())
    });
    let texts = ["Hello", "from", "the", "other", "side", ""];
    let with = |indents: [f64; 6]| -> Vec<(f64, String)> {
        indents
            .iter()
            .zip(texts)
            .map(|(i, t)| (*i, t.to_owned()))
            .collect()
    };
    indent(&editor, 3);
    assert_eq!(items(&editor), with([3.0, 4.0, 5.0, 6.0, 6.0, 6.0]));
    indent(&editor, 3);
    assert_eq!(items(&editor), with([6.0; 6]));
    indent(&editor, 3);
    assert_eq!(items(&editor), with([6.0; 6]));
    editor.dispose();
}

fn outdent_never_negative(initial: f64) {
    let editor = create_editor();
    update(&editor, move || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("Hello")])?;
        paragraph.set_indent(initial)?;
        let root = get_root();
        root.clear()?;
        root.append(std::slice::from_ref(&paragraph))?;
        paragraph.select_end()?;
        Ok(())
    });
    for i in 1..=8 {
        let inner = editor.editor().clone();
        update(&editor, move || {
            inner.dispatch_command(&OUTDENT_CONTENT_COMMAND, ())?;
            Ok(())
        });
        assert_eq!(
            paragraph_indent(&editor),
            (initial - f64::from(i)).max(0.0),
            "{initial} {i}"
        );
    }
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::outdent never makes indent 0 negative
#[test]
fn outdent_never_negative_0() {
    outdent_never_negative(0.0);
}

// upstream: lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::outdent never makes indent 0.025 negative
#[test]
fn outdent_never_negative_0_025() {
    outdent_never_negative(0.025);
}

// upstream: lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::outdent never makes indent 0.05 negative
#[test]
fn outdent_never_negative_0_05() {
    outdent_never_negative(0.05);
}

// upstream: lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::outdent never makes indent 0.075 negative
#[test]
fn outdent_never_negative_0_075() {
    outdent_never_negative(0.075);
}

// upstream: lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::outdent never makes indent 1 negative
#[test]
fn outdent_never_negative_1() {
    outdent_never_negative(1.0);
}

// upstream: lexical-extension/src/__tests__/unit/IndentationLimits.test.ts::outdent never makes indent 6 negative
#[test]
fn outdent_never_negative_6() {
    outdent_never_negative(6.0);
}
