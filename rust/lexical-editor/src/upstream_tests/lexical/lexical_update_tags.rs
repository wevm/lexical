//! Ports of `packages/lexical/src/__tests__/unit/LexicalUpdateTags.test.ts`.

use super::super::support::{initialize_unit_test, update};
use crate::{
    COLLABORATION_TAG, CUT_TAG, CreateEditorArgs, HISTORIC_TAG, HISTORY_MERGE_TAG,
    HISTORY_PUSH_TAG, PASTE_TAG, SKIP_DOM_SELECTION_TAG, SKIP_SCROLL_INTO_VIEW_TAG, UpdateOptions,
    add_update_tag, create_paragraph_node, get_root, has_update_tag,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// upstream: lexical/src/__tests__/unit/LexicalUpdateTags.test.ts::LexicalUpdateTags tests > Built-in update tags work correctly
#[test]
fn built_in_update_tags_work_correctly() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let built_in_tags = [
            HISTORIC_TAG,
            HISTORY_PUSH_TAG,
            HISTORY_MERGE_TAG,
            PASTE_TAG,
            CUT_TAG,
            COLLABORATION_TAG,
            SKIP_DOM_SELECTION_TAG,
            SKIP_SCROLL_INTO_VIEW_TAG,
        ];
        for tag in built_in_tags {
            add_update_tag(tag);
            assert!(has_update_tag(tag));
        }
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalUpdateTags.test.ts::LexicalUpdateTags tests > $addUpdateTag and $hasUpdateTag work correctly
#[test]
fn add_update_tag_and_has_update_tag_work_correctly() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let tag = "test-tag";
        assert!(!has_update_tag(tag));
        add_update_tag(tag);
        assert!(has_update_tag(tag));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalUpdateTags.test.ts::LexicalUpdateTags tests > Multiple update tags can be added
#[test]
fn multiple_update_tags_can_be_added() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let tags = ["tag1", "tag2", "tag3"];
        for tag in tags {
            add_update_tag(tag);
        }
        for tag in tags {
            assert!(has_update_tag(tag));
        }
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/LexicalUpdateTags.test.ts::LexicalUpdateTags tests > Update tags via editor.update() options work
#[test]
fn update_tags_via_editor_update_options_work() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let tag = "test-tag";
    let has_tag = Rc::new(Cell::new(false));
    let seen = has_tag.clone();
    editor
        .update(
            move || {
                seen.set(has_update_tag(tag));
                Ok(())
            },
            UpdateOptions {
                discrete: true,
                tags: vec![tag.to_owned()],
                ..UpdateOptions::default()
            },
        )
        .unwrap();
    assert!(has_tag.get());
}

// upstream: lexical/src/__tests__/unit/LexicalUpdateTags.test.ts::LexicalUpdateTags tests > Update tags are cleared after update
#[test]
fn update_tags_are_cleared_after_update() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let tag = HISTORIC_TAG;
    update(&editor, move || {
        add_update_tag(tag);
        assert!(has_update_tag(tag));
        Ok(())
    });
    let has_tag = Rc::new(Cell::new(false));
    let seen = has_tag.clone();
    update(&editor, move || {
        seen.set(has_update_tag(tag));
        Ok(())
    });
    assert!(!has_tag.get());
}

// upstream: lexical/src/__tests__/unit/LexicalUpdateTags.test.ts::LexicalUpdateTags tests > Update tags affect editor behavior
#[test]
fn update_tags_affect_editor_behavior() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let calls = Rc::new(RefCell::new(Vec::<Vec<String>>::new()));
    let log = calls.clone();
    let _ = editor.register_update_listener(move |payload| {
        log.borrow_mut().push(payload.tags.clone());
    });
    editor
        .update(
            || {
                let root = get_root();
                let paragraph = create_paragraph_node();
                root.append(&[paragraph])?;
                Ok(())
            },
            UpdateOptions {
                discrete: true,
                tags: vec![SKIP_DOM_SELECTION_TAG.to_owned()],
                ..UpdateOptions::default()
            },
        )
        .unwrap();
    assert!(
        calls
            .borrow()
            .iter()
            .any(|tags| tags.iter().any(|t| t == SKIP_DOM_SELECTION_TAG))
    );
}
