//! Ports of `packages/lexical/src/__tests__/unit/SlotParseEditorState.test.ts`.
#![cfg(feature = "extension")]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::node::NodeBase;
use crate::{
    ElementData, InitialEditorStateType, LexicalExtension, LexicalNode, NodeClass, UpdateOptions,
    create, create_paragraph_node, create_text_node, define_extension, get_root, set_slot,
};

/// A shadow-root block used as a slot value, as a slot host holds one.
#[derive(Clone, Debug, Default)]
struct SlotContainerNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for SlotContainerNode {
    fn get_type(&self) -> &'static str {
        "slot_container_parse"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn element(&self) -> Option<&ElementData> {
        Some(&self.element)
    }
    fn element_mut(&mut self) -> Option<&mut ElementData> {
        Some(&mut self.element)
    }
    fn is_shadow_root(&self) -> bool {
        true
    }
}

fn build_editor() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        nodes: vec![NodeClass::of::<SlotContainerNode>()],
        ..LexicalExtension::<(), (), ()>::new("[slot-parse-editor-state]", ())
    })
    .into()])
    .unwrap()
}

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

fn seed_slot_document(editor: &LexicalEditorWithDispose) {
    editor
        .update(
            || {
                let host = create_paragraph_node();
                let container = create::<SlotContainerNode>();
                let paragraph = create_paragraph_node();
                paragraph.append(&[create_text_node("Title")])?;
                container.append(&[paragraph])?;
                set_slot(&host, "title", &container)?;
                get_root().clear()?;
                get_root().append(&[host])
            },
            discrete(),
        )
        .unwrap();
}

fn to_json(editor: &LexicalEditorWithDispose) -> String {
    editor.get_editor_state().to_json(false).to_string()
}

// upstream: lexical/src/__tests__/unit/SlotParseEditorState.test.ts::_slotsUsed survives parseEditorState > a state built by update() carries the flag
#[test]
fn a_state_built_by_update_carries_the_flag() {
    let editor = build_editor();
    seed_slot_document(&editor);

    assert!(editor.0.slots_used.get());
    assert!(editor.get_editor_state().0.slots_used.get());
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/SlotParseEditorState.test.ts::_slotsUsed survives parseEditorState > a parsed state carries the flag
#[test]
fn a_parsed_state_carries_the_flag() {
    let source = build_editor();
    seed_slot_document(&source);
    let json = to_json(&source);

    let target = build_editor();
    let parsed = target.parse_editor_state(&json).unwrap();

    // The premise: the slot really did survive serialization.
    parsed.read(|| {
        let host = get_root().get_first_child_or_throw().unwrap();
        assert!(host.get_text_content().contains("Title"));
    });

    assert!(parsed.0.slots_used.get());
    source.dispose();
    target.dispose();
}

// upstream: lexical/src/__tests__/unit/SlotParseEditorState.test.ts::_slotsUsed survives parseEditorState > setEditorState latches the flag on an editor that did not parse it
#[test]
fn set_editor_state_latches_the_flag_on_an_editor_that_did_not_parse_it() {
    let source = build_editor();
    seed_slot_document(&source);
    let json = to_json(&source);

    // One editor parses, another receives the state.
    let parser = build_editor();
    let parsed = parser.parse_editor_state(&json).unwrap();

    let target = build_editor();
    assert!(!target.0.slots_used.get());
    target.set_editor_state(&parsed, None).unwrap();

    assert!(target.0.slots_used.get());
    for editor in [source, parser, target] {
        editor.dispose();
    }
}

// upstream: lexical/src/__tests__/unit/SlotParseEditorState.test.ts::_slotsUsed survives parseEditorState > parsing a slotless document leaves the flag alone
#[test]
fn parsing_a_slotless_document_leaves_the_flag_alone() {
    let source = build_editor();
    source
        .update(
            || {
                get_root().clear()?;
                let paragraph = create_paragraph_node();
                paragraph.append(&[create_text_node("plain")])?;
                get_root().append(&[paragraph])
            },
            discrete(),
        )
        .unwrap();
    let json = to_json(&source);

    let target = build_editor();
    let parsed = target.parse_editor_state(&json).unwrap();

    assert!(!parsed.0.slots_used.get());
    target.set_editor_state(&parsed, None).unwrap();
    assert!(!target.0.slots_used.get());
    source.dispose();
    target.dispose();
}
