//! Ports of `packages/lexical-text/src/__tests__/unit/registerLexicalTextEntityJoin.test.ts`.
//!
//! #9704: joining a block onto text that ends in another format (or a tab)
//! turns a hashtag at the start of the joined block into text (`word#tag`
//! holds no hashtag), and its characters keep their NodeState (comment ids
//! stored as NodeState included).

use super::support::{entity_class, regex_match, update_discrete};
use crate::extension::build_editor_from_extensions;
use crate::hashtag::{HashtagNode, create_hashtag_node, get_hashtag_match};
use crate::list::{ListItemNode, ListNode, ListType, create_list_item_node, create_list_node};
use crate::mark::{add_mark_id, get_node_mark_ids};
use crate::nodes::tab::create_tab_node;
use crate::text::{EntityMatch, register_lexical_text_entity};
use crate::upstream_tests::support::mount_in_jsdom;
use crate::{
    CreateEditorArgs, Editor, LexicalError, LexicalExtension, NodeClass, NodeRef, PointType,
    StateConfig, Which, create, create_editor, create_paragraph_node, create_range_selection,
    create_state, create_text_node, define_extension, get_root, get_selection, get_state, json,
    set_selection, set_state,
};
use std::cell::Cell;
use std::rc::Rc;

/// A NodeState on the hashtag, as a comment mark stored as NodeState is.
fn note() -> StateConfig<String> {
    thread_local! {
        static NOTE: StateConfig<String> = create_state("note", |v: &json::Value| {
            v.as_str().map(str::to_owned).unwrap_or_default()
        });
    }
    NOTE.with(Clone::clone)
}

fn create_hashtag_editor() -> Editor {
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![
            NodeClass::of::<HashtagNode>(),
            NodeClass::of::<ListNode>(),
            NodeClass::of::<ListItemNode>(),
        ],
        ..CreateEditorArgs::default()
    });
    mount_in_jsdom(&editor);
    let _ = register_lexical_text_entity(&editor, get_hashtag_match, "hashtag", |node| {
        create_hashtag_node(&node.get_text_content())
    });
    editor
}

type Row = (String, String, i64, String);

fn row(t: &str, text: &str, format: i64, state: &str) -> Row {
    (t.to_owned(), text.to_owned(), format, state.to_owned())
}

/// `[type, text, format, state]` for every text node.
fn read(editor: &Editor) -> Vec<Row> {
    let note = note();
    editor
        .read(|| {
            get_root()
                .get_all_text_nodes()
                .iter()
                .map(|n| {
                    (
                        n.get_type().to_owned(),
                        n.get_text_content(),
                        n.text_data().get_format() as i64,
                        get_state(n, &note).unwrap(),
                    )
                })
                .collect()
        })
        .unwrap()
}

/// Backspace at the start of the last block: select from the end of the
/// first block to the start of the last one and remove.
fn join_last_block() -> Result<(), LexicalError> {
    let last = get_root()
        .get_first_child_or_throw()?
        .get_last_descendant()
        .unwrap();
    let first = get_root()
        .get_last_child_or_throw()?
        .get_first_descendant()
        .unwrap();
    let s = create_range_selection();
    s.point_set(
        Which::Anchor,
        last.get_key(),
        last.get_text_content_size(),
        PointType::Text,
    )?;
    s.point_set(Which::Focus, first.get_key(), 0, PointType::Text)?;
    set_selection(Some(s.clone()));
    s.remove_text()?;
    set_selection(None);
    Ok(())
}

fn list(children: &[NodeRef]) -> Result<NodeRef, LexicalError> {
    let item = create_list_item_node(None);
    item.append(children)?;
    let list = create_list_node(ListType::Bullet, 1.0);
    list.append(&[item])?;
    Ok(list)
}

fn text(text: &str, format: &str) -> NodeRef {
    let node = create_text_node(text);
    if !format.is_empty() {
        node.set_format_type(format);
    }
    node
}

fn with_note(node: NodeRef, value: &str) -> NodeRef {
    set_state(&node, &note(), value.to_owned()).unwrap()
}

fn paragraph(children: &[NodeRef]) -> NodeRef {
    let paragraph = create_paragraph_node();
    paragraph.append(children).unwrap();
    paragraph
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityJoin.test.ts::joining blocks before a hashtag keeps its NodeState (#9704) > #9704: a list item that starts with a hashtag, joined onto italic text
#[test]
fn a_list_item_that_starts_with_a_hashtag_joined_onto_italic_text() {
    let editor = create_hashtag_editor();
    update_discrete(&editor, || {
        get_root().append(&[
            paragraph(&[text("one ", ""), text("word", "italic")]),
            list(&[
                with_note(create_hashtag_node("#tag"), "c1"),
                text(" more", ""),
            ])?,
        ])
    });
    update_discrete(&editor, join_last_block);
    // "word#tag" is no hashtag (a fresh parse finds none), so the hashtag
    // becomes text; its characters keep their NodeState.
    assert_eq!(
        read(&editor),
        [
            row("text", "one ", 0, ""),
            row("text", "word", 2, ""),
            row("text", "#tag", 0, "c1"),
            row("text", " more", 0, ""),
        ]
    );
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityJoin.test.ts::joining blocks before a hashtag keeps its NodeState (#9704) > the same with backspace from the start of the item (deleteCharacter)
#[test]
fn the_same_with_backspace_from_the_start_of_the_item() {
    let editor = create_hashtag_editor();
    update_discrete(&editor, || {
        get_root().append(&[
            paragraph(&[text("one ", ""), text("word", "italic")]),
            paragraph(&[
                with_note(create_hashtag_node("#tag"), "c1"),
                text(" more", ""),
            ]),
        ])
    });
    update_discrete(&editor, || {
        let hashtag = get_root()
            .get_last_child_or_throw()?
            .get_first_descendant()
            .unwrap();
        hashtag.select(Some(0), Some(0))?.delete_character(true)?;
        set_selection(None);
        Ok(())
    });
    assert_eq!(
        read(&editor),
        [
            row("text", "one ", 0, ""),
            row("text", "word", 2, ""),
            row("text", "#tag", 0, "c1"),
            row("text", " more", 0, ""),
        ]
    );
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityJoin.test.ts::joining blocks before a hashtag keeps its NodeState (#9704) > forward delete at the end of the text before it
#[test]
fn forward_delete_at_the_end_of_the_text_before_it() {
    let editor = create_hashtag_editor();
    update_discrete(&editor, || {
        get_root().append(&[
            paragraph(&[text("one ", ""), text("word", "bold")]),
            paragraph(&[with_note(create_hashtag_node("#tag"), "c1")]),
        ])
    });
    update_discrete(&editor, || {
        let word = get_root()
            .get_first_child_or_throw()?
            .get_last_descendant()
            .unwrap();
        word.select(Some(4), Some(4))?.delete_character(false)?;
        set_selection(None);
        Ok(())
    });
    assert_eq!(
        read(&editor),
        [
            row("text", "one ", 0, ""),
            row("text", "word", 1, ""),
            row("text", "#tag", 0, "c1"),
        ]
    );
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityJoin.test.ts::joining blocks before a hashtag keeps its NodeState (#9704) > the block before ends with a tab, the text and tab carry NodeState
#[test]
fn the_block_before_ends_with_a_tab_the_text_and_tab_carry_node_state() {
    // From a long-session test: a block ending with a tab (both with comment
    // marks) joined with a list item starting with a commented hashtag.
    let editor = create_hashtag_editor();
    update_discrete(&editor, || {
        get_root().append(&[
            paragraph(&[
                with_note(create_text_node("t5015z t5016z"), "c1,c4"),
                with_note(create_tab_node(), "c1,c4"),
            ]),
            list(&[
                with_note(create_hashtag_node("#t5017z"), "c1,c4"),
                with_note(create_text_node(" t5018z"), "c4"),
            ])?,
        ])
    });
    update_discrete(&editor, join_last_block);
    let rows = read(&editor);
    assert_eq!(
        editor.read(|| get_root().get_text_content()).unwrap(),
        "t5015z t5016z\t#t5017z t5018z"
    );
    // Whatever the hashtag becomes, its characters keep their NodeState
    assert_eq!(
        rows.iter()
            .find(|r| r.1.contains("#t5017z"))
            .map(|r| r.3.as_str()),
        Some("c1,c4")
    );
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityJoin.test.ts::with MarkStateExtension comments (#9704) > a commented hashtag joined onto text in another format keeps its comment ids
#[test]
fn a_commented_hashtag_joined_onto_text_in_another_format_keeps_its_comment_ids() {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            crate::list::list_extension().into(),
            crate::hashtag::hashtag_extension().into(),
            crate::mark::mark_extension().into(),
            crate::mark::mark_state_extension().into(),
        ],
        ..LexicalExtension::<(), (), ()>::new("test", ())
    })
    .into()])
    .unwrap();
    update_discrete(&editor, || {
        get_root().clear()?;
        get_root().append(&[
            paragraph(&[text("one ", ""), text("word", "italic")]),
            list(&[create_hashtag_node("#tag"), text(" more", "")])?,
        ])
    });
    update_discrete(&editor, || {
        let hashtag = get_root()
            .get_last_child_or_throw()?
            .get_first_descendant()
            .unwrap();
        add_mark_id("c1", &hashtag.select(Some(0), Some(4))?)?;
        add_mark_id("c4", &hashtag.select(Some(0), Some(4))?)?;
        set_selection(None);
        Ok(())
    });
    update_discrete(&editor, join_last_block);
    let ids = editor
        .read(|| {
            get_root()
                .get_all_text_nodes()
                .iter()
                .map(|n| {
                    (
                        n.get_text_content(),
                        get_node_mark_ids(n).unwrap().join(","),
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap();
    assert_eq!(
        ids.into_iter().find(|(text, _)| text.contains("#tag")),
        Some(("#tag".to_owned(), "c1,c4".to_owned()))
    );
    editor.dispose();
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityJoin.test.ts::an entity turned back into text does not take the text after it first > two hashtags turned into text: the text after them keeps its format, NodeState and the selection
#[test]
fn two_hashtags_turned_into_text_the_text_after_them_keeps_its_format_node_state_and_the_selection()
{
    let editor = create_hashtag_editor();
    update_discrete(&editor, || {
        let suffix = with_note(text("z!", "italic"), "suffix");
        let b = create_hashtag_node("#b");
        b.set_format_type("bold");
        get_root().append(&[paragraph(&[
            text("x", ""),
            text("a", "bold"),
            b,
            with_note(create_hashtag_node("#c"), "entity"),
            suffix.clone(),
        ])])?;
        suffix.select(Some(2), Some(2))?;
        Ok(())
    });
    assert_eq!(
        read(&editor),
        [
            row("text", "x", 0, ""),
            row("text", "a#b", 1, ""),
            row("text", "#c", 0, "entity"),
            row("text", "z!", 2, "suffix"),
        ]
    );
    editor.get_editor_state().read(|| {
        let selection = get_selection().filter(crate::SelectionRef::is_range);
        let selection = selection.expect("a range selection");
        let anchor = selection.point(Which::Anchor).unwrap();
        assert_eq!(
            (anchor.get_node().unwrap().get_text_content(), anchor.offset),
            ("z!".to_owned(), 2)
        );
    });
}

entity_class!(MentionNode, "join-mention");
type GetMatch = Rc<dyn Fn(&str) -> Option<EntityMatch>>;

entity_class!(SegmentedNode, "join-segmented");
entity_class!(PhraseNode, "join-phrase");

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityJoin.test.ts::entities of two registrations that touch > typing before a hashtag that a token and a segmented entity follow settles
#[test]
fn typing_before_a_hashtag_that_a_token_and_a_segmented_entity_follow_settles() {
    let calls = Rc::new(Cell::new(0));
    let counted = |get_match: GetMatch| {
        let calls = calls.clone();
        move |text: &str| {
            calls.set(calls.get() + 1);
            get_match(text)
        }
    };
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![
            NodeClass::of::<HashtagNode>(),
            NodeClass::of::<MentionNode>(),
            NodeClass::of::<SegmentedNode>(),
        ],
        ..CreateEditorArgs::default()
    });
    let _ = register_lexical_text_entity(
        &editor,
        counted(Rc::new(get_hashtag_match)),
        "hashtag",
        |node| create_hashtag_node(&node.get_text_content()),
    );
    let _ = register_lexical_text_entity(
        &editor,
        counted(Rc::new(regex_match("@[a-z]+", ""))),
        "join-mention",
        |node| {
            let mention = create::<MentionNode>();
            mention.set_text_content(&node.get_text_content());
            mention.set_mode("token").unwrap();
            mention
        },
    );
    let _ = register_lexical_text_entity(
        &editor,
        counted(Rc::new(regex_match(r"\$[A-Z]+", ""))),
        "join-segmented",
        |node| {
            let segmented = create::<SegmentedNode>();
            segmented.set_text_content(&node.get_text_content());
            segmented.set_mode("segmented").unwrap();
            segmented
        },
    );
    update_discrete(&editor, || {
        get_root().append(&[paragraph(&[
            text("x", ""),
            text(" #b", "bold"),
            text("$B@x", "code"),
        ])])
    });
    calls.set(0);
    update_discrete(&editor, || {
        let space = get_root().get_all_text_nodes()[1].clone();
        let selection = space.select(Some(1), Some(1))?;
        selection.set_format(space.text_data().get_format());
        selection.insert_text("a")
    });
    assert_eq!(
        editor.read(|| get_root().get_text_content()).unwrap(),
        "x a#b$B@x"
    );
    // The transforms of the two that touch run a bounded number of times
    assert!(calls.get() < 200, "{}", calls.get());
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityJoin.test.ts::an entity that may not be kept does not grow > after text with 7 earlier matches, a later match in the text after it is still made
// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityJoin.test.ts::an entity that may not be kept does not grow > after text with 8 earlier matches, a later match in the text after it is still made
#[test]
fn after_text_with_earlier_matches_a_later_match_in_the_text_after_it_is_still_made() {
    for repeat in [7, 8] {
        let editor = create_editor(CreateEditorArgs {
            nodes: vec![NodeClass::of::<HashtagNode>()],
            ..CreateEditorArgs::default()
        });
        let _ = register_lexical_text_entity(&editor, get_hashtag_match, "hashtag", |node| {
            create_hashtag_node(&node.get_text_content())
        });
        update_discrete(&editor, || {
            get_root().append(&[paragraph(&[
                text("#", "bold"),
                text("a ", ""),
                text("#b", "italic"),
            ])])
        });
        update_discrete(&editor, move || {
            get_root()
                .get_all_text_nodes()
                .into_iter()
                .find(|n| n.get_text_content() == "a ")
                .unwrap()
                .splice_text(2, 0, &"#x ".repeat(repeat), false)?;
            Ok(())
        });
        update_discrete(&editor, || {
            get_root()
                .get_all_text_nodes()
                .into_iter()
                .find(|n| n.get_text_content() == "#b")
                .unwrap()
                .select_end()?
                .insert_nodes(vec![text("c rest #d", "bold")])
        });
        editor
            .read(|| {
                assert_eq!(
                    get_root().get_text_content(),
                    format!("#a {}#bc rest #d", "#x ".repeat(repeat))
                );
                let hashtags: Vec<String> = get_root()
                    .get_all_text_nodes()
                    .iter()
                    .filter(|n| n.get_type() == "hashtag")
                    .map(|n| n.get_text_content())
                    .collect();
                assert!(
                    hashtags.contains(&"#d".to_owned()),
                    "{repeat}: {hashtags:?}"
                );
            })
            .unwrap();
    }
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityJoin.test.ts::an entity that may not be kept does not grow > after text with 7 earlier matches, the text after it keeps its format and NodeState
// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityJoin.test.ts::an entity that may not be kept does not grow > after text with 8 earlier matches, the text after it keeps its format and NodeState
#[test]
fn after_text_with_earlier_matches_the_text_after_it_keeps_its_format_and_node_state() {
    let phrase_re = crate::markdown::RegExp::new("^a .*cat$", "").unwrap();
    let phrase_match = Rc::new(move |text: &str| {
        phrase_re.exec(text).map(|_| EntityMatch {
            start: 0,
            end: crate::utf16::len(text),
        })
    });
    for repeat in [7, 8] {
        let editor = create_editor(CreateEditorArgs {
            nodes: vec![
                NodeClass::of::<HashtagNode>(),
                NodeClass::of::<PhraseNode>(),
            ],
            ..CreateEditorArgs::default()
        });
        let _ = register_lexical_text_entity(&editor, get_hashtag_match, "hashtag", |node| {
            create_hashtag_node(&node.get_text_content())
        });
        let pm = phrase_match.clone();
        let _ = register_lexical_text_entity(
            &editor,
            move |t| pm(t),
            "join-phrase",
            |node| {
                let phrase = create::<PhraseNode>();
                phrase.set_text_content(&node.get_text_content());
                phrase
            },
        );
        let phrase_text = format!("a {}cat", "#x ".repeat(repeat));
        let pt = phrase_text.clone();
        update_discrete(&editor, move || {
            // Made in document order: the transforms run in that order
            let first = create_text_node("y");
            let bold = text("x", "bold");
            let phrase = create::<PhraseNode>();
            phrase.set_text_content(&pt);
            phrase.set_format_type("bold");
            let hashtag = with_note(create_hashtag_node("#c"), "entity");
            let suffix = with_note(text("z!", "italic"), "suffix");
            get_root().append(&[paragraph(&[
                first,
                bold,
                phrase,
                hashtag.clone(),
                suffix.clone(),
            ])])?;
            // Comment ids too, as MarkStateExtension keeps them
            add_mark_id("entity", &hashtag.select(Some(0), Some(2))?)?;
            add_mark_id("suffix", &suffix.select(Some(0), Some(2))?)?;
            suffix.select(Some(2), Some(2))?;
            Ok(())
        });
        let note = note();
        editor
            .read(|| {
                assert_eq!(
                    get_root().get_text_content(),
                    format!("yx{phrase_text}#cz!")
                );
                let suffix = get_root()
                    .get_all_text_nodes()
                    .into_iter()
                    .find(|n| n.get_text_content().contains('z'))
                    .unwrap();
                assert_eq!(
                    (
                        suffix.get_text_content(),
                        suffix.text_data().get_format() as i64,
                        get_state(&suffix, &note).unwrap(),
                        get_node_mark_ids(&suffix).unwrap(),
                    ),
                    (
                        "z!".to_owned(),
                        2,
                        "suffix".to_owned(),
                        vec!["suffix".to_owned()]
                    ),
                    "{repeat}"
                );
                let selection = get_selection()
                    .filter(crate::SelectionRef::is_range)
                    .unwrap();
                let anchor = selection.point(Which::Anchor).unwrap();
                assert_eq!(
                    (
                        anchor.get_node().unwrap().get_key() == suffix.get_key(),
                        anchor.offset
                    ),
                    (true, 2)
                );
            })
            .unwrap();
    }
}
