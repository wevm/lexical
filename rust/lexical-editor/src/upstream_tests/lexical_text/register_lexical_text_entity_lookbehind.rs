//! Ports of `packages/lexical-text/src/__tests__/unit/registerLexicalTextEntityLookbehind.test.ts`.

use super::support::{entity_class, regex_match, update_discrete};
use crate::hashtag::{HashtagNode, create_hashtag_node, get_hashtag_match};
use crate::text::register_lexical_text_entity;
use crate::{
    CreateEditorArgs, NodeClass, create, create_editor, create_paragraph_node, create_state,
    create_text_node, get_root, get_state, json, set_state,
};

entity_class!(Letter, "letter");

// #9725 (known limit, see transforms.md): typing into a run of repeated
// characters, read as text inserted at the end of the run, drops the NodeState
// of the last existing character when a matcher that looks back into the run
// makes it an entity. `test.fails` upstream until the edit's reading is fixed.

/// Types an "A" into "$AA" right after "$" and returns the characters with
/// their NodeState, as `(note, text)`.
fn type_into_run() -> Vec<(String, String)> {
    let note = create_state("note", |v: &json::Value| {
        v.as_str().map(str::to_owned).unwrap_or_default()
    });
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![NodeClass::of::<HashtagNode>(), NodeClass::of::<Letter>()],
        ..CreateEditorArgs::default()
    });
    let _ = register_lexical_text_entity(&editor, get_hashtag_match, "hashtag", |node| {
        create_hashtag_node(&node.get_text_content())
    });
    // Matches an "A" only when "$AA" comes right before it.
    let _ = register_lexical_text_entity(
        &editor,
        regex_match(r"^x$|^A$|(?<=^#)x|(?<=\$AA)A", ""),
        "letter",
        |node| {
            let letter = create::<Letter>();
            letter.set_text_content(&node.get_text_content());
            letter
        },
    );
    let n = note.clone();
    update_discrete(&editor, move || {
        let y = create_text_node("y");
        y.set_format_type("bold");
        let run = create_text_node("$AA");
        set_state(&run, &n, "c1".to_owned())?;
        let paragraph = create_paragraph_node();
        paragraph.append(&[y, create_text_node("a"), create_hashtag_node("#x"), run])?;
        get_root().append(&[paragraph])
    });
    update_discrete(&editor, || {
        let node = get_root()
            .get_all_text_nodes()
            .into_iter()
            .find(|node| node.get_text_content() == "$AA")
            .unwrap();
        node.select(Some(1), Some(1))?.insert_text("A")
    });
    let nodes = editor
        .read(|| {
            get_root()
                .get_all_text_nodes()
                .iter()
                .map(|node| (get_state(node, &note).unwrap(), node.get_text_content()))
                .collect::<Vec<_>>()
        })
        .unwrap();
    // `text.split('')`: one entry per UTF-16 code unit (all ASCII here)
    nodes
        .into_iter()
        .flat_map(|(note, text)| {
            text.chars()
                .map(|c| (note.clone(), c.to_string()))
                .collect::<Vec<_>>()
        })
        .collect()
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityLookbehind.test.ts::typing into a run of repeated characters keeps the text
#[test]
fn typing_into_a_run_of_repeated_characters_keeps_the_text() {
    // the typed A is at index 5
    let text: String = type_into_run().into_iter().map(|(_, text)| text).collect();
    assert_eq!(text, "ya#x$AAA");
}

// `test.fails` upstream: the assertion is expected to fail (#9725), so this
// test passes only while it does.
// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityLookbehind.test.ts::typing into a run of repeated characters keeps NodeState on an existing character
#[test]
fn typing_into_a_run_of_repeated_characters_keeps_node_state_on_an_existing_character() {
    // the A that was last before typing is at index 7
    let chars = type_into_run();
    assert_ne!(
        chars.get(7),
        Some(&("c1".to_owned(), "A".to_owned())),
        "#9725 is fixed: make this an assert_eq, as upstream's test.fails will be a test"
    );
}
