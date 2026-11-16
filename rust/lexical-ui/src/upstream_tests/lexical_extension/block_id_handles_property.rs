//! Ports of `packages/lexical-extension/src/__tests__/unit/BlockIdExtension.handles.property.test.ts`.
//!
//! Upstream draws its documents with fast-check, which the crate cannot
//! depend on. This port draws them from the same distributions with a
//! seeded generator (15 documents, as upstream's default `numRuns`, plus
//! upstream's explicit example), without shrinking. The typo-rate
//! measurement upstream skips unless `BLOCK_HANDLE_TYPO_DOCS` is set is not
//! a collected test, so it is not ported.

#![cfg(feature = "rich-text")]

use super::super::support::update;
use super::support::{as_edited, paragraph};
use crate::extension::block_handles::{HANDLE_KEY_LENGTH, handle_key};
use crate::extension::{
    BlockHandleOptions, BlockHandleResult, BlockIdConfigOverrides, GenerateId,
    MIN_BLOCK_HANDLE_LENGTH, block_id_extension, block_id_state, build_editor_from_extensions,
    get_block_by_id, get_block_handle, get_block_handles, get_block_id,
};
use crate::{
    Editor, InitialEditorStateType, LexicalExtension, config_extension, define_extension, get_root,
    set_state,
};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

const RUNS: u32 = 15;

const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// A seeded generator (mulberry32).
struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4294967296.0
    }
    /// An integer in `min..=max`.
    fn int(&mut self, min: usize, max: usize) -> usize {
        min + (self.next() * (max - min + 1) as f64) as usize
    }
    fn nat(&mut self) -> usize {
        self.int(0, 1 << 20)
    }
    fn id_string(&mut self, min: usize, max: usize) -> String {
        let len = self.int(min, max);
        (0..len)
            .map(|_| ALPHABET[self.int(0, 63)] as char)
            .collect()
    }
}

/// `IdSpec`.
#[derive(Clone, Debug)]
enum IdSpec {
    Random(String),
    Short(String),
    Shared {
        base: usize,
        prefix: usize,
        tail: String,
    },
}

/// `idSpec`: random (weight 3), short (1) or shared (4).
fn id_spec(rng: &mut Rng) -> IdSpec {
    match rng.int(0, 7) {
        0..=2 => IdSpec::Random(rng.id_string(21, 21)),
        3 => IdSpec::Short(rng.id_string(1, 3)),
        _ => IdSpec::Shared {
            base: rng.nat(),
            prefix: rng.int(1, 10),
            tail: rng.id_string(1, 17),
        },
    }
}

/// `resolveId(spec, earlier)`.
fn resolve_id(spec: &IdSpec, earlier: &[String]) -> String {
    match spec {
        IdSpec::Random(id) | IdSpec::Short(id) => id.clone(),
        IdSpec::Shared { tail, .. } if earlier.is_empty() => tail.clone(),
        IdSpec::Shared { base, prefix, tail } => {
            let base = &earlier[base % earlier.len()];
            // Ids are ASCII.
            format!("{}{tail}", &base[..(*prefix).min(base.len())])
        }
    }
}

/// `Edit`.
#[derive(Clone, Debug)]
enum Edit {
    Insert { at: usize, id: IdSpec },
    Delete { at: usize },
    Replace { at: usize },
    Move { at: usize, to: usize },
}

fn edit(rng: &mut Rng) -> Edit {
    match rng.int(0, 3) {
        0 => Edit::Insert {
            at: rng.nat(),
            id: id_spec(rng),
        },
        1 => Edit::Delete { at: rng.nat() },
        2 => Edit::Replace { at: rng.nat() },
        _ => Edit::Move {
            at: rng.nat(),
            to: rng.nat(),
        },
    }
}

/// `bruteForceHandle(id, ids)`.
fn brute_force_handle(id: &str, ids: &[String]) -> String {
    let key = handle_key(id);
    let others: Vec<String> = ids
        .iter()
        .filter(|o| *o != id)
        .map(|o| handle_key(o))
        .collect();
    let mut length = MIN_BLOCK_HANDLE_LENGTH;
    while length < key.len() && others.iter().any(|o| o.starts_with(&key[..length])) {
        length += 1;
    }
    key[..length].to_owned()
}

/// `oneCharTypos(s)`.
fn one_char_typos(s: &str) -> Vec<String> {
    let mut result = HashSet::new();
    for i in 0..s.len() {
        result.insert(format!("{}{}", &s[..i], &s[i + 1..]));
        for &c in ALPHABET {
            result.insert(format!("{}{}{}", &s[..i], c as char, &s[i + 1..]));
        }
    }
    for i in 0..=s.len() {
        for &c in ALPHABET {
            result.insert(format!("{}{}{}", &s[..i], c as char, &s[i..]));
        }
    }
    result.remove(s);
    result.into_iter().collect()
}

/// `check(editor, () => true, context)`: every invariant on the current
/// state, and the typo property on every block.
fn check(editor: &Editor, context: &str) {
    editor
        .read(|| {
            let blocks = get_root().get_children();
            let ids: Vec<String> = blocks
                .iter()
                .map(|node| get_block_id(node).unwrap().unwrap())
                .collect();
            assert_eq!(ids.iter().collect::<HashSet<_>>().len(), ids.len(), "{context}");
            let handles: HashMap<String, String> =
                get_block_handles().unwrap().into_iter().collect();
            assert_eq!(handles.len(), ids.len(), "{context}");
            let mut handle_of_id = HashMap::new();
            for (block, id) in blocks.iter().zip(&ids) {
                let handle = get_block_handle(block).unwrap().unwrap();
                assert_eq!(handle, brute_force_handle(id, &ids), "{context} handle of {id}");
                assert_eq!(handles.get(id), Some(&handle), "{context}");
                handle_of_id.insert(id.clone(), handle.clone());
                // A whole id is accepted when it is longer than any handle.
                let mut inputs = vec![handle.clone()];
                if id.len() > HANDLE_KEY_LENGTH {
                    inputs.push(id.clone());
                }
                for input in inputs {
                    let result = crate::extension::resolve_block_handle(&input, None).unwrap();
                    assert!(
                        matches!(&result, BlockHandleResult::Found { node } if node.get_key() == block.get_key()),
                        "{context}: {input} → {}",
                        result.status()
                    );
                }
            }
            let all_handles: HashSet<&String> = handle_of_id.values().collect();
            for (block, id) in blocks.iter().zip(&ids) {
                let own = &handle_of_id[id];
                for s in one_char_typos(own) {
                    match crate::extension::resolve_block_handle(&s, None).unwrap() {
                        BlockHandleResult::Found { node } if node.get_key() != block.get_key() => {
                            let other_id = get_block_id(&node).unwrap().unwrap();
                            let other_handle = &handle_of_id[&other_id];
                            assert!(
                                s == other_id
                                    || (handle_key(&other_id).starts_with(&s)
                                        && s.starts_with(other_handle.as_str())),
                                "{context}: typo {s} of {own} named {other_id} (handle {other_handle})"
                            );
                        }
                        BlockHandleResult::Ambiguous { candidates, .. } => {
                            for candidate in &candidates {
                                assert!(all_handles.contains(candidate), "{context}");
                            }
                        }
                        _ => {}
                    }
                }
            }
        })
        .unwrap();
}

/// `runDocument(specs, batches)`.
fn run_document(specs: &[IdSpec], batches: &[Vec<Edit>]) {
    let mut ids: Vec<String> = Vec::new();
    for spec in specs {
        let id = resolve_id(spec, &ids);
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    let n = Cell::new(0);
    let editor = as_edited(
        build_editor_from_extensions([define_extension(LexicalExtension {
            initial_editor_state: Some(InitialEditorStateType::Null),
            dependencies: vec![
                crate::rich_text::rich_text_extension().into(),
                // Only for blocks whose id is taken (edits bring their own
                // ids): longer than any generated id, so never one.
                config_extension(
                    &block_id_extension(),
                    [BlockIdConfigOverrides {
                        generate_id: Some(GenerateId(Rc::new(move || {
                            n.set(n.get() + 1);
                            format!("{}{}", "F".repeat(28), n.get())
                        }))),
                        ..Default::default()
                    }],
                ),
            ],
            ..LexicalExtension::<(), (), ()>::new("[block-handle-property]", ())
        })
        .into()])
        .unwrap(),
    );
    let initial = ids.clone();
    update(&editor, move || {
        for id in &initial {
            let block = paragraph(id);
            set_state(&block, &block_id_state(), Some(id.clone()))?;
            get_root().append(&[block])?;
        }
        Ok(())
    });
    let mut previous = editor.read(get_block_handles).unwrap().unwrap();
    check(&editor, &format!("initial {ids:?}"));
    for (step, batch) in batches.iter().enumerate() {
        // Each batch of edits is one update.
        let ops = batch.clone();
        update(&editor, move || {
            for op in &ops {
                let blocks = get_root().get_children();
                let index = match op {
                    Edit::Insert { at, .. }
                    | Edit::Delete { at }
                    | Edit::Replace { at }
                    | Edit::Move { at, .. } => *at,
                };
                let at = &blocks[index % blocks.len()];
                if matches!(op, Edit::Insert { .. }) || blocks.len() < 3 {
                    // Blocks inserted earlier in this update have no id yet.
                    let current: Vec<String> = blocks
                        .iter()
                        .filter_map(|block| get_block_id(block).unwrap())
                        .collect();
                    let block = paragraph(&format!("t{step}"));
                    if let Edit::Insert { id, .. } = op {
                        set_state(&block, &block_id_state(), Some(resolve_id(id, &current)))?;
                    }
                    at.insert_after(&block, true)?;
                } else {
                    match op {
                        Edit::Delete { .. } => at.remove(false)?,
                        Edit::Move { to, .. } => {
                            let target = &blocks[to % blocks.len()];
                            if target.get_key() != at.get_key() {
                                target.insert_after(at, true)?;
                            }
                        }
                        _ => {
                            at.replace(&paragraph(&format!("r{step}")), false)?;
                        }
                    }
                }
            }
            Ok(())
        });
        // The handles of the previous read name the blocks they named
        // then, while those exist.
        let options = BlockHandleOptions {
            read: Some(previous.clone()),
        };
        editor
            .read(|| {
                for (id, handle) in &previous {
                    let result =
                        crate::extension::resolve_block_handle(handle, Some(&options)).unwrap();
                    match get_block_by_id(id).unwrap() {
                        None => assert_eq!(result.status(), "not-found", "step {step} {handle}"),
                        Some(node) => assert!(
                            matches!(&result, BlockHandleResult::Found { node: found } if found.get_key() == node.get_key()),
                            "step {step} {handle} → {}",
                            result.status()
                        ),
                    }
                }
            })
            .unwrap();
        previous = editor.read(get_block_handles).unwrap().unwrap();
        check(&editor, &format!("step {step} ({batch:?})"));
    }
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handles.property.test.ts::block handles: property > handles, resolution and typo safety under random ids and edits
#[test]
fn handles_resolution_and_typo_safety_under_random_ids_and_edits() {
    // Upstream's example: an insert that repeats an id (its copy gets a
    // fallback id), with every short counter-like id already taken.
    run_document(
        &(1..=32)
            .map(|i| IdSpec::Random(format!("zz{i}")))
            .collect::<Vec<_>>(),
        &[vec![Edit::Insert {
            at: 0,
            id: IdSpec::Short("zz1".to_owned()),
        }]],
    );
    for run in 0..RUNS {
        let mut rng = Rng(0x5eed_0000 + run);
        let specs: Vec<IdSpec> = (0..rng.int(3, 40)).map(|_| id_spec(&mut rng)).collect();
        let batches: Vec<Vec<Edit>> = (0..rng.int(0, 10))
            .map(|_| (0..rng.int(1, 3)).map(|_| edit(&mut rng)).collect())
            .collect();
        run_document(&specs, &batches);
    }
}
