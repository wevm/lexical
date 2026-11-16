//! Ports of `packages/lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts`.
//!
//! Upstream's `SlotContainerNode` (a shadow-root element) is the shared
//! [`TestShadowRootNode`]; only its type name differs.

#![cfg(all(feature = "rich-text", feature = "history"))]

use super::super::support::{TestShadowRootNode, create_test_shadow_root_node};
use super::support::{as_edited, counter, strings};
use crate::extension::positional_block_id::{
    derive_duplicate_id, derive_positional_id, document_key,
};
use crate::extension::{
    BlockIdConfigOverrides, LexicalEditorWithDispose, block_id_extension, block_id_state,
    build_editor_from_extensions, default_is_identified_block, get_block_by_id, get_block_id,
    get_extension_dependency_from_editor,
};
use crate::rich_text::{HeadingTagType, create_heading_node};
use crate::selection::set_blocks_type;
use crate::{
    COLLABORATION_TAG, Editor, InitialEditorStateType, LexicalError, LexicalExtension, NodeClass,
    NodeRef, UNDO_COMMAND, UpdateOptions, config_extension, create_paragraph_node,
    create_text_node, define_extension, get_root, get_slot, get_slot_names, remove_slot, set_slot,
    set_state,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

/// `build({disabled, history})`.
fn build(disabled: bool, history: bool) -> LexicalEditorWithDispose {
    let mut dependencies = vec![crate::rich_text::rich_text_extension().into()];
    if history {
        let clock = Rc::new(Cell::new(0.0));
        dependencies.push(config_extension(
            &crate::history::history_extension(),
            [crate::history::HistoryConfigOverrides {
                disabled: Some(false),
                now: Some(crate::history::HistoryNow(Rc::new(move || {
                    clock.set(clock.get() + 10000.0);
                    clock.get()
                }))),
                ..Default::default()
            }],
        ));
    }
    dependencies.push(config_extension(
        &block_id_extension(),
        [BlockIdConfigOverrides {
            disabled: Some(disabled),
            generate_id: Some(counter("b")),
            ..Default::default()
        }],
    ));
    as_edited(
        build_editor_from_extensions([define_extension(LexicalExtension {
            initial_editor_state: Some(InitialEditorStateType::Null),
            dependencies,
            nodes: vec![NodeClass::of::<TestShadowRootNode>()],
            ..LexicalExtension::<(), (), ()>::new("[block-id-slots-test]", ())
        })
        .into()])
        .unwrap(),
    )
}

fn default_editor() -> LexicalEditorWithDispose {
    build(false, false)
}

/// `update(editor, fn, {skipTransforms, tag})`.
fn update(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    super::super::support::update(editor, f);
}

/// A committed merge: `{skipTransforms: true, tag: COLLABORATION_TAG}`.
fn update_merge(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    editor
        .update(
            f,
            UpdateOptions {
                discrete: true,
                skip_transforms: true,
                tags: vec![COLLABORATION_TAG.to_owned()],
                ..UpdateOptions::default()
            },
        )
        .unwrap();
}

/// `$paragraph(text, id)`.
fn paragraph(text: &str, id: Option<&str>) -> NodeRef {
    let node = create_paragraph_node();
    node.append(&[create_text_node(text)]).unwrap();
    if let Some(id) = id {
        set_state(&node, &block_id_state(), Some(id.to_owned())).unwrap();
    }
    node
}

fn p(text: &str) -> NodeRef {
    paragraph(text, None)
}

/// `$host(index)`.
fn host(index: usize) -> NodeRef {
    let node = get_root().get_child_at_index(index).unwrap();
    assert!(node.is_element());
    node
}

/// `$container(index, name)`.
fn container(index: usize) -> NodeRef {
    let slot = get_slot(&host(index), "title").unwrap();
    assert_eq!(slot.get_type(), "test_shadow_root");
    slot
}

/// `$positionalAliases()`.
fn positional_aliases() -> HashMap<String, String> {
    let key = document_key();
    let mut aliases = HashMap::new();
    for i in 0..8 {
        aliases.insert(derive_positional_id(&key, &i.to_string()), format!("@{i}"));
        for j in 0..8 {
            aliases.insert(
                derive_positional_id(&key, &format!("{i}.{j}")),
                format!("@{i}.{j}"),
            );
        }
    }
    aliases
}

/// `outline(editor)`: every identified block in document order (a host's
/// slots before its children), as `text:id`, indented by depth.
fn outline(editor: &Editor) -> Vec<String> {
    editor
        .read(|| {
            let positional = positional_aliases();
            let mut out = Vec::new();
            fn visit(
                node: &NodeRef,
                depth: usize,
                positional: &HashMap<String, String>,
                out: &mut Vec<String>,
            ) {
                if default_is_identified_block(node) {
                    // The block's own text, without that of its slots.
                    let text = if node.is_element() {
                        node.get_children()
                            .iter()
                            .map(|c| c.get_text_content())
                            .collect::<String>()
                    } else {
                        node.get_text_content()
                    };
                    let shown = match get_block_id(node).unwrap() {
                        Some(id) => positional.get(&id).cloned().unwrap_or(id),
                        None => "null".to_owned(),
                    };
                    out.push(format!("{}{text}:{shown}", "  ".repeat(depth)));
                }
                if node.is_element() || node.is_decorator() {
                    for name in get_slot_names(node) {
                        if let Some(slot) = get_slot(node, &name) {
                            visit(&slot, depth + 1, positional, out);
                        }
                    }
                }
                if node.is_element() {
                    for child in node.get_children() {
                        visit(&child, depth + 1, positional, out);
                    }
                }
            }
            for child in get_root().get_children() {
                visit(&child, 0, &positional, &mut out);
            }
            out
        })
        .unwrap()
}

/// `$hostWithSlot(host, texts)`.
fn host_with_slot(host: &str, texts: &[&str]) -> NodeRef {
    let node = p(host);
    let container = create_test_shadow_root_node();
    container
        .append(&texts.iter().map(|t| p(t)).collect::<Vec<_>>())
        .unwrap();
    set_slot(&node, "title", &container).unwrap();
    node
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts::BlockIdExtension: slots > blocks inside a slot get ids in the commit that creates them
#[test]
fn blocks_inside_a_slot_get_ids_in_the_commit() {
    let editor = default_editor();
    let ids: Rc<RefCell<Vec<Vec<Option<String>>>>> = Rc::default();
    let record = ids.clone();
    let _unregister = editor.register_update_listener(move |payload| {
        record.borrow_mut().push(payload.editor_state.read(|| {
            container(0)
                .get_children()
                .iter()
                .map(|n| get_block_id(n).unwrap())
                .collect()
        }));
    });
    update(&editor, || {
        get_root().append(&[host_with_slot("host", &["one", "two"])])?;
        Ok(())
    });
    assert_eq!(
        *ids.borrow(),
        [vec![Some("b2".to_owned()), Some("b3".to_owned())]]
    );
    assert_eq!(
        outline(&editor),
        strings(&["host:b1", "    one:b2", "    two:b3"])
    );
    editor
        .read(|| {
            assert_eq!(
                get_block_by_id("b3").unwrap().unwrap().get_text_content(),
                "two"
            );
            // The container is a wrapper, the blocks in it hold text.
            assert_eq!(get_block_id(&container(0)).unwrap(), None);
            assert!(!default_is_identified_block(&container(0)));
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts::BlockIdExtension: slots > a block that is itself a slot value gets an id
#[test]
fn a_block_that_is_a_slot_value_gets_an_id() {
    let editor = default_editor();
    update(&editor, || {
        let host = p("host");
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "caption", &p("caption"))?;
        Ok(())
    });
    assert_eq!(outline(&editor), strings(&["host:b1", "  caption:b2"]));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts::BlockIdExtension: slots > a duplicate inside a slot yields to the existing block
#[test]
fn a_duplicate_inside_a_slot_yields() {
    let editor = default_editor();
    update(&editor, || {
        get_root().append(&[host_with_slot("host", &["one"]), p("two")])?;
        Ok(())
    });
    assert_eq!(
        outline(&editor),
        strings(&["host:b1", "    one:b2", "two:b3"])
    );
    // A copy of the last root block pasted into the slot, before the
    // original in document order.
    update(&editor, || {
        container(0).append(&[paragraph("copy of two", Some("b3"))])?;
        Ok(())
    });
    assert_eq!(
        outline(&editor),
        strings(&["host:b1", "    one:b2", "    copy of two:b4", "two:b3"])
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts::BlockIdExtension: slots > a committed duplicate across slots is repaired in document order
#[test]
fn a_committed_duplicate_across_slots_is_repaired_in_document_order() {
    // The nodes are created in either order; the block first in document
    // order (a host's slots before its children) keeps the id either way.
    let mut results = Vec::new();
    for slot_block_first in [true, false] {
        let editor = default_editor();
        update(&editor, || {
            // A root container whose children are identified, with a slot.
            let host = create_test_shadow_root_node();
            host.append(&[p("body")])?;
            set_slot(&host, "title", &create_test_shadow_root_node())?;
            get_root().append(&[host, p("after")])?;
            Ok(())
        });
        update_merge(&editor, move || {
            let make = |text: &str| paragraph(text, Some("same"));
            let (slot_block, body_block) = if slot_block_first {
                let first = make("in slot");
                (first, make("in host body"))
            } else {
                let first = make("in host body");
                (make("in slot"), first)
            };
            host(0).splice(0, 0, &[body_block])?;
            container(0).append(&[slot_block])?;
            Ok(())
        });
        results.push(outline(&editor));
        editor.dispose();
    }
    let renamed = derive_duplicate_id("same", "1");
    assert_eq!(
        results[0],
        [
            "in host bodybody:b1".to_owned(),
            // The title container is a slot value but not a block.
            "    in slot:same".to_owned(),
            // The later holder: renamed from the repeated id and ordinal.
            format!("  in host body:{renamed}"),
            "  body:b2".to_owned(),
            "after:b3".to_owned(),
        ]
    );
    assert_eq!(results[1], results[0]);
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts::BlockIdExtension: slots > a committed duplicate between two slots follows the slot order
#[test]
fn a_committed_duplicate_between_two_slots_follows_the_slot_order() {
    let mut results = Vec::new();
    for order in [["caption", "title"], ["title", "caption"]] {
        let editor = default_editor();
        update(&editor, || {
            get_root().append(&[p("host")])?;
            Ok(())
        });
        update_merge(&editor, move || {
            // Created (and slotted) in either order.
            for name in order {
                set_slot(
                    &host(0),
                    name,
                    &paragraph(&format!("in {name}"), Some("same")),
                )?;
            }
            Ok(())
        });
        results.push(outline(&editor));
        editor.dispose();
    }
    // Slots are kept in a canonical order.
    assert_eq!(results[1], results[0]);
    assert_eq!(
        results[0],
        [
            "host:b1".to_owned(),
            "  in caption:same".to_owned(),
            format!("  in title:{}", derive_duplicate_id("same", "1")),
        ]
    );
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts::BlockIdExtension: slots > replacing a slot value with $setSlot keeps the id
#[test]
fn replacing_a_slot_value_keeps_the_id() {
    let editor = default_editor();
    update(&editor, || {
        let host = p("host");
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "caption", &p("old caption"))?;
        Ok(())
    });
    assert_eq!(outline(&editor), strings(&["host:b1", "  old caption:b2"]));
    update(&editor, || {
        set_slot(
            &host(0),
            "caption",
            &create_heading_node(HeadingTagType::H2),
        )?;
        Ok(())
    });
    assert_eq!(outline(&editor), strings(&["host:b1", "  :b2"]));
    // Removing the slot and setting a new one later is not a replacement.
    update(&editor, || {
        remove_slot(&host(0), "caption")?;
        Ok(())
    });
    update(&editor, || {
        set_slot(&host(0), "caption", &p("new caption"))?;
        Ok(())
    });
    assert_eq!(outline(&editor), strings(&["host:b1", "  new caption:b3"]));
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts::BlockIdExtension: slots > a type change inside a slot keeps the id
#[test]
fn a_type_change_inside_a_slot_keeps_the_id() {
    let editor = default_editor();
    update(&editor, || {
        get_root().append(&[host_with_slot("host", &["one", "two"])])?;
        Ok(())
    });
    update(&editor, || {
        let two = container(0).get_children()[1].clone();
        assert!(two.is_element());
        two.select(Some(0), Some(0))?;
        let selection = two.select_start()?.clone_selection();
        set_blocks_type(
            Some(&selection),
            || create_heading_node(HeadingTagType::H1),
            None,
        )
    });
    assert_eq!(
        editor
            .read(|| container(0).get_children()[1].get_type().to_owned())
            .unwrap(),
        "heading"
    );
    assert_eq!(
        outline(&editor),
        strings(&["host:b1", "    one:b2", "    two:b3"])
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts::BlockIdExtension: slots > the index follows a state replaced wholesale (undo) into slots
#[test]
fn the_index_follows_an_undo_into_slots() {
    let editor = build(false, true);
    update(&editor, || {
        get_root().append(&[host_with_slot("host", &["one"])])?;
        Ok(())
    });
    update(&editor, || {
        container(0).append(&[p("two")])?;
        Ok(())
    });
    assert_eq!(
        outline(&editor),
        strings(&["host:b1", "    one:b2", "    two:b3"])
    );
    editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
    assert_eq!(outline(&editor), strings(&["host:b1", "    one:b2"]));
    editor
        .read(|| {
            assert_eq!(
                get_block_by_id("b2").unwrap().unwrap().get_text_content(),
                "one"
            );
            assert!(get_block_by_id("b3").unwrap().is_none());
        })
        .unwrap();
    // A JSON document loaded with setEditorState is indexed too.
    let json = editor.get_editor_state().to_json(false).to_string();
    let other = default_editor();
    other
        .set_editor_state(&other.parse_editor_state(&json).unwrap(), None)
        .unwrap();
    other
        .read(|| {
            assert_eq!(
                get_block_by_id("b2").unwrap().unwrap().get_text_content(),
                "one"
            );
        })
        .unwrap();
    other.dispose();
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts::BlockIdExtension: slots > removing a host drops the blocks of its slots from the index
#[test]
fn removing_a_host_drops_its_slot_blocks_from_the_index() {
    let editor = default_editor();
    update(&editor, || {
        get_root().append(&[host_with_slot("host", &["one"]), p("two")])?;
        Ok(())
    });
    update(&editor, || host(0).remove(false));
    assert!(
        editor
            .read(|| get_block_by_id("b2").unwrap().is_none())
            .unwrap()
    );
    // The id is free again: a block given it keeps it.
    update(&editor, || {
        get_root().append(&[paragraph("restored", Some("b2"))])?;
        Ok(())
    });
    assert_eq!(outline(&editor), strings(&["two:b3", "restored:b2"]));
    assert_eq!(
        editor
            .read(|| get_block_by_id("b2").unwrap().unwrap().get_text_content())
            .unwrap(),
        "restored"
    );
    editor.dispose();
}

// upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.slots.test.ts::BlockIdExtension: slots > while disabled, the lookup scans slots too
#[test]
fn while_disabled_the_lookup_scans_slots() {
    let editor = build(true, false);
    update(&editor, || {
        let host = host_with_slot("host", &["one"]);
        get_root().append(std::slice::from_ref(&host))?;
        set_state(&host, &block_id_state(), Some("h".to_owned()))?;
        let container = get_slot(&host, "title").unwrap();
        assert!(container.is_element());
        set_state(
            &container.get_first_child_or_throw()?,
            &block_id_state(),
            Some("manual".to_owned()),
        )?;
        Ok(())
    });
    let manual = |editor: &Editor| {
        editor
            .read(|| {
                get_block_by_id("manual")
                    .unwrap()
                    .unwrap()
                    .get_text_content()
            })
            .unwrap()
    };
    assert_eq!(manual(&editor), "one");
    get_extension_dependency_from_editor(&editor, &block_id_extension())
        .unwrap()
        .output
        .unwrap()
        .disabled
        .set_value(false);
    assert_eq!(outline(&editor), strings(&["host:h", "    one:manual"]));
    assert_eq!(manual(&editor), "one");
    editor.dispose();
}
