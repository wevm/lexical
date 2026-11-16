//! Ports of `packages/lexical-extension/src/__tests__/unit/BlockIdExtension.handlesCache.test.ts`.
//!
//! JS spies on `buildHandleTable` with `vi.mock`; the crate counts its
//! calls in tests ([`BUILD_HANDLE_TABLE_CALLS`]).

#![cfg(feature = "rich-text")]

use super::super::support::update;
use super::support::{as_edited, paragraph};
use crate::extension::block_handles::BUILD_HANDLE_TABLE_CALLS;
use crate::extension::{
    BlockIdConfigOverrides, GenerateId, block_id_extension, block_id_state,
    build_editor_from_extensions, get_block_by_id, get_block_handle, get_block_handles,
    resolve_block_handle,
};
use crate::{
    Editor, InitialEditorStateType, LexicalExtension, config_extension, create_paragraph_node,
    define_extension, get_root, set_state,
};
use std::cell::Cell;
use std::rc::Rc;

fn builds() -> usize {
    BUILD_HANDLE_TABLE_CALLS.with(Cell::get)
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.handlesCache.test.ts::block handle cache > builds the handles once per set of ids
#[test]
fn builds_the_handles_once_per_set_of_ids() {
    const IDS: [&str; 2] = ["V1StGXR8_Z5jdHi6B-myT", "9xQeLm2Pq0aK7cTb_wYuA"];
    let n = Cell::new(0);
    let editor = as_edited(
        build_editor_from_extensions([define_extension(LexicalExtension {
            initial_editor_state: Some(InitialEditorStateType::Null),
            dependencies: vec![
                crate::rich_text::rich_text_extension().into(),
                config_extension(
                    &block_id_extension(),
                    [BlockIdConfigOverrides {
                        generate_id: Some(GenerateId(Rc::new(move || {
                            let i = n.get();
                            n.set(i + 1);
                            IDS.get(i)
                                .map_or_else(|| format!("zz{i}"), |id| (*id).to_owned())
                        }))),
                        ..Default::default()
                    }],
                ),
            ],
            ..LexicalExtension::<(), (), ()>::new("[block-handle-cache]", ())
        })
        .into()])
        .unwrap(),
    );
    update(&editor, || {
        get_root().append(&[paragraph("a"), paragraph("b")])?;
        Ok(())
    });
    let read_all = |editor: &Editor| {
        editor
            .read(|| {
                for node in get_root().get_children() {
                    get_block_handle(&node).unwrap();
                }
                get_block_handles().unwrap();
                for handle in ["V1St", "9xQe", "zzzz"] {
                    resolve_block_handle(handle, None).unwrap();
                }
            })
            .unwrap();
    };
    let start = builds();

    read_all(&editor);
    read_all(&editor);
    assert_eq!(builds() - start, 1);

    // Typing changes no id: no rebuild.
    update(&editor, || {
        let text = get_root().get_first_descendant().unwrap();
        assert!(text.is_text());
        text.set_text_content("typed");
        Ok(())
    });
    read_all(&editor);
    assert_eq!(builds() - start, 1);

    // A new id: one rebuild.
    update(&editor, || {
        get_root().append(&[create_paragraph_node()])?;
        Ok(())
    });
    read_all(&editor);
    read_all(&editor);
    assert_eq!(builds() - start, 2);

    // A changed id: one rebuild.
    update(&editor, || {
        set_state(
            &get_block_by_id(IDS[1])?.unwrap(),
            &block_id_state(),
            Some("renamed".to_owned()),
        )?;
        Ok(())
    });
    read_all(&editor);
    assert_eq!(builds() - start, 3);
    editor.dispose();
}
