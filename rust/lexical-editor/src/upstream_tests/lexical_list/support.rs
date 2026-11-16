//! Helpers shared by the ports of `packages/lexical-list`'s unit tests.

#![allow(dead_code)]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::list::{ListType, create_list_item_node, create_list_node};
use crate::{
    AnyLexicalExtensionArgument, Editor, LexicalError, LexicalExtension, NodeRef, UpdateOptions,
    create_text_node, define_extension,
};

/// `{discrete: true}`.
pub(crate) fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

/// `editor.update(fn, {discrete: true})`, on any editor.
pub(crate) fn update_discrete(
    editor: &Editor,
    f: impl FnOnce() -> Result<(), LexicalError> + 'static,
) {
    editor.update(f, discrete()).unwrap();
}

/// `buildEditorFromExtensions(defineExtension({dependencies, name}))`.
pub(crate) fn build(
    name: &str,
    dependencies: Vec<AnyLexicalExtensionArgument>,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

/// `$createListNode(type)` with `children` appended.
pub(crate) fn list(list_type: ListType, children: &[NodeRef]) -> NodeRef {
    let list = create_list_node(list_type, 1.0);
    list.append(children).unwrap();
    list
}

/// `$createListItemNode().append(...children)`.
pub(crate) fn item(children: &[NodeRef]) -> NodeRef {
    let item = create_list_item_node(None);
    item.append(children).unwrap();
    item
}

/// `$createListItemNode().append($createTextNode(text))`.
pub(crate) fn text_item(text: &str) -> NodeRef {
    item(&[create_text_node(text)])
}

/// `prettifyHtml` as `expectHtmlToBeEqual` compares it: the HTML without
/// its line breaks and the whitespace between tags.
pub(crate) fn prettify_html(s: &str) -> String {
    let s = s.replace('\n', "");
    let mut out = String::new();
    let mut pending = String::new();
    for c in s.chars() {
        if c.is_whitespace() {
            pending.push(c);
            continue;
        }
        // Whitespace between two tags, or before the first, is dropped.
        let between_tags = c == '<' && (out.is_empty() || out.ends_with('>'));
        if !between_tags {
            out.push_str(&pending);
        }
        pending.clear();
        out.push(c);
    }
    out
}
