//! Ports of `packages/lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeStateProperty.test.ts`.
//!
//! Property: in random layouts of entities and text in several formats, with
//! NodeState on entities and text and some text in MarkNodes, an edit that
//! turns an entity back into text leaves every character of that entity that
//! is still there with the entity's NodeState (or inside another entity that
//! grew over it, which takes characters with its own NodeState), and inside
//! the same MarkNode. Each entity is written with a letter no other text has,
//! which tells its characters apart after the edit.

use super::support::{discrete, entity_class, regex_match};
use crate::hashtag::{HashtagNode, create_hashtag_node, get_hashtag_match};
use crate::mark::{MarkNode, create_mark_node, is_mark_node};
use crate::text::register_lexical_text_entity;
use crate::upstream_tests::support::{mount_in_jsdom, panic_message};
use crate::{
    CreateEditorArgs, Editor, NodeClass, NodeRef, PointType, StateConfig, Which,
    apply_node_replacement, create_editor, create_paragraph_node, create_range_selection,
    create_state, create_text_node, get_root, get_state, json, set_selection, set_state,
};
use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    static NOTE: StateConfig<String> = create_state("note", |v: &json::Value| {
        v.as_str().map(str::to_owned).unwrap_or_default()
    });
}

fn note() -> StateConfig<String> {
    NOTE.with(Clone::clone)
}

entity_class!(MentionNode, "property-mention");

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Hashtag,
    Mention,
}

/// `process.env.TEXT_ENTITY_NODE_STATE_SEEDS ?? 200`.
fn seeds() -> u32 {
    std::env::var("TEXT_ENTITY_NODE_STATE_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(200)
}

const ENTITY_LETTERS: &str = "abcdefghijklmnop";
const PLAIN: [&str; 9] = [" ", " ", "x", "y", ".", "#", "@", "xy", " x "];
const TYPED: [&str; 6] = [" ", "x", "#", ".", "@", "z"];
const FORMATS: [f64; 4] = [0.0, 0.0, 1.0, 2.0];

/// Mulberry32, so a failure names a seed that reproduces it.
fn rng(seed: u32) -> impl FnMut() -> f64 {
    let mut a = seed;
    move || {
        a = a.wrapping_add(0x6d2b_79f5);
        let mut t = a;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

fn create_mention(text: &str) -> NodeRef {
    let node = apply_node_replacement(&MentionNode::new_node(text)).unwrap();
    node.set_mode("token").unwrap();
    node
}

fn create_entity_editor(kind: Kind) -> Editor {
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![
            NodeClass::of::<HashtagNode>(),
            NodeClass::of::<MentionNode>(),
            NodeClass::of::<MarkNode>(),
        ],
        ..CreateEditorArgs::default()
    });
    mount_in_jsdom(&editor);
    let _ = if kind == Kind::Hashtag {
        register_lexical_text_entity(&editor, get_hashtag_match, "hashtag", |node| {
            create_hashtag_node(&node.get_text_content())
        })
    } else {
        register_lexical_text_entity(
            &editor,
            regex_match("@[a-z0-9]+", ""),
            "property-mention",
            |node| create_mention(&node.get_text_content()),
        )
    };
    editor
}

fn create_entity(kind: Kind, text: &str) -> NodeRef {
    match kind {
        Kind::Hashtag => create_hashtag_node(text),
        Kind::Mention => create_mention(text),
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Info {
    entity: bool,
    mark: String,
    note: String,
    text: String,
}

fn read(editor: &Editor) -> Vec<Info> {
    let note = note();
    editor
        .read(|| {
            get_root()
                .get_all_text_nodes()
                .iter()
                .map(|n| {
                    let parent = n.get_parent();
                    Info {
                        entity: n.is_text_entity(),
                        mark: parent
                            .filter(is_mark_node)
                            .and_then(|p| p.get_ids())
                            .map(|ids| ids.join(","))
                            .unwrap_or_default(),
                        note: get_state(n, &note).unwrap(),
                        text: n.get_text_content(),
                    }
                })
                .collect()
        })
        .unwrap()
}

fn check(label: &str, before: &[Info], after: &[Info], failures: &mut Vec<String>) {
    // The entities before the edit, by their own letter
    for node in before {
        if !node.entity || node.note.is_empty() {
            continue;
        }
        let Some(letter) = node.text.chars().find(|c| ENTITY_LETTERS.contains(*c)) else {
            continue;
        };
        for now in after {
            if !now.text.contains(letter) {
                continue;
            }
            if now.note == node.note && now.mark == node.mark {
                continue;
            }
            // Characters that another entity grew over (an entity takes the
            // text its match extends into with its own NodeState, as
            // before): not at the start of an entity, right after its sigil
            let grown_over = now.entity && now.text.find(letter).is_some_and(|i| i > 1);
            if grown_over {
                continue;
            }
            failures.push(format!(
                "{label}: the characters of {node:?} are in {now:?}; before {before:?} after {after:?}"
            ));
            return;
        }
    }
}

fn pick<T: Clone>(random: &mut impl FnMut() -> f64, values: &[T]) -> T {
    values[(random() * values.len() as f64) as usize].clone()
}

fn an_edit_that_turns_an_entity_into_text_keeps_its_node_state_and_marks(kind: Kind) {
    let mut failures: Vec<String> = Vec::new();
    let mut demoted = 0;
    let mut seed = 0;
    while seed < seeds() && failures.len() < 3 {
        let random = Rc::new(RefCell::new(rng(
            seed * 11 + if kind == Kind::Hashtag { 1 } else { 2 }
        )));
        let editor = create_entity_editor(kind);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let r = random.clone();
            editor.update(
                move || {
                    let mut random = || (r.borrow_mut())();
                    let mut letter = 0;
                    let blocks = 1 + (random() * 3.0) as usize;
                    for _ in 0..blocks {
                        let paragraph = create_paragraph_node();
                        let runs = 1 + (random() * 5.0) as usize;
                        for _ in 0..runs {
                            let node;
                            if random() < 0.45 && letter < ENTITY_LETTERS.len() {
                                let own = &ENTITY_LETTERS[letter..letter + 1];
                                letter += 1;
                                let sigil = if kind == Kind::Hashtag { "#" } else { "@" };
                                let one = if random() < 0.5 { "1" } else { "" };
                                node = set_state(
                                    &create_entity(kind, &format!("{sigil}{own}{one}")),
                                    &note(),
                                    format!("e{letter}"),
                                )?;
                            } else {
                                let text = pick(&mut random, &PLAIN).to_owned()
                                    + pick(&mut random, &PLAIN);
                                node = create_text_node(&text);
                                if random() < 0.5 {
                                    let t = (random() * 3.0) as usize;
                                    set_state(&node, &note(), format!("t{t}"))?;
                                }
                            }
                            node.set_format(pick(&mut random, &FORMATS));
                            if random() < 0.2 {
                                let m = (random() * 2.0) as usize;
                                let mark = create_mark_node(&[format!("m{m}")]);
                                mark.append(&[node])?;
                                paragraph.append(&[mark])?;
                            } else {
                                paragraph.append(&[node])?;
                            }
                        }
                        get_root().append(&[paragraph])?;
                    }
                    Ok(())
                },
                discrete(),
            )?;
            for edit in 0..4 {
                let before = read(&editor);
                let op = (random.borrow_mut())();
                let description = Rc::new(RefCell::new(String::new()));
                let (r, d) = (random.clone(), description.clone());
                editor.update(
                    move || {
                        let mut random = || (r.borrow_mut())();
                        let nodes = get_root().get_all_text_nodes();
                        if nodes.is_empty() {
                            return Ok(());
                        }
                        let index_of =
                            |n: &NodeRef| nodes.iter().position(|x| x.get_key() == n.get_key());
                        let node = pick(&mut random, &nodes);
                        let offset =
                            (random() * (node.get_text_content_size() + 1) as f64) as usize;
                        if op < 0.35 {
                            let typed = pick(&mut random, &TYPED);
                            *d.borrow_mut() =
                                format!("type {typed:?} at {:?}:{offset}", index_of(&node));
                            node.select(Some(offset), Some(offset))?
                                .insert_text(typed)?;
                        } else if op < 0.6 {
                            let backward = random() < 0.5;
                            *d.borrow_mut() = format!(
                                "{} at {:?}:{offset}",
                                if backward { "backspace" } else { "delete" },
                                index_of(&node)
                            );
                            node.select(Some(offset), Some(offset))?
                                .delete_character(backward)?;
                        } else if op < 0.8 {
                            // Join a block onto the one before it (backspace
                            // at its start)
                            let blocks: Vec<NodeRef> = get_root()
                                .get_children()
                                .into_iter()
                                .filter(NodeRef::is_element)
                                .collect();
                            let block = pick(&mut random, &blocks);
                            *d.borrow_mut() = format!(
                                "join block {:?}",
                                blocks.iter().position(|b| b.get_key() == block.get_key())
                            );
                            if let Some(first) =
                                block.get_first_descendant().filter(NodeRef::is_text)
                            {
                                first.select(Some(0), Some(0))?.delete_character(true)?;
                            }
                        } else {
                            // Delete a range across nodes
                            let other = pick(&mut random, &nodes);
                            let (a, b) = if index_of(&node) <= index_of(&other) {
                                (node.clone(), other)
                            } else {
                                (other, node.clone())
                            };
                            let selection = create_range_selection();
                            let from = offset.min(a.get_text_content_size());
                            let to = (random() * (b.get_text_content_size() + 1) as f64) as usize;
                            selection.point_set(
                                Which::Anchor,
                                a.get_key(),
                                from,
                                PointType::Text,
                            )?;
                            selection.point_set(
                                Which::Focus,
                                b.get_key(),
                                if a.get_key() == b.get_key() {
                                    from.max(to)
                                } else {
                                    to
                                },
                                PointType::Text,
                            )?;
                            *d.borrow_mut() =
                                format!("remove {:?}:{from}-{:?}:{to}", index_of(&a), index_of(&b));
                            set_selection(Some(selection.clone()));
                            selection.remove_text()?;
                        }
                        set_selection(None);
                        Ok(())
                    },
                    discrete(),
                )?;
                let after = read(&editor);
                let entities_before = before.iter().filter(|n| n.entity).count();
                let entities_after = after.iter().filter(|n| n.entity).count();
                if entities_after < entities_before {
                    demoted += 1;
                }
                check(
                    &format!("seed {seed} edit {edit} {}", description.borrow()),
                    &before,
                    &after,
                    &mut failures,
                );
            }
            Ok::<(), crate::LexicalError>(())
        }));
        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(e)) => failures.push(format!("seed {seed}: {e}")),
            Err(p) => failures.push(format!("seed {seed}: {}", panic_message(&*p))),
        }
        seed += 1;
    }
    assert_eq!(failures, Vec::<String>::new());
    // The property was exercised
    assert!(f64::from(demoted) > f64::from(seeds()) / 10.0, "{demoted}");
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeStateProperty.test.ts::registerLexicalTextEntity keeps NodeState (property) > hashtag: an edit that turns an entity into text keeps its NodeState and marks on its characters
#[test]
fn hashtag_an_edit_that_turns_an_entity_into_text_keeps_its_node_state_and_marks() {
    an_edit_that_turns_an_entity_into_text_keeps_its_node_state_and_marks(Kind::Hashtag);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeStateProperty.test.ts::registerLexicalTextEntity keeps NodeState (property) > mention: an edit that turns an entity into text keeps its NodeState and marks on its characters
#[test]
fn mention_an_edit_that_turns_an_entity_into_text_keeps_its_node_state_and_marks() {
    an_edit_that_turns_an_entity_into_text_keeps_its_node_state_and_marks(Kind::Mention);
}
