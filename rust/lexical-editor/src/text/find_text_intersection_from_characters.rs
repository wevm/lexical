//! `packages/lexical-text/src/findTextIntersectionFromCharacters.ts`.

use super::TextNodeWithOffset;
use crate::NodeRef;

/// `$findTextIntersectionFromCharacters(root, targetCharacters)`: the first
/// text node, in document order, whose text takes the count of the text
/// nodes' characters (UTF-16 code units) past `target_characters`, and the
/// offset of that character in it; `None` when there's no such node.
///
/// Deprecated upstream: "has never worked correctly and will be removed".
pub fn find_text_intersection_from_characters(
    root: &NodeRef,
    target_characters: usize,
) -> Option<TextNodeWithOffset> {
    let mut node = root.get_first_child();
    let mut current_characters = 0;

    'main_loop: while let Some(current) = node {
        if current.is_element() {
            if let Some(child) = current.get_first_child() {
                node = Some(child);
                continue;
            }
        } else if current.is_text() {
            let characters = current.get_text_content_size();

            if current_characters + characters > target_characters {
                return Some(TextNodeWithOffset {
                    node: current,
                    offset: target_characters - current_characters,
                });
            }
            current_characters += characters;
        }
        if let Some(sibling) = current.get_next_sibling() {
            node = Some(sibling);
            continue;
        }
        let mut parent = current.get_parent();
        while let Some(p) = parent {
            if let Some(parent_sibling) = p.get_next_sibling() {
                node = Some(parent_sibling);
                continue 'main_loop;
            }
            parent = p.get_parent();
        }
        break;
    }

    None
}
