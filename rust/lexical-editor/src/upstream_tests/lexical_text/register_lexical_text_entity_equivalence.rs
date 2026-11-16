//! Ports of `packages/lexical-text/src/__tests__/unit/registerLexicalTextEntityEquivalence.test.ts`,
//! less its recording cases (text-edit records belong to `@lexical/y`) and
//! its strict setter case (rust/parity/skips.json).
//!
//! registerLexicalTextEntity records what its transforms do with the
//! characters they touch, around the calls it made before. This compares it
//! with a copy of the helper as it was before that change, in two editors
//! with a root element driven by the same random edits next to two kinds of
//! entities: after every edit, the same JSON, the same selection, the same
//! mutation-listener events and the same node keys.

use super::support::{discrete, entity_class, regex_match};
use crate::markdown::RegExp;
use crate::text::{EntityMatch, register_lexical_text_entity};
use crate::upstream_tests::support::set_test_root_element;
use crate::{
    CreateEditorArgs, Editor, LexicalError, MutationListenerOptions, NodeClass, NodeRef, PointType,
    Unregister, UpdateOptions, Which, create, create_editor, create_node_selection,
    create_paragraph_node, create_range_selection, create_text_node, get_root, get_selection, json,
    reset_random_key, set_selection, utf16,
};
use std::cell::RefCell;
use std::rc::Rc;

type GetMatch = Rc<dyn Fn(&str) -> Option<EntityMatch>>;

/// `registerLexicalTextEntity` as it was before it recorded its edits.
fn register_lexical_text_entity_before(
    editor: &Editor,
    get_match: GetMatch,
    target_type: &'static str,
    create_node: Rc<dyn Fn(&NodeRef) -> NodeRef>,
) -> Vec<Unregister> {
    let is_target_node = move |node: &NodeRef| node.get_type() == target_type;

    fn replace_with_simple_text(node: &NodeRef) -> Result<(), LexicalError> {
        let data = node.text_data();
        let text_node = create_text_node(&node.get_text_content());
        text_node.set_format(data.get_format());
        text_node.set_style(data.get_style());
        text_node.set_detail(data.get_detail())?;
        node.replace(&text_node, false)?;
        Ok(())
    }

    let get_mode = |node: &NodeRef| node.text_data().mode;

    let (gm, cn) = (get_match.clone(), create_node.clone());
    let text_node_transform = move |node: &NodeRef| -> Result<(), LexicalError> {
        if !node.is_simple_text() {
            return Ok(());
        }
        let mut prev_sibling = node.get_previous_sibling();
        let mut text = node.get_text_content();
        let mut current_node = node.clone();

        if let Some(prev) = prev_sibling.clone().filter(NodeRef::is_text) {
            let previous_text = prev.get_text_content();
            let combined_text = previous_text.clone() + &text;
            let prev_match = gm(&combined_text);
            let previous_len = utf16::len(&previous_text);
            if is_target_node(&prev) {
                match prev_match {
                    Some(prev_match) if get_mode(&prev) == 0 => {
                        let diff = prev_match.end as i64 - previous_len as i64;
                        if diff > 0 {
                            let diff = diff as usize;
                            let concat_text = utf16::slice(&text, 0, diff);
                            prev.select(None, None)?;
                            prev.set_text_content(&(previous_text + &concat_text));
                            if diff == utf16::len(&text) {
                                node.remove(false)?;
                            } else {
                                node.set_text_content(&utf16::slice_from(&text, diff));
                            }
                            return Ok(());
                        }
                    }
                    _ => {
                        replace_with_simple_text(&prev)?;
                        return Ok(());
                    }
                }
            } else if prev_match.is_none_or(|m| m.start < previous_len) {
                return Ok(());
            }
        }

        let mut prev_match_length_to_skip = 0;
        loop {
            let remaining_text = text.clone();
            let m = gm(&remaining_text);
            let next_text = match m {
                None => String::new(),
                Some(m) => utf16::slice_from(&remaining_text, m.end),
            };
            text = next_text.clone();

            if next_text.is_empty() {
                let next_sibling = current_node.get_next_sibling();
                if let Some(next) = next_sibling.filter(NodeRef::is_text) {
                    let combined_text = remaining_text.clone() + &next.get_text_content();
                    let next_match = gm(&combined_text);
                    match next_match {
                        None => {
                            if is_target_node(&next) {
                                replace_with_simple_text(&next)?;
                            } else {
                                next.mark_dirty();
                            }
                            return Ok(());
                        }
                        Some(next_match) => {
                            if m.is_none_or(|m| next_match.start != m.start) {
                                return Ok(());
                            }
                        }
                    }
                }
            }

            let Some(m) = m else {
                return Ok(());
            };

            if m.start == 0
                && prev_sibling
                    .as_ref()
                    .is_some_and(|p| p.is_text() && p.is_text_entity())
            {
                prev_match_length_to_skip += m.end;
                continue;
            }

            let (node_to_replace, next_current) = if m.start == 0 {
                let parts = current_node.split_text(&[m.end as i64])?;
                (parts.first().cloned(), parts.get(1).cloned())
            } else {
                let parts = current_node.split_text(&[
                    (m.start + prev_match_length_to_skip) as i64,
                    (m.end + prev_match_length_to_skip) as i64,
                ])?;
                (parts.get(1).cloned(), parts.get(2).cloned())
            };
            let node_to_replace = node_to_replace
                .ok_or_else(|| LexicalError::new("nodeToReplace should not be undefined"))?;

            let replacement_node = cn(&node_to_replace);
            let data = node_to_replace.text_data();
            replacement_node.set_format(data.get_format());
            replacement_node.set_style(data.get_style());
            replacement_node.set_detail(data.get_detail())?;
            node_to_replace.replace(&replacement_node, false)?;

            let Some(next_current) = next_current else {
                return Ok(());
            };
            current_node = next_current;
            prev_match_length_to_skip = 0;
            prev_sibling = Some(replacement_node);
        }
    };

    let reverse_node_transform = move |node: &NodeRef| -> Result<(), LexicalError> {
        let text = node.get_text_content();
        let m = get_match(&text);
        let Some(m) = m.filter(|m| m.start == 0) else {
            replace_with_simple_text(node)?;
            return Ok(());
        };
        if utf16::len(&text) > m.end {
            node.split_text(&[m.end as i64])?;
            return Ok(());
        }
        let prev_sibling = node.get_previous_sibling();
        if let Some(prev) = prev_sibling.filter(|p| p.is_text() && p.is_text_entity()) {
            replace_with_simple_text(&prev)?;
            replace_with_simple_text(node)?;
        }
        let next_sibling = node.get_next_sibling();
        if let Some(next) = next_sibling.filter(|n| n.is_text() && n.is_text_entity()) {
            replace_with_simple_text(&next)?;
            if is_target_node(node) {
                replace_with_simple_text(node)?;
            }
        }
        Ok(())
    };

    vec![
        editor.register_node_transform("text", text_node_transform),
        editor.register_node_transform(target_type, reverse_node_transform),
    ]
}

#[derive(Clone, Copy, PartialEq)]
enum Register {
    Before,
    After,
}

fn register(
    which: Register,
    editor: &Editor,
    get_match: GetMatch,
    target_type: &'static str,
    create_node: Rc<dyn Fn(&NodeRef) -> NodeRef>,
) {
    let _ = match which {
        Register::Before => {
            register_lexical_text_entity_before(editor, get_match, target_type, create_node)
        }
        Register::After => register_lexical_text_entity(
            editor,
            move |text| get_match(text),
            target_type,
            move |node| create_node(node),
        ),
    };
}

// A hashtag-like entity whose setter refuses to clear it: a Rust node has no
// `setTextContent` to override (see the skip of the strict setter case).
entity_class!(TagNode, "equivalence-tag");
// A plain entity (the review's repros).
entity_class!(EntityNode, "equivalence-entity");
// A keyword entity that text is not typed into from either end.
entity_class!(KeywordNode, "equivalence-keyword", false, false);

fn tag_match() -> GetMatch {
    Rc::new(regex_match("#[a-z0-9]+", ""))
}

fn keyword_match() -> GetMatch {
    let re = RegExp::new(r"(^|\s)(la)(?=\s|$)", "").unwrap();
    Rc::new(move |text| {
        let m = re.exec(text)?;
        let start = m.index + utf16::len(m.str(1));
        Some(EntityMatch {
            start,
            end: start + 2,
        })
    })
}

fn creator<T: crate::LexicalNode + Default>() -> Rc<dyn Fn(&NodeRef) -> NodeRef> {
    Rc::new(|node: &NodeRef| {
        let entity = create::<T>();
        entity.set_text_content(&node.get_text_content());
        entity
    })
}

type Events = Rc<RefCell<Vec<(String, Vec<(String, &'static str)>)>>>;

fn make_editor(which: Register) -> (Editor, Events) {
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![NodeClass::of::<TagNode>(), NodeClass::of::<KeywordNode>()],
        ..CreateEditorArgs::default()
    });
    register(
        which,
        &editor,
        tag_match(),
        "equivalence-tag",
        creator::<TagNode>(),
    );
    register(
        which,
        &editor,
        keyword_match(),
        "equivalence-keyword",
        creator::<KeywordNode>(),
    );
    let events: Events = Rc::default();
    for klass in ["text", "equivalence-tag", "equivalence-keyword"] {
        let sink = events.clone();
        let _ = editor
            .register_mutation_listener(
                klass,
                move |mutations, _| {
                    let mut entries: Vec<(String, &'static str)> = mutations
                        .iter()
                        .map(|(k, m)| (k.to_string(), m.as_str()))
                        .collect();
                    entries.sort();
                    sink.borrow_mut().push((klass.to_owned(), entries));
                },
                MutationListenerOptions {
                    skip_initialization: true,
                },
            )
            .unwrap();
    }
    set_test_root_element(&editor);
    (editor, events)
}

/// xorshift32.
fn rng(seed: u32) -> impl FnMut() -> f64 {
    let mut s = if seed == 0 { 1 } else { seed };
    move || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        f64::from(s) / 4_294_967_296.0
    }
}

fn texts() -> Result<Vec<NodeRef>, LexicalError> {
    let line = get_root().get_first_child_or_throw()?;
    Ok(if line.is_element() {
        line.get_children()
            .into_iter()
            .filter(NodeRef::is_text)
            .collect()
    } else {
        Vec::new()
    })
}

fn point_at(at: usize, left: bool) -> Result<Option<(NodeRef, usize)>, LexicalError> {
    let mut p = 0;
    let mut found = None;
    for t in texts()? {
        let n = t.get_text_content_size();
        if at >= p && at <= p + n {
            found = Some((t.clone(), at - p));
            if left || at < p + n {
                return Ok(found);
            }
        }
        p += n;
    }
    Ok(found)
}

/// The selection, as JSON.
fn selection_json() -> json::Value {
    let s = json::Value::String;
    let n = |x: usize| json::Value::Number(x as f64);
    match get_selection() {
        Some(sel) if sel.is_range() => {
            let r = sel.range().unwrap();
            json::Value::Array(vec![
                s(r.anchor.key.to_string()),
                n(r.anchor.offset),
                s(r.anchor.point_type.as_str().to_owned()),
                s(r.focus.key.to_string()),
                n(r.focus.offset),
                s(r.focus.point_type.as_str().to_owned()),
                json::Value::Number(r.format),
                s(r.style.clone()),
            ])
        }
        Some(sel) if sel.is_node() => json::Value::Array(
            sel.get_nodes()
                .unwrap()
                .iter()
                .map(|n| s(n.get_key().to_string()))
                .collect(),
        ),
        _ => json::Value::Null,
    }
}

const PIECES: [&str; 10] = ["#ab", " ", "la", "la ", "cd", "#", "x y", "ab", " #c", "a "];
const TYPED: [&str; 11] = ["a", " ", "#", "la", "b ", "#ab", "x", "l", "a ", "1", "cd"];

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Type,
    Backspace,
    Delete,
    Over,
    Set,
    Remove,
}

const KINDS: [Kind; 7] = [
    Kind::Type,
    Kind::Type,
    Kind::Backspace,
    Kind::Delete,
    Kind::Over,
    Kind::Set,
    Kind::Remove,
];

#[derive(Clone, Copy, Debug)]
struct Op {
    at: usize,
    /// At the end of text node `node` instead of at `at`.
    edge: bool,
    kind: Kind,
    left: bool,
    len: usize,
    node: usize,
    s: &'static str,
}

/// One edit, as a caret edit (or app code setting or removing a node).
fn apply(op: Op) -> Result<(), LexicalError> {
    let text_len = utf16::len(&get_root().get_text_content());
    if op.kind == Kind::Set || op.kind == Kind::Remove {
        let texts = texts()?;
        let Some(node) = texts.get(op.node % texts.len().max(1)) else {
            return Ok(());
        };
        if op.kind == Kind::Remove {
            node.remove(false)?;
        } else {
            let t = node.get_text_content();
            let at = op.at % (utf16::len(&t) + 1);
            let next = utf16::slice(&t, 0, at) + op.s + &utf16::slice_from(&t, at + op.len);
            if !next.is_empty() {
                node.set_text_content(&next);
            }
        }
        return Ok(());
    }
    let mut at = op.at as i64;
    if op.edge {
        let texts = texts()?;
        // With no text node, JS's `op.node % 0` is NaN, and `slice(0, NaN)`
        // is empty.
        let end = op.node.checked_rem(texts.len()).map_or(0, |i| i + 1);
        at = texts[..end]
            .iter()
            .map(|t| t.get_text_content_size() as i64)
            .sum();
    }
    let (mut from, mut to) = (at, at);
    match op.kind {
        Kind::Backspace => from = at - 1,
        Kind::Delete => to = at + 1,
        Kind::Over => to = (text_len as i64).min(at + op.len as i64),
        _ => {}
    }
    if from < 0 || to > text_len as i64 {
        return Ok(());
    }
    let a = point_at(from as usize, if from == to { op.left } else { true })?;
    let b = if from == to {
        a.clone()
    } else {
        point_at(to as usize, false)?
    };
    let (Some(a), Some(b)) = (a, b) else {
        return Ok(());
    };
    let sel = create_range_selection();
    sel.point_set(Which::Anchor, a.0.get_key(), a.1, PointType::Text)?;
    sel.point_set(Which::Focus, b.0.get_key(), b.1, PointType::Text)?;
    set_selection(Some(sel.clone()));
    if op.kind == Kind::Type || op.kind == Kind::Over {
        sel.insert_text(op.s)?;
    } else if from != to {
        sel.remove_text()?;
    }
    Ok(())
}

#[derive(Debug, PartialEq)]
struct Step {
    events: Vec<(String, Vec<(String, &'static str)>)>,
    json: json::Value,
    result: String,
    selection: json::Value,
}

/// A session: its initial pieces, then its ops, each step after an op.
fn session(which: Register, pieces: &[&'static str], ops: &[Op]) -> Vec<Step> {
    reset_random_key();
    let (editor, events) = make_editor(which);
    let pieces = pieces.to_vec();
    editor
        .update(
            move || {
                let paragraph = create_paragraph_node();
                paragraph.append(
                    &pieces
                        .iter()
                        .map(|s| create_text_node(s))
                        .collect::<Vec<_>>(),
                )?;
                get_root().append(&[paragraph])
            },
            discrete(),
        )
        .unwrap();
    let mut steps = Vec::new();
    for &op in ops {
        events.borrow_mut().clear();
        let result = match editor.update(move || apply(op), discrete()) {
            Ok(()) => "ok".to_owned(),
            Err(e) => e.to_string(),
        };
        steps.push(Step {
            events: events.borrow().clone(),
            json: editor.get_editor_state().to_json(false),
            result,
            selection: editor.read(selection_json).unwrap(),
        });
    }
    editor.set_root_element(None).unwrap();
    steps
}

/// `process.env.TEXT_ENTITY_EQUIVALENCE_SESSIONS || 300`.
fn sessions() -> u32 {
    std::env::var("TEXT_ENTITY_EQUIVALENCE_SESSIONS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n != 0)
        .unwrap_or(300)
}

// The recording case is skipped (text-edit records are @lexical/y's); without
// recording upstream also checks that no record is made, which Rust can't make.
// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityEquivalence.test.ts::registerLexicalTextEntity against the helper before it recorded its edits > 300 random sessions of 8 edits next to entities, not recording text edits: the same JSON, selection, mutations and keys
#[test]
fn random_sessions_of_8_edits_next_to_entities_not_recording() {
    for seed in 1..=sessions() {
        let mut r = rng(seed.wrapping_mul(7919).wrapping_add(13));
        let pieces: Vec<&'static str> = (0..2 + (r() * 4.0) as usize)
            .map(|_| PIECES[(r() * PIECES.len() as f64) as usize])
            .collect();
        let ops: Vec<Op> = (0..8)
            .map(|_| Op {
                at: (r() * 20.0) as usize,
                edge: r() < 0.5,
                kind: KINDS[(r() * KINDS.len() as f64) as usize],
                left: r() < 0.5,
                len: (r() * 3.0) as usize,
                node: (r() * 6.0) as usize,
                s: TYPED[(r() * TYPED.len() as f64) as usize],
            })
            .collect();
        let before = session(Register::Before, &pieces, &ops);
        let after = session(Register::After, &pieces, &ops);
        for (k, (a, b)) in before.iter().zip(&after).enumerate() {
            assert_eq!(b, a, "seed {seed}, edit {k} {:?}", ops[k]);
        }
    }
}

// The edge cases where a change of the calls would differ: merging into a
// fresh node instead of replace() would leave a stale caret inside the
// entity unclamped and turn a NodeSelection into a range: the helper makes
// the same calls.

#[derive(Clone, Copy, PartialEq)]
enum Case {
    Range,
    NodeSelection,
    Shortened,
}

#[derive(Debug, PartialEq)]
struct Outcome {
    errors: Vec<String>,
    json: json::Value,
    selection: json::Value,
}

fn run(which: Register, kind: Case) -> Outcome {
    reset_random_key();
    let mut errors = Vec::new();
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![NodeClass::of::<TagNode>(), NodeClass::of::<EntityNode>()],
        ..CreateEditorArgs::default()
    });
    register(
        which,
        &editor,
        tag_match(),
        "equivalence-entity",
        creator::<EntityNode>(),
    );
    set_test_root_element(&editor);
    let mut push = |r: Result<(), LexicalError>| {
        if let Err(e) = r {
            errors.push(e.to_string());
        }
    };
    if kind == Case::Shortened {
        push(editor.update(
            || {
                let node = create::<EntityNode>();
                node.set_text_content("#abc");
                let paragraph = create_paragraph_node();
                paragraph.append(std::slice::from_ref(&node))?;
                get_root().append(&[paragraph])?;
                node.select(Some(4), Some(4))?;
                Ok(())
            },
            discrete(),
        ));
        push(editor.update(
            || {
                get_root()
                    .get_first_descendant()
                    .unwrap()
                    .set_text_content("xx");
                Ok(())
            },
            discrete(),
        ));
    } else {
        push(editor.update(
            move || {
                let text = create_text_node("#abc");
                let paragraph = create_paragraph_node();
                paragraph.append(std::slice::from_ref(&text))?;
                get_root().append(&[paragraph])?;
                if kind == Case::NodeSelection {
                    let sel = create_node_selection();
                    sel.add(text.get_key());
                    set_selection(Some(sel));
                } else {
                    let sel = text.select(Some(2), Some(3))?;
                    sel.set_format(1.0);
                    sel.set_style("color: red");
                }
                Ok(())
            },
            UpdateOptions {
                discrete: true,
                skip_transforms: true,
                ..UpdateOptions::default()
            },
        ));
        push(editor.update(
            || {
                get_root().get_first_descendant().unwrap().mark_dirty();
                Ok(())
            },
            discrete(),
        ));
    }
    let outcome = Outcome {
        errors,
        json: editor.get_editor_state().to_json(false),
        selection: editor.read(selection_json).unwrap(),
    };
    editor.set_root_element(None).unwrap();
    outcome
}

fn check_case(kind: Case) {
    let before = run(Register::Before, kind);
    assert_eq!(run(Register::After, kind), before);
    assert_eq!(before.errors, Vec::<String>::new());
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityEquivalence.test.ts::the edge cases where a change of the calls would differ > range, not recording
#[test]
fn range_not_recording() {
    check_case(Case::Range);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityEquivalence.test.ts::the edge cases where a change of the calls would differ > node selection, not recording
#[test]
fn node_selection_not_recording() {
    check_case(Case::NodeSelection);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityEquivalence.test.ts::the edge cases where a change of the calls would differ > shortened, not recording
#[test]
fn shortened_not_recording() {
    check_case(Case::Shortened);
}
