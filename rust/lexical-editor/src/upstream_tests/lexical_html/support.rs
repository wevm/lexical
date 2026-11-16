//! Helpers for the ports of `packages/lexical-html`'s unit tests.

use crate::dom::{DomNode, parse_from_string};

/// What `expectHtmlToBeEqual` compares: upstream formats both sides with
/// prettier and `prettier-plugin-organize-attributes` (the repository's
/// `.prettierrc`: `attributeGroups` `class`, `id`/`name`, the rest,
/// `aria-*`, `data-*`, each sorted ascending), so attribute order doesn't
/// count. The serialization of `html` with each element's attributes in
/// that order (the expectations are written without prettier's
/// formatting whitespace).
#[cfg_attr(not(feature = "extension"), allow(dead_code))]
pub(super) fn organize_attributes(html: &str) -> String {
    let doc = parse_from_string(&format!("<body>{html}</body>"));
    let body = doc.body().expect("a body");
    let mut out = String::new();
    for child in body.child_nodes() {
        write_node(&child, &mut out);
    }
    out
}

fn group(name: &str) -> u8 {
    match name {
        "class" => 0,
        "id" | "name" => 1,
        _ if name.starts_with("aria-") => 3,
        _ if name.starts_with("data-") => 4,
        _ => 2,
    }
}

fn write_node(node: &DomNode, out: &mut String) {
    if !node.is_element() {
        // A text node's or comment's serialization.
        let container = crate::dom::get_document().create_element("div");
        container.append_child(&node.clone_node(true));
        out.push_str(&container.inner_html());
        return;
    }
    let tag = node.tag_name().to_lowercase();
    let mut attributes = node.attributes();
    attributes.sort_by(|(a, _), (b, _)| (group(a), a).cmp(&(group(b), b)));
    out.push('<');
    out.push_str(&tag);
    for (name, value) in attributes {
        out.push_str(&format!(" {name}=\"{value}\""));
    }
    out.push('>');
    for child in node.child_nodes() {
        write_node(&child, out);
    }
    if !crate::dom::is_void_html_element(&tag) {
        out.push_str(&format!("</{tag}>"));
    }
}

/// `buildEditorFromExtensions(defineExtension({dependencies, name}))`.
#[cfg(feature = "extension")]
pub(super) fn build(
    name: &str,
    dependencies: Vec<crate::AnyLexicalExtensionArgument>,
) -> crate::extension::LexicalEditorWithDispose {
    crate::extension::build_editor_from_extensions([crate::define_extension(
        crate::LexicalExtension {
            dependencies,
            ..crate::LexicalExtension::<(), (), ()>::new(name, ())
        },
    )
    .into()])
    .unwrap()
}

/// `$generate(html)`: the `DOMImportExtension` import of `html` in a
/// document's body.
#[cfg(feature = "extension")]
pub(super) fn generate(html: &str) -> Result<Vec<crate::NodeRef>, crate::LexicalError> {
    let dom = parse_from_string(&format!("<!doctype html><html><body>{html}</body></html>"));
    crate::html::import::generate_nodes_from_dom_via_extension(&dom, None)
}

/// `importInto(editor, html)`: `$getRoot().clear().append(...$generate(html))`.
#[cfg(feature = "extension")]
pub(super) fn import_into(editor: &crate::Editor, html: &str) {
    let html = html.to_owned();
    super::super::support::update(editor, move || {
        let nodes = generate(&html)?;
        let root = crate::get_root();
        root.clear()?;
        root.append(&nodes)
    });
}

/// `importInto(editor, html)` with `splice(0, 0, nodes)` in place of
/// `append`, as some files write it.
#[cfg(feature = "extension")]
pub(super) fn import_into_splice(editor: &crate::Editor, html: &str) {
    let html = html.to_owned();
    super::super::support::update(editor, move || {
        let nodes = generate(&html)?;
        let root = crate::get_root();
        root.clear()?;
        root.splice(0, 0, &nodes)
    });
}

/// `$findTextByContent(content)`.
#[cfg(feature = "extension")]
pub(super) fn find_text_by_content(content: &str) -> crate::NodeRef {
    crate::get_root()
        .get_all_text_nodes()
        .into_iter()
        .find(|n| n.get_text_content() == content)
        .unwrap_or_else(|| panic!("No TextNode with text {content:?} found"))
}

/// `counter(prefix)` behind `withIds(ids, ...)`: hands out `ids` first,
/// then `${prefix}1`, `${prefix}2`, ...
#[cfg(feature = "extension")]
pub(super) fn with_ids(ids: &[&str], prefix: &str) -> crate::extension::GenerateId {
    let ids: Vec<String> = ids.iter().map(|id| (*id).to_owned()).collect();
    let prefix = prefix.to_owned();
    let (i, n) = (std::cell::Cell::new(0), std::cell::Cell::new(0));
    crate::extension::GenerateId(std::rc::Rc::new(move || {
        if i.get() < ids.len() {
            i.set(i.get() + 1);
            return ids[i.get() - 1].clone();
        }
        n.set(n.get() + 1);
        format!("{prefix}{}", n.get())
    }))
}

/// `asEdited(editor)`: the editor has had content once, so the first
/// blocks a test adds get ids from `generateId`, not positional ones.
#[cfg(feature = "extension")]
pub(super) fn as_edited(editor: &crate::Editor) {
    for clear in [false, true] {
        editor
            .update(
                move || {
                    let root = crate::get_root();
                    if clear {
                        root.clear()
                    } else {
                        root.append(&[crate::create_paragraph_node()])
                    }
                },
                crate::UpdateOptions {
                    discrete: true,
                    tags: vec![crate::HISTORY_MERGE_TAG.to_owned()],
                    ..crate::UpdateOptions::default()
                },
            )
            .unwrap();
    }
}

/// Runs `f` on a thread with a large stack, for imports nested a hundred
/// elements deep (the walk recurses per element, more deeply in a debug
/// build than in JS).
#[cfg(feature = "extension")]
pub(super) fn with_large_stack<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> R {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap_or_else(|e| std::panic::resume_unwind(e))
}

/// `$setBlockText(block, text)` of `__tests__/utils/setBlockText.ts` (the
/// Block IDs documentation page's): changes a block's text in place, only
/// the UTF-16 units that differ, in its own text nodes; `false`, changing
/// nothing, unless the block holds only simple text, directly or in inline
/// elements.
#[cfg(feature = "extension")]
pub(super) fn set_block_text(block: &crate::NodeRef, text: &str) -> bool {
    if !block.is_element() || !holds_only_text(block) {
        return false;
    }
    let nodes = block.get_all_text_nodes();
    let old: Vec<u16> = nodes
        .iter()
        .map(crate::NodeRef::get_text_content)
        .collect::<String>()
        .encode_utf16()
        .collect();
    let new: Vec<u16> = text.encode_utf16().collect();
    let mut start = 0;
    while start < old.len() && Some(&old[start]) == new.get(start) {
        start += 1;
    }
    let mut end = 0;
    while end < old.len() - start
        && end < new.len().saturating_sub(start)
        && old[old.len() - 1 - end] == new[new.len() - 1 - end]
    {
        end += 1;
    }
    let inserted = String::from_utf16_lossy(&new[start..new.len() - end]);
    let delete_end = old.len() - end;
    let (start, delete_end) = (start as i64, delete_end as i64);
    let mut offset: i64 = 0;
    let mut done = false;
    for node in &nodes {
        let size = node.get_text_content_size() as i64;
        let from = (start - offset).max(0);
        let to = (delete_end - offset).min(size);
        if !done && (start - offset < size || (start - offset == size && start == delete_end)) {
            // The node where the change starts (for an insert at a node
            // boundary, the node before it): delete and insert there.
            if start == delete_end {
                insert_text(node, from, &inserted);
            } else {
                node.splice_text(from, to - from, &inserted, false).unwrap();
            }
            done = true;
        } else if to > from {
            node.splice_text(from, to - from, "", false).unwrap();
        }
        offset += size;
    }
    if !done && !inserted.is_empty() {
        block.append(&[crate::create_text_node(&inserted)]).unwrap();
    }
    true
}

/// `$holdsOnlyText(element)`.
#[cfg(feature = "extension")]
fn holds_only_text(element: &crate::NodeRef) -> bool {
    // Named slots hold blocks of their own (and count in getTextContent).
    crate::slot::get_slot_names(element).is_empty()
        && element.get_children().iter().all(|child| {
            (child.is_text() && child.is_simple_text())
                || (child.is_element() && child.is_inline() && holds_only_text(child))
        })
}

/// `$insertText(node, at, text)`: inserts at `at`, or, at an edge of
/// inline elements that take no text there, beside the outermost of them.
#[cfg(feature = "extension")]
fn insert_text(node: &crate::NodeRef, at: i64, text: &str) {
    if text.is_empty() {
        return;
    }
    let at_end = at == node.get_text_content_size() as i64;
    let mut outermost = None;
    let mut inner = node.clone();
    let mut parent = node.get_parent();
    while let Some(p) = parent.clone().filter(|p| p.is_element() && p.is_inline()) {
        let blocked = if at_end {
            inner.get_next_sibling().is_some()
        } else {
            at != 0 || inner.get_previous_sibling().is_some()
        };
        if blocked {
            break;
        }
        if if at_end {
            !p.can_insert_text_after()
        } else {
            !p.can_insert_text_before()
        } {
            outermost = Some(p.clone());
        }
        parent = p.get_parent();
        inner = p;
    }
    let Some(outermost) = outermost else {
        node.splice_text(at, 0, text, false).unwrap();
        return;
    };
    let beside = if at_end {
        outermost.get_next_sibling()
    } else {
        outermost.get_previous_sibling()
    };
    match beside.filter(|b| b.is_text() && b.is_simple_text()) {
        Some(beside) => {
            let at = if at_end {
                0
            } else {
                beside.get_text_content_size() as i64
            };
            beside.splice_text(at, 0, text, false).unwrap();
        }
        None if at_end => {
            outermost
                .insert_after(&crate::create_text_node(text), true)
                .unwrap();
        }
        None => {
            outermost
                .insert_before(&crate::create_text_node(text), true)
                .unwrap();
        }
    }
}
