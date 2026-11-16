//! Ports of `packages/lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts`.
//!
//! Without `DEFER_COMPOSITION_MERGE_TAG`, an IME composition does not change
//! how text nodes are normalized: a text node that becomes adjacent to the
//! composed node, with the same format, is merged with it in the same
//! update, exactly as when nothing is being composed. Every scenario runs
//! twice, once with the composition key on the composed node and once
//! without, and the two runs must produce identical editor states (JSON)
//! and node identities.

use crate::upstream_tests::support::update;
use crate::{
    COLLABORATION_TAG, COMPOSITION_END_TAG, COMPOSITION_START_TAG, CreateEditorArgs, Editor,
    HISTORIC_TAG, HISTORY_MERGE_TAG, LexicalError, NodeRef, PASTE_TAG, SKIP_COLLAB_TAG,
    UpdateOptions, create_editor, create_paragraph_node, create_text_node, get_root,
    set_composition_key,
};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Copy)]
enum Op {
    InsertBefore,
    InsertAfter,
    InsertOnBothSides,
    /// The bold node between plain text and the composed node goes away.
    RemoveTheBoldNodeBefore,
    RemoveTheBoldNodeAfter,
    UnboldTheNodeBefore,
    /// A line join from another source: the next line's text moves next to
    /// the composition.
    JoinTheNextLine,
}

impl Op {
    fn name(self) -> &'static str {
        match self {
            Op::InsertBefore => "insert before",
            Op::InsertAfter => "insert after",
            Op::InsertOnBothSides => "insert on both sides",
            Op::RemoveTheBoldNodeBefore => "remove the bold node before",
            Op::RemoveTheBoldNodeAfter => "remove the bold node after",
            Op::UnboldTheNodeBefore => "unbold the node before",
            Op::JoinTheNextLine => "join the next line",
        }
    }

    /// `op.$run(composed)`.
    fn run(self, composed: &NodeRef) -> Result<(), LexicalError> {
        match self {
            Op::InsertBefore => {
                composed.insert_before(&create_text_node("j1z"), true)?;
            }
            Op::InsertAfter => {
                composed.insert_after(&create_text_node("j3z"), true)?;
            }
            Op::InsertOnBothSides => {
                composed.insert_before(&create_text_node("j1z"), true)?;
                composed.insert_after(&create_text_node("j3z"), true)?;
            }
            Op::RemoveTheBoldNodeBefore => {
                composed.get_previous_sibling().unwrap().remove(false)?;
            }
            Op::RemoveTheBoldNodeAfter => {
                composed.get_next_sibling().unwrap().remove(false)?;
            }
            Op::UnboldTheNodeBefore => composed.get_previous_sibling().unwrap().set_format(0.0),
            Op::JoinTheNextLine => {
                let line = composed.get_parent_or_throw()?;
                let next = line.get_next_sibling().unwrap();
                line.append(&next.get_children())?;
                next.remove(false)?;
            }
        }
        Ok(())
    }
}

/// One snapshot of `run`'s outcome: the editor state's JSON and each line's
/// children, labelled.
#[derive(Debug, PartialEq)]
struct Step {
    json: String,
    nodes: Vec<String>,
}

/// `createTestEditor()`.
fn create_test_editor() -> Editor {
    create_editor(CreateEditorArgs {
        namespace: "test".to_owned(),
        ..CreateEditorArgs::default()
    })
}

/// `$composedText()`: the text node holding the composed text (it may have
/// been merged).
fn composed_text() -> NodeRef {
    get_root()
        .get_all_text_nodes()
        .into_iter()
        .find(|node| node.get_text_content().contains('に'))
        .expect("the composed text is gone")
}

fn discrete(tag: Option<&str>, skip_transforms: bool) -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        tags: tag.into_iter().map(str::to_owned).collect(),
        skip_transforms,
        ..UpdateOptions::default()
    }
}

/// `run(op, tag, skipTransforms, composing)`.
fn run(op: Op, tag: Option<&'static str>, skip_transforms: bool, composing: bool) -> Vec<Step> {
    let editor = create_test_editor();
    let labels = Rc::new(std::cell::RefCell::new(HashMap::<String, &str>::new()));
    let snapshot = |editor: &Editor| {
        let labels = labels.clone();
        editor
            .read(|| Step {
                json: editor.get_editor_state().to_json(false).to_string(),
                nodes: get_root()
                    .get_children()
                    .iter()
                    .inspect(|node| assert!(node.is_element()))
                    .flat_map(NodeRef::get_children)
                    .map(|n| {
                        format!(
                            "{}:{}{}",
                            labels.borrow().get(n.get_key()).copied().unwrap_or("new"),
                            n.get_text_content(),
                            if n.is_text() && n.text_data().get_format() != 0.0 {
                                "{b}"
                            } else {
                                ""
                            }
                        )
                    })
                    .collect(),
            })
            .unwrap()
    };
    let mut steps = Vec::new();
    let l = labels.clone();
    editor
        .update(
            move || {
                let before = create_text_node("j1z");
                let bold1 = create_text_node("B");
                bold1.toggle_format("bold");
                let composed = create_text_node("にj2z");
                let bold2 = create_text_node("C");
                bold2.toggle_format("bold");
                let after = create_text_node("j3z");
                let joined = create_text_node("k1z");
                let is_bold = op.name().contains("bold");
                get_root().clear()?;
                let line = create_paragraph_node();
                line.append(&if is_bold {
                    vec![
                        before.clone(),
                        bold1.clone(),
                        composed.clone(),
                        bold2.clone(),
                        after.clone(),
                    ]
                } else {
                    vec![composed.clone()]
                })?;
                let next = create_paragraph_node();
                next.append(std::slice::from_ref(&joined))?;
                get_root().append(&[line, next])?;
                for (node, label) in [
                    (&before, "before"),
                    (&bold1, "bold1"),
                    (&composed, "composed"),
                    (&bold2, "bold2"),
                    (&after, "after"),
                    (&joined, "joined"),
                ] {
                    l.borrow_mut().insert(node.get_key().to_owned(), label);
                }
                composed.select(Some(1), Some(1))?;
                if composing {
                    set_composition_key(Some(composed.get_key()));
                } else {
                    crate::lexical_utils::get_writable(composed.get_key());
                }
                Ok(())
            },
            discrete(None, false),
        )
        .unwrap();
    steps.push(snapshot(&editor));
    editor
        .update(
            move || op.run(&composed_text()),
            discrete(tag, skip_transforms),
        )
        .unwrap();
    steps.push(snapshot(&editor));
    // The next composition step: the composed node's text changes.
    update(&editor, || {
        let node = composed_text();
        node.set_text_content(&node.get_text_content().replacen('に', "にほ", 1));
        Ok(())
    });
    steps.push(snapshot(&editor));
    // The composition ends.
    update(&editor, move || {
        if composing {
            set_composition_key(None);
        } else {
            crate::lexical_utils::get_writable(composed_text().get_key());
        }
        Ok(())
    });
    steps.push(snapshot(&editor));
    steps
}

fn same_with_and_without_composition(op: Op, tag: Option<&'static str>, skip_transforms: bool) {
    assert_eq!(
        run(op, tag, skip_transforms, true),
        run(op, tag, skip_transforms, false)
    );
}

fn nodes_of(steps: Vec<Step>) -> Vec<Vec<String>> {
    steps.into_iter().map(|s| s.nodes).collect()
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > text inserted before the composed node merges with it at once
#[test]
fn text_inserted_before_the_composed_node_merges_with_it_at_once() {
    assert_eq!(
        nodes_of(run(Op::InsertBefore, None, false, true)),
        [
            ["composed:にj2z", "joined:k1z"],
            ["new:j1zにj2z", "joined:k1z"],
            ["new:j1zにほj2z", "joined:k1z"],
            ["new:j1zにほj2z", "joined:k1z"],
        ]
    );
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > a line joined into the composing line merges at once
#[test]
fn a_line_joined_into_the_composing_line_merges_at_once() {
    assert_eq!(
        nodes_of(run(Op::JoinTheNextLine, None, true, true)),
        [
            vec!["composed:にj2z", "joined:k1z"],
            vec!["composed:にj2zk1z"],
            vec!["composed:にほj2zk1z"],
            vec!["composed:にほj2zk1z"],
        ]
    );
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag (none)
#[test]
fn insert_before_tag_none() {
    same_with_and_without_composition(Op::InsertBefore, None, false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag (none), skipTransforms
#[test]
fn insert_before_tag_none_skip_transforms() {
    same_with_and_without_composition(Op::InsertBefore, None, true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag collaboration
#[test]
fn insert_before_tag_collaboration() {
    same_with_and_without_composition(Op::InsertBefore, Some(COLLABORATION_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag collaboration, skipTransforms
#[test]
fn insert_before_tag_collaboration_skip_transforms() {
    same_with_and_without_composition(Op::InsertBefore, Some(COLLABORATION_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag historic
#[test]
fn insert_before_tag_historic() {
    same_with_and_without_composition(Op::InsertBefore, Some(HISTORIC_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag historic, skipTransforms
#[test]
fn insert_before_tag_historic_skip_transforms() {
    same_with_and_without_composition(Op::InsertBefore, Some(HISTORIC_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag history-merge
#[test]
fn insert_before_tag_history_merge() {
    same_with_and_without_composition(Op::InsertBefore, Some(HISTORY_MERGE_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag history-merge, skipTransforms
#[test]
fn insert_before_tag_history_merge_skip_transforms() {
    same_with_and_without_composition(Op::InsertBefore, Some(HISTORY_MERGE_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag paste
#[test]
fn insert_before_tag_paste() {
    same_with_and_without_composition(Op::InsertBefore, Some(PASTE_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag paste, skipTransforms
#[test]
fn insert_before_tag_paste_skip_transforms() {
    same_with_and_without_composition(Op::InsertBefore, Some(PASTE_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag skip-collab
#[test]
fn insert_before_tag_skip_collab() {
    same_with_and_without_composition(Op::InsertBefore, Some(SKIP_COLLAB_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag skip-collab, skipTransforms
#[test]
fn insert_before_tag_skip_collab_skip_transforms() {
    same_with_and_without_composition(Op::InsertBefore, Some(SKIP_COLLAB_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag composition-start
#[test]
fn insert_before_tag_composition_start() {
    same_with_and_without_composition(Op::InsertBefore, Some(COMPOSITION_START_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag composition-start, skipTransforms
#[test]
fn insert_before_tag_composition_start_skip_transforms() {
    same_with_and_without_composition(Op::InsertBefore, Some(COMPOSITION_START_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag composition-end
#[test]
fn insert_before_tag_composition_end() {
    same_with_and_without_composition(Op::InsertBefore, Some(COMPOSITION_END_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert before, tag composition-end, skipTransforms
#[test]
fn insert_before_tag_composition_end_skip_transforms() {
    same_with_and_without_composition(Op::InsertBefore, Some(COMPOSITION_END_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag (none)
#[test]
fn insert_after_tag_none() {
    same_with_and_without_composition(Op::InsertAfter, None, false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag (none), skipTransforms
#[test]
fn insert_after_tag_none_skip_transforms() {
    same_with_and_without_composition(Op::InsertAfter, None, true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag collaboration
#[test]
fn insert_after_tag_collaboration() {
    same_with_and_without_composition(Op::InsertAfter, Some(COLLABORATION_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag collaboration, skipTransforms
#[test]
fn insert_after_tag_collaboration_skip_transforms() {
    same_with_and_without_composition(Op::InsertAfter, Some(COLLABORATION_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag historic
#[test]
fn insert_after_tag_historic() {
    same_with_and_without_composition(Op::InsertAfter, Some(HISTORIC_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag historic, skipTransforms
#[test]
fn insert_after_tag_historic_skip_transforms() {
    same_with_and_without_composition(Op::InsertAfter, Some(HISTORIC_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag history-merge
#[test]
fn insert_after_tag_history_merge() {
    same_with_and_without_composition(Op::InsertAfter, Some(HISTORY_MERGE_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag history-merge, skipTransforms
#[test]
fn insert_after_tag_history_merge_skip_transforms() {
    same_with_and_without_composition(Op::InsertAfter, Some(HISTORY_MERGE_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag paste
#[test]
fn insert_after_tag_paste() {
    same_with_and_without_composition(Op::InsertAfter, Some(PASTE_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag paste, skipTransforms
#[test]
fn insert_after_tag_paste_skip_transforms() {
    same_with_and_without_composition(Op::InsertAfter, Some(PASTE_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag skip-collab
#[test]
fn insert_after_tag_skip_collab() {
    same_with_and_without_composition(Op::InsertAfter, Some(SKIP_COLLAB_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag skip-collab, skipTransforms
#[test]
fn insert_after_tag_skip_collab_skip_transforms() {
    same_with_and_without_composition(Op::InsertAfter, Some(SKIP_COLLAB_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag composition-start
#[test]
fn insert_after_tag_composition_start() {
    same_with_and_without_composition(Op::InsertAfter, Some(COMPOSITION_START_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag composition-start, skipTransforms
#[test]
fn insert_after_tag_composition_start_skip_transforms() {
    same_with_and_without_composition(Op::InsertAfter, Some(COMPOSITION_START_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag composition-end
#[test]
fn insert_after_tag_composition_end() {
    same_with_and_without_composition(Op::InsertAfter, Some(COMPOSITION_END_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert after, tag composition-end, skipTransforms
#[test]
fn insert_after_tag_composition_end_skip_transforms() {
    same_with_and_without_composition(Op::InsertAfter, Some(COMPOSITION_END_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag (none)
#[test]
fn insert_on_both_sides_tag_none() {
    same_with_and_without_composition(Op::InsertOnBothSides, None, false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag (none), skipTransforms
#[test]
fn insert_on_both_sides_tag_none_skip_transforms() {
    same_with_and_without_composition(Op::InsertOnBothSides, None, true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag collaboration
#[test]
fn insert_on_both_sides_tag_collaboration() {
    same_with_and_without_composition(Op::InsertOnBothSides, Some(COLLABORATION_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag collaboration, skipTransforms
#[test]
fn insert_on_both_sides_tag_collaboration_skip_transforms() {
    same_with_and_without_composition(Op::InsertOnBothSides, Some(COLLABORATION_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag historic
#[test]
fn insert_on_both_sides_tag_historic() {
    same_with_and_without_composition(Op::InsertOnBothSides, Some(HISTORIC_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag historic, skipTransforms
#[test]
fn insert_on_both_sides_tag_historic_skip_transforms() {
    same_with_and_without_composition(Op::InsertOnBothSides, Some(HISTORIC_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag history-merge
#[test]
fn insert_on_both_sides_tag_history_merge() {
    same_with_and_without_composition(Op::InsertOnBothSides, Some(HISTORY_MERGE_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag history-merge, skipTransforms
#[test]
fn insert_on_both_sides_tag_history_merge_skip_transforms() {
    same_with_and_without_composition(Op::InsertOnBothSides, Some(HISTORY_MERGE_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag paste
#[test]
fn insert_on_both_sides_tag_paste() {
    same_with_and_without_composition(Op::InsertOnBothSides, Some(PASTE_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag paste, skipTransforms
#[test]
fn insert_on_both_sides_tag_paste_skip_transforms() {
    same_with_and_without_composition(Op::InsertOnBothSides, Some(PASTE_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag skip-collab
#[test]
fn insert_on_both_sides_tag_skip_collab() {
    same_with_and_without_composition(Op::InsertOnBothSides, Some(SKIP_COLLAB_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag skip-collab, skipTransforms
#[test]
fn insert_on_both_sides_tag_skip_collab_skip_transforms() {
    same_with_and_without_composition(Op::InsertOnBothSides, Some(SKIP_COLLAB_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag composition-start
#[test]
fn insert_on_both_sides_tag_composition_start() {
    same_with_and_without_composition(Op::InsertOnBothSides, Some(COMPOSITION_START_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag composition-start, skipTransforms
#[test]
fn insert_on_both_sides_tag_composition_start_skip_transforms() {
    same_with_and_without_composition(Op::InsertOnBothSides, Some(COMPOSITION_START_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag composition-end
#[test]
fn insert_on_both_sides_tag_composition_end() {
    same_with_and_without_composition(Op::InsertOnBothSides, Some(COMPOSITION_END_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > insert on both sides, tag composition-end, skipTransforms
#[test]
fn insert_on_both_sides_tag_composition_end_skip_transforms() {
    same_with_and_without_composition(Op::InsertOnBothSides, Some(COMPOSITION_END_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag (none)
#[test]
fn remove_the_bold_node_before_tag_none() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeBefore, None, false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag (none), skipTransforms
#[test]
fn remove_the_bold_node_before_tag_none_skip_transforms() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeBefore, None, true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag collaboration
#[test]
fn remove_the_bold_node_before_tag_collaboration() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeBefore, Some(COLLABORATION_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag collaboration, skipTransforms
#[test]
fn remove_the_bold_node_before_tag_collaboration_skip_transforms() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeBefore, Some(COLLABORATION_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag historic
#[test]
fn remove_the_bold_node_before_tag_historic() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeBefore, Some(HISTORIC_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag historic, skipTransforms
#[test]
fn remove_the_bold_node_before_tag_historic_skip_transforms() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeBefore, Some(HISTORIC_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag history-merge
#[test]
fn remove_the_bold_node_before_tag_history_merge() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeBefore, Some(HISTORY_MERGE_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag history-merge, skipTransforms
#[test]
fn remove_the_bold_node_before_tag_history_merge_skip_transforms() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeBefore, Some(HISTORY_MERGE_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag paste
#[test]
fn remove_the_bold_node_before_tag_paste() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeBefore, Some(PASTE_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag paste, skipTransforms
#[test]
fn remove_the_bold_node_before_tag_paste_skip_transforms() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeBefore, Some(PASTE_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag skip-collab
#[test]
fn remove_the_bold_node_before_tag_skip_collab() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeBefore, Some(SKIP_COLLAB_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag skip-collab, skipTransforms
#[test]
fn remove_the_bold_node_before_tag_skip_collab_skip_transforms() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeBefore, Some(SKIP_COLLAB_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag composition-start
#[test]
fn remove_the_bold_node_before_tag_composition_start() {
    same_with_and_without_composition(
        Op::RemoveTheBoldNodeBefore,
        Some(COMPOSITION_START_TAG),
        false,
    );
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag composition-start, skipTransforms
#[test]
fn remove_the_bold_node_before_tag_composition_start_skip_transforms() {
    same_with_and_without_composition(
        Op::RemoveTheBoldNodeBefore,
        Some(COMPOSITION_START_TAG),
        true,
    );
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag composition-end
#[test]
fn remove_the_bold_node_before_tag_composition_end() {
    same_with_and_without_composition(
        Op::RemoveTheBoldNodeBefore,
        Some(COMPOSITION_END_TAG),
        false,
    );
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node before, tag composition-end, skipTransforms
#[test]
fn remove_the_bold_node_before_tag_composition_end_skip_transforms() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeBefore, Some(COMPOSITION_END_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag (none)
#[test]
fn remove_the_bold_node_after_tag_none() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeAfter, None, false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag (none), skipTransforms
#[test]
fn remove_the_bold_node_after_tag_none_skip_transforms() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeAfter, None, true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag collaboration
#[test]
fn remove_the_bold_node_after_tag_collaboration() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeAfter, Some(COLLABORATION_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag collaboration, skipTransforms
#[test]
fn remove_the_bold_node_after_tag_collaboration_skip_transforms() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeAfter, Some(COLLABORATION_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag historic
#[test]
fn remove_the_bold_node_after_tag_historic() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeAfter, Some(HISTORIC_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag historic, skipTransforms
#[test]
fn remove_the_bold_node_after_tag_historic_skip_transforms() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeAfter, Some(HISTORIC_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag history-merge
#[test]
fn remove_the_bold_node_after_tag_history_merge() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeAfter, Some(HISTORY_MERGE_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag history-merge, skipTransforms
#[test]
fn remove_the_bold_node_after_tag_history_merge_skip_transforms() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeAfter, Some(HISTORY_MERGE_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag paste
#[test]
fn remove_the_bold_node_after_tag_paste() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeAfter, Some(PASTE_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag paste, skipTransforms
#[test]
fn remove_the_bold_node_after_tag_paste_skip_transforms() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeAfter, Some(PASTE_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag skip-collab
#[test]
fn remove_the_bold_node_after_tag_skip_collab() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeAfter, Some(SKIP_COLLAB_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag skip-collab, skipTransforms
#[test]
fn remove_the_bold_node_after_tag_skip_collab_skip_transforms() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeAfter, Some(SKIP_COLLAB_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag composition-start
#[test]
fn remove_the_bold_node_after_tag_composition_start() {
    same_with_and_without_composition(
        Op::RemoveTheBoldNodeAfter,
        Some(COMPOSITION_START_TAG),
        false,
    );
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag composition-start, skipTransforms
#[test]
fn remove_the_bold_node_after_tag_composition_start_skip_transforms() {
    same_with_and_without_composition(
        Op::RemoveTheBoldNodeAfter,
        Some(COMPOSITION_START_TAG),
        true,
    );
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag composition-end
#[test]
fn remove_the_bold_node_after_tag_composition_end() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeAfter, Some(COMPOSITION_END_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > remove the bold node after, tag composition-end, skipTransforms
#[test]
fn remove_the_bold_node_after_tag_composition_end_skip_transforms() {
    same_with_and_without_composition(Op::RemoveTheBoldNodeAfter, Some(COMPOSITION_END_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag (none)
#[test]
fn unbold_the_node_before_tag_none() {
    same_with_and_without_composition(Op::UnboldTheNodeBefore, None, false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag (none), skipTransforms
#[test]
fn unbold_the_node_before_tag_none_skip_transforms() {
    same_with_and_without_composition(Op::UnboldTheNodeBefore, None, true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag collaboration
#[test]
fn unbold_the_node_before_tag_collaboration() {
    same_with_and_without_composition(Op::UnboldTheNodeBefore, Some(COLLABORATION_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag collaboration, skipTransforms
#[test]
fn unbold_the_node_before_tag_collaboration_skip_transforms() {
    same_with_and_without_composition(Op::UnboldTheNodeBefore, Some(COLLABORATION_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag historic
#[test]
fn unbold_the_node_before_tag_historic() {
    same_with_and_without_composition(Op::UnboldTheNodeBefore, Some(HISTORIC_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag historic, skipTransforms
#[test]
fn unbold_the_node_before_tag_historic_skip_transforms() {
    same_with_and_without_composition(Op::UnboldTheNodeBefore, Some(HISTORIC_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag history-merge
#[test]
fn unbold_the_node_before_tag_history_merge() {
    same_with_and_without_composition(Op::UnboldTheNodeBefore, Some(HISTORY_MERGE_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag history-merge, skipTransforms
#[test]
fn unbold_the_node_before_tag_history_merge_skip_transforms() {
    same_with_and_without_composition(Op::UnboldTheNodeBefore, Some(HISTORY_MERGE_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag paste
#[test]
fn unbold_the_node_before_tag_paste() {
    same_with_and_without_composition(Op::UnboldTheNodeBefore, Some(PASTE_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag paste, skipTransforms
#[test]
fn unbold_the_node_before_tag_paste_skip_transforms() {
    same_with_and_without_composition(Op::UnboldTheNodeBefore, Some(PASTE_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag skip-collab
#[test]
fn unbold_the_node_before_tag_skip_collab() {
    same_with_and_without_composition(Op::UnboldTheNodeBefore, Some(SKIP_COLLAB_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag skip-collab, skipTransforms
#[test]
fn unbold_the_node_before_tag_skip_collab_skip_transforms() {
    same_with_and_without_composition(Op::UnboldTheNodeBefore, Some(SKIP_COLLAB_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag composition-start
#[test]
fn unbold_the_node_before_tag_composition_start() {
    same_with_and_without_composition(Op::UnboldTheNodeBefore, Some(COMPOSITION_START_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag composition-start, skipTransforms
#[test]
fn unbold_the_node_before_tag_composition_start_skip_transforms() {
    same_with_and_without_composition(Op::UnboldTheNodeBefore, Some(COMPOSITION_START_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag composition-end
#[test]
fn unbold_the_node_before_tag_composition_end() {
    same_with_and_without_composition(Op::UnboldTheNodeBefore, Some(COMPOSITION_END_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > unbold the node before, tag composition-end, skipTransforms
#[test]
fn unbold_the_node_before_tag_composition_end_skip_transforms() {
    same_with_and_without_composition(Op::UnboldTheNodeBefore, Some(COMPOSITION_END_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag (none)
#[test]
fn join_the_next_line_tag_none() {
    same_with_and_without_composition(Op::JoinTheNextLine, None, false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag (none), skipTransforms
#[test]
fn join_the_next_line_tag_none_skip_transforms() {
    same_with_and_without_composition(Op::JoinTheNextLine, None, true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag collaboration
#[test]
fn join_the_next_line_tag_collaboration() {
    same_with_and_without_composition(Op::JoinTheNextLine, Some(COLLABORATION_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag collaboration, skipTransforms
#[test]
fn join_the_next_line_tag_collaboration_skip_transforms() {
    same_with_and_without_composition(Op::JoinTheNextLine, Some(COLLABORATION_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag historic
#[test]
fn join_the_next_line_tag_historic() {
    same_with_and_without_composition(Op::JoinTheNextLine, Some(HISTORIC_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag historic, skipTransforms
#[test]
fn join_the_next_line_tag_historic_skip_transforms() {
    same_with_and_without_composition(Op::JoinTheNextLine, Some(HISTORIC_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag history-merge
#[test]
fn join_the_next_line_tag_history_merge() {
    same_with_and_without_composition(Op::JoinTheNextLine, Some(HISTORY_MERGE_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag history-merge, skipTransforms
#[test]
fn join_the_next_line_tag_history_merge_skip_transforms() {
    same_with_and_without_composition(Op::JoinTheNextLine, Some(HISTORY_MERGE_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag paste
#[test]
fn join_the_next_line_tag_paste() {
    same_with_and_without_composition(Op::JoinTheNextLine, Some(PASTE_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag paste, skipTransforms
#[test]
fn join_the_next_line_tag_paste_skip_transforms() {
    same_with_and_without_composition(Op::JoinTheNextLine, Some(PASTE_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag skip-collab
#[test]
fn join_the_next_line_tag_skip_collab() {
    same_with_and_without_composition(Op::JoinTheNextLine, Some(SKIP_COLLAB_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag skip-collab, skipTransforms
#[test]
fn join_the_next_line_tag_skip_collab_skip_transforms() {
    same_with_and_without_composition(Op::JoinTheNextLine, Some(SKIP_COLLAB_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag composition-start
#[test]
fn join_the_next_line_tag_composition_start() {
    same_with_and_without_composition(Op::JoinTheNextLine, Some(COMPOSITION_START_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag composition-start, skipTransforms
#[test]
fn join_the_next_line_tag_composition_start_skip_transforms() {
    same_with_and_without_composition(Op::JoinTheNextLine, Some(COMPOSITION_START_TAG), true);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag composition-end
#[test]
fn join_the_next_line_tag_composition_end() {
    same_with_and_without_composition(Op::JoinTheNextLine, Some(COMPOSITION_END_TAG), false);
}

// upstream: lexical/src/__tests__/unit/LexicalNormalizationComposition.test.ts::Normalization next to an IME composition, without DEFER_COMPOSITION_MERGE_TAG > join the next line, tag composition-end, skipTransforms
#[test]
fn join_the_next_line_tag_composition_end_skip_transforms() {
    same_with_and_without_composition(Op::JoinTheNextLine, Some(COMPOSITION_END_TAG), true);
}
