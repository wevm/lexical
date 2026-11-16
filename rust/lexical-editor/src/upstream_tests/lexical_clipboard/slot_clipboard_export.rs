//! Ports of
//! `packages/lexical-clipboard/src/__tests__/unit/SlotClipboardExport.test.ts`.

#![cfg(feature = "html")]

use crate::clipboard::{
    generate_json_from_selected_nodes, generate_nodes_from_serialized_nodes, insert_generated_nodes,
};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::html::generate_html_from_nodes;
use crate::json::{Object, Value};
use crate::nodes::element::ElementData;
use crate::upstream_tests::support::{panic_message, update, update_with};
use crate::{
    InitialEditorStateType, LexicalExtension, LexicalNode, NodeBase, NodeClass, NodeRef, PointType,
    Which, create, create_node_selection, create_paragraph_node, create_range_selection,
    create_text_node, define_extension, get_node_by_key, get_root, get_selection, get_slot,
    get_slot_host, get_slot_names, set_selection, set_slot,
};

macro_rules! shadow_root_node {
    ($name:ident, $type_:literal, $exclude:expr) => {
        #[derive(Clone, Debug, Default)]
        struct $name {
            base: NodeBase,
            element: ElementData,
        }

        impl LexicalNode for $name {
            fn get_type(&self) -> &'static str {
                $type_
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
            fn exclude_from_copy(&self, _destination: &str) -> bool {
                $exclude
            }
            fn create_dom(
                &self,
                _this: &NodeRef,
                _config: &crate::EditorConfig,
                _editor: &crate::Editor,
            ) -> Result<crate::dom::DomNode, crate::LexicalError> {
                Ok(crate::lexical_dom::get_document().create_element("div"))
            }
        }
    };
}

// A plain shadow-root slot value.
shadow_root_node!(PlainShadowRootNode, "plain_shadow_root", false);
// A shadow-root slot value that excludes itself from copy.
shadow_root_node!(ExcludedShadowRootNode, "excluded_shadow_root", true);
// A Card-shaped host.
shadow_root_node!(CardLikeNode, "card_like", false);

fn build(name: &str, nodes: Vec<NodeClass>) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        nodes,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

fn paragraph_of(children: &[NodeRef]) -> NodeRef {
    let paragraph = create_paragraph_node();
    paragraph.append(children).unwrap();
    paragraph
}

fn stringify(nodes: &[Object]) -> String {
    crate::json::stringify(&Value::Array(
        nodes.iter().cloned().map(Value::from).collect(),
    ))
    .unwrap()
}

/// `expect(() => $generateJSONFromSelectedNodes(editor, null)).toThrow(...)`.
fn assert_export_throws(editor: &LexicalEditorWithDispose) {
    let ed = (**editor).clone();
    editor
        .read(move || {
            let message = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                generate_json_from_selected_nodes(&ed, None)
            })) {
                Ok(Ok(_)) => panic!("expected the export to throw"),
                Ok(Err(error)) => error.to_string(),
                Err(payload) => panic_message(payload.as_ref()),
            };
            assert!(
                message.contains("did not serialize to exactly the slot value node"),
                "{message}"
            );
        })
        .unwrap();
}

// upstream: lexical-clipboard/src/__tests__/unit/SlotClipboardExport.test.ts::slot clipboard export > throws when a slot value is excluded from copy
#[test]
fn throws_when_a_slot_value_is_excluded_from_copy() {
    let editor = build(
        "[slot-exclude]",
        vec![NodeClass::of::<ExcludedShadowRootNode>()],
    );
    update(&editor, || {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "title", &create::<ExcludedShadowRootNode>())?;
        Ok(())
    });
    assert_export_throws(&editor);
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/SlotClipboardExport.test.ts::slot clipboard export > a host outside the selection does not gate the export on its slot
#[test]
fn a_host_outside_the_selection_does_not_gate_the_export_on_its_slot() {
    let editor = build(
        "[slot-exclude]",
        vec![NodeClass::of::<ExcludedShadowRootNode>()],
    );
    let before_key = update_with(&editor, || {
        let before = create_text_node("BEFORE");
        get_root().append(&[paragraph_of(std::slice::from_ref(&before))])?;
        let host = paragraph_of(&[create_text_node("CHILD")]);
        set_slot(&host, "title", &create::<ExcludedShadowRootNode>())?;
        get_root().append(&[host])?;
        Ok(before.get_key().to_owned())
    });
    update(&editor, move || {
        let before = get_node_by_key(&before_key).unwrap();
        assert!(before.is_text());
        before.select(Some(0), Some(6))?;
        Ok(())
    });
    let ed = (*editor).clone();
    editor
        .read(move || {
            // The selection covers only "BEFORE": no throw.
            generate_json_from_selected_nodes(&ed, get_selection().as_ref()).unwrap();
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/SlotClipboardExport.test.ts::slot clipboard export > a slot-bearing host round-trips through JSON copy + insert
#[test]
fn a_slot_bearing_host_round_trips_through_json_copy_and_insert() {
    let editor = build(
        "[slot-roundtrip]",
        vec![NodeClass::of::<PlainShadowRootNode>()],
    );
    update(&editor, || {
        let host = paragraph_of(&[create_text_node("HostChild")]);
        let slot = create::<PlainShadowRootNode>();
        slot.append(&[paragraph_of(&[create_text_node("SlotText")])])?;
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "media", &slot)?;
        Ok(())
    });
    // Copy: null selection → whole tree.
    let ed = (*editor).clone();
    let serialized = editor
        .read(move || generate_json_from_selected_nodes(&ed, None).unwrap())
        .unwrap();
    assert_eq!(serialized.nodes.len(), 1);
    let slots = serialized.nodes[0]
        .get("$slots")
        .and_then(Value::as_object)
        .expect("$slots");
    assert!(slots.get("media").is_some());

    // Paste into a fresh editor and verify the slot survived.
    let editor2 = build(
        "[slot-roundtrip]",
        vec![NodeClass::of::<PlainShadowRootNode>()],
    );
    let ed2 = (*editor2).clone();
    let nodes: Vec<Value> = serialized.nodes.into_iter().map(Value::from).collect();
    update(&editor2, move || {
        let target = create_paragraph_node();
        get_root().append(std::slice::from_ref(&target))?;
        target.select(None, None)?;
        let nodes = generate_nodes_from_serialized_nodes(&nodes)?;
        insert_generated_nodes(&ed2, nodes, &get_selection().unwrap())
    });
    editor2
        .read(|| {
            let inserted = get_root()
                .get_children()
                .into_iter()
                .find(|n| !get_slot_names(n).is_empty())
                .expect("a host");
            assert_eq!(get_slot_names(&inserted), ["media"]);
            let slot = get_slot(&inserted, "media").expect("the slot");
            assert!(slot.get_text_content().contains("SlotText"));
            // The slot up-link points back to the inserted host.
            assert!(get_slot_host(&slot).unwrap() == inserted);
        })
        .unwrap();
    editor.dispose();
    editor2.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/SlotClipboardExport.test.ts::slot clipboard export > a selection inside a slot exports its content on both channels
#[test]
fn a_selection_inside_a_slot_exports_its_content_on_both_channels() {
    let editor = build(
        "[slot-inner-copy]",
        vec![NodeClass::of::<PlainShadowRootNode>()],
    );
    let slot_text_key = update_with(&editor, || {
        let host = paragraph_of(&[create_text_node("HostChild")]);
        let slot = create::<PlainShadowRootNode>();
        let slot_text = create_text_node("SlotText");
        slot.append(&[paragraph_of(std::slice::from_ref(&slot_text))])?;
        get_root().append(std::slice::from_ref(&host))?;
        set_slot(&host, "media", &slot)?;
        Ok(slot_text.get_key().to_owned())
    });
    update(&editor, move || {
        let slot_text = get_node_by_key(&slot_text_key).unwrap();
        assert!(slot_text.is_text());
        slot_text.select(Some(0), Some("SlotText".len()))?;
        Ok(())
    });
    let ed = (*editor).clone();
    editor
        .read(move || {
            let selection = get_selection();
            let json = stringify(
                &generate_json_from_selected_nodes(&ed, selection.as_ref())
                    .unwrap()
                    .nodes,
            );
            assert!(json.contains("SlotText"));
            // The host (and its unselected child) stays out of an in-slot copy.
            assert!(!json.contains("HostChild"));
            let html = generate_html_from_nodes(&ed, selection.as_ref()).unwrap();
            assert!(html.contains("SlotText"));
            assert!(!html.contains("HostChild"));
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/SlotClipboardExport.test.ts::slot clipboard export > a partial range over an element host does not over-export
#[test]
fn a_partial_range_over_an_element_host_does_not_over_export() {
    let editor = build(
        "[slot-partial-range]",
        vec![
            NodeClass::of::<CardLikeNode>(),
            NodeClass::of::<PlainShadowRootNode>(),
        ],
    );
    let (intro_key, body_key) = update_with(&editor, || {
        let intro = create_text_node("Intro");
        get_root().append(&[paragraph_of(std::slice::from_ref(&intro))])?;
        let card = create::<CardLikeNode>();
        let body_text = create_text_node("Body");
        card.append(&[paragraph_of(std::slice::from_ref(&body_text))])?;
        card.append(&[paragraph_of(&[create_text_node("UNSELECTED")])])?;
        let title = create::<PlainShadowRootNode>();
        title.append(&[paragraph_of(&[create_text_node("Title")])])?;
        get_root().append(std::slice::from_ref(&card))?;
        set_slot(&card, "title", &title)?;
        Ok((intro.get_key().to_owned(), body_text.get_key().to_owned()))
    });
    update(&editor, move || {
        let selection = create_range_selection();
        selection.point_set(Which::Anchor, &intro_key, 0, PointType::Text)?;
        selection.point_set(Which::Focus, &body_key, 2, PointType::Text)?;
        set_selection(Some(selection));
        Ok(())
    });
    let ed = (*editor).clone();
    editor
        .read(move || {
            let selection = get_selection();
            let json = stringify(
                &generate_json_from_selected_nodes(&ed, selection.as_ref())
                    .unwrap()
                    .nodes,
            );
            assert!(json.contains("Intro"));
            assert!(json.contains("\"Bo\""));
            assert!(!json.contains("\"Body\""));
            assert!(!json.contains("UNSELECTED"));
            let html = generate_html_from_nodes(&ed, selection.as_ref()).unwrap();
            assert!(html.contains("Bo"));
            assert!(!html.contains("Body"));
            assert!(!html.contains("UNSELECTED"));
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/SlotClipboardExport.test.ts::slot clipboard export > a NodeSelection on a node inside a slot exports the node, not an empty list
#[test]
fn a_node_selection_on_a_node_inside_a_slot_exports_the_node() {
    let editor = build(
        "[slot-node-selection]",
        vec![NodeClass::of::<PlainShadowRootNode>()],
    );
    let inner_para_key = update_with(&editor, || {
        let host = create_paragraph_node();
        let slot = create::<PlainShadowRootNode>();
        let inner_para = paragraph_of(&[create_text_node("SlotNodeTarget")]);
        slot.append(std::slice::from_ref(&inner_para))?;
        set_slot(&host, "media", &slot)?;
        get_root().append(&[host])?;
        Ok(inner_para.get_key().to_owned())
    });
    update(&editor, move || {
        let inner_para = get_node_by_key(&inner_para_key).unwrap();
        let node_selection = create_node_selection();
        node_selection.add(inner_para.get_key());
        set_selection(Some(node_selection));
        Ok(())
    });
    let ed = (*editor).clone();
    editor
        .read(move || {
            let json = generate_json_from_selected_nodes(&ed, get_selection().as_ref()).unwrap();
            assert!(!json.nodes.is_empty());
            assert!(stringify(&json.nodes).contains("SlotNodeTarget"));
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/SlotClipboardExport.test.ts::slot clipboard export > throws when a 1-child excluded slot value would export its child instead
#[test]
fn throws_when_a_1_child_excluded_slot_value_would_export_its_child_instead() {
    let editor = build(
        "[slot-exclude-one-child]",
        vec![NodeClass::of::<ExcludedShadowRootNode>()],
    );
    update(&editor, || {
        let host = create_paragraph_node();
        get_root().append(std::slice::from_ref(&host))?;
        let slot = create::<ExcludedShadowRootNode>();
        slot.append(&[paragraph_of(&[create_text_node("inner")])])?;
        set_slot(&host, "title", &slot)?;
        Ok(())
    });
    assert_export_throws(&editor);
    editor.dispose();
}
