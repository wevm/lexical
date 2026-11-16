//! Ports of `packages/lexical-extension/src/__tests__/unit/BlockIdExtension.handlesCost.test.ts`:
//! the cost of reading the handle of every block, counted in calls of
//! `$isIdentifiedBlock` rather than timed: it must grow linearly with the
//! number of blocks, also when many blocks hold one id (which persists
//! while the extension is disabled).

use crate::extension::{
    BlockIdConfigOverrides, IsIdentifiedBlock, block_id_extension, build_editor_from_extensions,
    get_block_handle, get_block_handles, is_root_child_block,
};
use crate::json::Value;
use crate::{
    InitialEditorStateType, LexicalExtension, config_extension, define_extension, get_root,
};
use std::cell::Cell;
use std::rc::Rc;

/// `countCalls(count, {disabled, duplicate})`: `(calls, handles)`.
fn count_calls(count: usize, disabled: bool, duplicate: bool) -> (usize, usize) {
    let calls = Rc::new(Cell::new(0));
    let counted = calls.clone();
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies: vec![
            crate::headless::headless_extension().into(),
            config_extension(
                &block_id_extension(),
                [BlockIdConfigOverrides {
                    is_identified_block: Some(IsIdentifiedBlock(Rc::new(move |node| {
                        counted.set(counted.get() + 1);
                        is_root_child_block(node)
                    }))),
                    disabled: Some(disabled),
                    ..Default::default()
                }],
            ),
        ],
        ..LexicalExtension::<(), (), ()>::new("[block-handle-cost]", ())
    })
    .into()])
    .unwrap();
    let children: Vec<String> = (0..count)
        .map(|i| {
            let id = if duplicate {
                "shared-block-id".to_owned()
            } else {
                format!("block-{i:0>12}")
            };
            format!(
                r#"{{"$":{{"blockId":{}}},"children":[{{"text":"p{i}","type":"text"}}],"type":"paragraph"}}"#,
                Value::from(id)
            )
        })
        .collect();
    let json = format!(
        r#"{{"root":{{"children":[{}],"type":"root"}}}}"#,
        children.join(",")
    );
    editor
        .set_editor_state(&editor.parse_editor_state(&json).unwrap(), None)
        .unwrap();
    // Build what reading needs once (the index, the scan, the table), so
    // that only the reads are counted.
    editor.read(get_block_handles).unwrap().unwrap();
    calls.set(0);
    let handles = editor
        .read(|| {
            get_root()
                .get_children()
                .iter()
                .filter(|node| get_block_handle(node).unwrap().is_some())
                .count()
        })
        .unwrap();
    editor.dispose();
    (calls.get(), handles)
}

/// `reading every handle costs linear time`. While enabled, loading
/// repairs the repeated id (every block gets an id of its own); while
/// disabled, every block keeps it.
fn linear(disabled: bool, duplicate: bool) {
    let rows: Vec<(usize, usize)> = [40, 80, 160]
        .iter()
        .map(|&count| count_calls(count, disabled, duplicate))
        .collect();
    for (i, (_, handles)) in rows.iter().enumerate() {
        // Only the first holder of a repeated id has a handle.
        let expected = if duplicate && disabled { 1 } else { 40 << i };
        assert_eq!(*handles, expected, "row {i}");
    }
    // Doubling the blocks at most doubles the calls (plus slack).
    let calls = |i: usize| rows[i].0 as f64;
    assert!(calls(2) <= 2.2 * calls(1) + 4.0, "{rows:?}");
    assert!(calls(1) <= 2.2 * calls(0) + 4.0, "{rows:?}");
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handlesCost.test.ts::reading every handle costs linear time > disabled, unique ids
#[test]
fn disabled_unique_ids() {
    linear(true, false);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handlesCost.test.ts::reading every handle costs linear time > disabled, one id repeated
#[test]
fn disabled_one_id_repeated() {
    linear(true, true);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handlesCost.test.ts::reading every handle costs linear time > enabled, unique ids
#[test]
fn enabled_unique_ids() {
    linear(false, false);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handlesCost.test.ts::reading every handle costs linear time > enabled, one id repeated
#[test]
fn enabled_one_id_repeated() {
    linear(false, true);
}
