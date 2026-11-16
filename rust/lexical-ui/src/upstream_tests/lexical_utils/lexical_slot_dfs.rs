//! Ports of
//! `packages/lexical-utils/src/__tests__/unit/LexicalSlotDfs.test.ts`.

#![cfg(feature = "extension")]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::upstream_tests::support::{
    TestBlockableDecoratorNode, TestShadowRootNode, create_test_decorator_node_inline,
    create_test_shadow_root_node,
};
use crate::utils::{DFSNode, dfs_with_slots, reverse_dfs_with_slots};
use crate::{
    InitialEditorStateType, LexicalExtension, NodeClass, NodeKey, NodeRef, create_paragraph_node,
    create_text_node, define_extension, get_root, get_slot, get_slot_names, set_slot,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

type Keys = Rc<RefCell<HashMap<&'static str, NodeKey>>>;

fn pairs(nodes: Vec<DFSNode>) -> Vec<(usize, NodeKey)> {
    nodes
        .into_iter()
        .map(|DFSNode { depth, node }| (depth, node.get_key().to_owned()))
        .collect()
}

fn expected(keys: &Keys, pairs: &[(usize, &str)]) -> Vec<(usize, NodeKey)> {
    let keys = keys.borrow();
    pairs
        .iter()
        .map(|(depth, name)| (*depth, keys[name].clone()))
        .collect()
}

/// `$slotAwareDfs(depth, node)`: slots first, then children.
fn slot_aware_dfs(depth: usize, node: &NodeRef, out: &mut Vec<(usize, NodeKey)>) {
    out.push((depth, node.get_key().to_owned()));
    if node.is_element() {
        for name in get_slot_names(node) {
            if let Some(slot) = get_slot(node, &name) {
                slot_aware_dfs(depth + 1, &slot, out);
            }
        }
        for child in node.get_children() {
            slot_aware_dfs(depth + 1, &child, out);
        }
    }
}

/// `$reverseSlotAwareDfs(depth, node)`: children in reverse, then slots in
/// reverse.
fn reverse_slot_aware_dfs(depth: usize, node: &NodeRef, out: &mut Vec<(usize, NodeKey)>) {
    out.push((depth, node.get_key().to_owned()));
    if node.is_element() {
        for child in node.get_children().iter().rev() {
            reverse_slot_aware_dfs(depth + 1, child, out);
        }
        for name in get_slot_names(node).iter().rev() {
            if let Some(slot) = get_slot(node, name) {
                reverse_slot_aware_dfs(depth + 1, &slot, out);
            }
        }
    }
}

/// `buildEditorFromExtensions(slotTreeExtension)`: host -> (slot title:
/// shadow root -> para -> "Title") + (child body -> "Body").
fn slot_tree_editor() -> (LexicalEditorWithDispose, Keys) {
    let keys: Keys = Rc::default();
    let out = keys.clone();
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| {
            let host = create_paragraph_node();
            let title = create_test_shadow_root_node();
            let title_para = create_paragraph_node();
            let title_text = create_text_node("Title");
            title_para.append(std::slice::from_ref(&title_text))?;
            title.append(std::slice::from_ref(&title_para))?;
            let body = create_paragraph_node();
            let body_text = create_text_node("Body");
            body.append(std::slice::from_ref(&body_text))?;
            get_root().append(std::slice::from_ref(&host))?;
            host.append(std::slice::from_ref(&body))?;
            set_slot(&host, "title", &title)?;
            let mut keys = out.borrow_mut();
            for (name, node) in [
                ("host", &host),
                ("title", &title),
                ("titlePara", &title_para),
                ("titleText", &title_text),
                ("body", &body),
                ("bodyText", &body_text),
            ] {
                keys.insert(name, node.get_key().to_owned());
            }
            Ok(())
        }))),
        nodes: vec![NodeClass::of::<TestShadowRootNode>()],
        ..LexicalExtension::<(), (), ()>::new("[slot-dfs]", ())
    })
    .into()])
    .unwrap();
    (editor, keys)
}

// upstream: lexical-utils/src/__tests__/unit/LexicalSlotDfs.test.ts::named-slots: $dfs traversal > $dfsWithSlots visits slot subtrees, slots-first, ahead of children
#[test]
fn dfs_with_slots_visits_slot_subtrees_slots_first() {
    let (editor, keys) = slot_tree_editor();
    editor
        .read(|| {
            let host = get_root().get_first_child().unwrap();
            // depth is absolute from root: host(1) -> title slot subtree
            // (shadow root 2 -> para 3 -> text 4) -> body child subtree(2,3)
            assert_eq!(
                pairs(dfs_with_slots(Some(&host), None)),
                expected(
                    &keys,
                    &[
                        (1, "host"),
                        (2, "title"),
                        (3, "titlePara"),
                        (4, "titleText"),
                        (2, "body"),
                        (3, "bodyText"),
                    ]
                )
            );
        })
        .unwrap();
}

// upstream: lexical-utils/src/__tests__/unit/LexicalSlotDfs.test.ts::named-slots: $dfs traversal > $dfsWithSlots slot traversal matches the reference slots-first walk
#[test]
fn dfs_with_slots_matches_the_reference_slots_first_walk() {
    let (editor, _) = slot_tree_editor();
    editor
        .read(|| {
            let host = get_root().get_first_child().unwrap();
            let mut reference = Vec::new();
            slot_aware_dfs(1, &host, &mut reference);
            assert_eq!(pairs(dfs_with_slots(Some(&host), None)), reference);
        })
        .unwrap();
}

// upstream: lexical-utils/src/__tests__/unit/LexicalSlotDfs.test.ts::named-slots: $dfs traversal > $reverseDfsWithSlots visits slots-last, mirroring $dfsWithSlots (#7112 invariant)
#[test]
fn reverse_dfs_with_slots_visits_slots_last() {
    let (editor, keys) = slot_tree_editor();
    editor
        .read(|| {
            let host = get_root().get_first_child().unwrap();
            let visited = pairs(reverse_dfs_with_slots(Some(&host), None));
            assert_eq!(
                visited,
                expected(
                    &keys,
                    &[
                        (1, "host"),
                        (2, "body"),
                        (3, "bodyText"),
                        (2, "title"),
                        (3, "titlePara"),
                        (4, "titleText"),
                    ]
                )
            );
            let mut reference = Vec::new();
            reverse_slot_aware_dfs(1, &host, &mut reference);
            assert_eq!(visited, reference);
        })
        .unwrap();
}

// upstream: lexical-utils/src/__tests__/unit/LexicalSlotDfs.test.ts::named-slots: $dfs traversal > endNode slot subtrees are not emitted past the inclusive stop
#[test]
fn end_node_slot_subtrees_are_not_emitted_past_the_inclusive_stop() {
    let (editor, keys) = slot_tree_editor();
    editor
        .read(|| {
            let root = get_root();
            let host = root.get_first_child().unwrap();
            let truncated = vec![
                (0, root.get_key().to_owned()),
                (1, keys.borrow()["host"].clone()),
            ];
            assert_eq!(pairs(dfs_with_slots(Some(&root), Some(&host))), truncated);
            assert_eq!(
                pairs(reverse_dfs_with_slots(Some(&root), Some(&host))),
                truncated
            );
        })
        .unwrap();
}

// upstream: lexical-utils/src/__tests__/unit/LexicalSlotDfs.test.ts::named-slots: $dfs traversal > reconciler text cache includes slot text (RootNode.__cachedText)
#[test]
fn reconciler_text_cache_includes_slot_text() {
    let (editor, _) = slot_tree_editor();
    editor
        .read(|| {
            let root = get_root();
            let host = root.get_first_child().unwrap();
            // the uncached element walk is slot-aware (slots-first)
            assert_eq!(host.get_text_content(), "TitleBody");
            assert_eq!(root.get_text_content(), "TitleBody");
        })
        .unwrap();
}

/// `buildEditorFromExtensions(decoratorSlotExtension)`: decorator host ->
/// (slot media: shadow root -> para -> "Eq").
fn decorator_slot_editor() -> (LexicalEditorWithDispose, Keys) {
    let keys: Keys = Rc::default();
    let out = keys.clone();
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| {
            let host = create_test_decorator_node_inline(false);
            let media = create_test_shadow_root_node();
            let media_para = create_paragraph_node();
            let media_text = create_text_node("Eq");
            media_para.append(std::slice::from_ref(&media_text))?;
            media.append(std::slice::from_ref(&media_para))?;
            get_root().append(std::slice::from_ref(&host))?;
            set_slot(&host, "media", &media)?;
            let mut keys = out.borrow_mut();
            for (name, node) in [
                ("host", &host),
                ("media", &media),
                ("mediaPara", &media_para),
                ("mediaText", &media_text),
            ] {
                keys.insert(name, node.get_key().to_owned());
            }
            Ok(())
        }))),
        nodes: vec![
            NodeClass::of::<TestShadowRootNode>(),
            NodeClass::of::<TestBlockableDecoratorNode>(),
        ],
        ..LexicalExtension::<(), (), ()>::new("[slot-dfs-decorator]", ())
    })
    .into()])
    .unwrap();
    (editor, keys)
}

const MEDIA: [(usize, &str); 4] = [
    (1, "host"),
    (2, "media"),
    (3, "mediaPara"),
    (4, "mediaText"),
];

// upstream: lexical-utils/src/__tests__/unit/LexicalSlotDfs.test.ts::named-slots: $dfs traversal into a decorator host > $dfsWithSlots descends into a decorator host's slots
#[test]
fn dfs_with_slots_descends_into_a_decorator_hosts_slots() {
    let (editor, keys) = decorator_slot_editor();
    editor
        .read(|| {
            let host = get_root().get_first_child().unwrap();
            assert_eq!(
                pairs(dfs_with_slots(Some(&host), None)),
                expected(&keys, &MEDIA)
            );
        })
        .unwrap();
}

// upstream: lexical-utils/src/__tests__/unit/LexicalSlotDfs.test.ts::named-slots: $dfs traversal into a decorator host > $reverseDfsWithSlots descends into a decorator host's slots
#[test]
fn reverse_dfs_with_slots_descends_into_a_decorator_hosts_slots() {
    let (editor, keys) = decorator_slot_editor();
    editor
        .read(|| {
            let host = get_root().get_first_child().unwrap();
            assert_eq!(
                pairs(reverse_dfs_with_slots(Some(&host), None)),
                expected(&keys, &MEDIA)
            );
        })
        .unwrap();
}
