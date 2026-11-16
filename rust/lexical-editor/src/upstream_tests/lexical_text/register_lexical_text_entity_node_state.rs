//! Ports of `packages/lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts`.

use super::support::{discrete, entity_class, regex_match};
use crate::extension::build_editor_from_extensions;
use crate::hashtag::{HashtagNode, create_hashtag_node, get_hashtag_match};
use crate::mark::{MarkNode, create_mark_node, is_mark_node};
use crate::nodes::text::TextNode;
use crate::text::{EntityMatch, register_lexical_text_entity};
use crate::upstream_tests::support::{json_to_equal, mount_in_jsdom};
use crate::{
    CreateEditorArgs, Editor, HISTORY_PUSH_TAG, LexicalError, LexicalExtension, LexicalNode,
    LexicalNodeReplacement, NodeBase, NodeClass, NodeRef, PointType, REDO_COMMAND,
    RequiredNodeStateConfig, StateConfig, StateValue, StateValueConfig, TextData, UNDO_COMMAND,
    UpdateOptions, Which, apply_node_replacement, create_editor, create_paragraph_node,
    create_range_selection, create_state, create_state_with, create_text_node, define_extension,
    get_root, get_selection, get_state, json, set_selection, set_state,
};
use std::rc::Rc;

fn string_state(key: &str) -> StateConfig<String> {
    create_state(key, |v: &json::Value| {
        v.as_str().map(str::to_owned).unwrap_or_default()
    })
}

thread_local! {
    // Ad-hoc NodeState, as an app keeps comment ids or block ids on text.
    static NOTE: StateConfig<String> = string_state("note");
    // Reference-typed state: the carried value is the same object.
    static IDS: StateConfig<Ids> = create_state_with(
        "ids",
        StateValueConfig {
            is_equal: Rc::new(|a: &Ids, b: &Ids| a.0 == b.0),
            ..StateValueConfig::new(|v: &json::Value| {
                Ids(Rc::new(match v {
                    json::Value::Array(xs) => {
                        xs.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()
                    }
                    _ => Vec::new(),
                }))
            })
        },
    );
    // An entity whose own $config declares a state (nested and flat).
    static ENTITY_OWN: StateConfig<String> = string_state("entityOwn");
    static ENTITY_FLAT: StateConfig<String> = string_state("entityFlat");
}

fn note() -> StateConfig<String> {
    NOTE.with(Clone::clone)
}
fn ids() -> StateConfig<Ids> {
    IDS.with(Clone::clone)
}

/// A `readonly string[]` value: an object, compared by identity.
#[derive(Clone, Debug)]
struct Ids(Rc<Vec<String>>);

impl StateValue for Ids {
    fn to_json(&self) -> json::Value {
        json::Value::Array(self.0.iter().cloned().map(json::Value::String).collect())
    }
    fn object_is(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

// A keyword-style entity (normal mode) and a token entity (mention-like).
entity_class!(KeywordNode, "test-keyword");
entity_class!(MentionNode, "test-mention");

fn create_keyword_node(text: &str) -> NodeRef {
    apply_node_replacement(&KeywordNode::new_node(text)).unwrap()
}

fn get_keyword_match(text: &str) -> Option<EntityMatch> {
    thread_local! {
        static RE: crate::markdown::RegExp =
            crate::markdown::RegExp::new("(^|[^A-Za-z])(cat)(?![A-Za-z])", "").unwrap();
    }
    RE.with(|re| {
        let m = re.exec(text)?;
        let start = m.index + crate::utf16::len(m.str(1));
        Some(EntityMatch {
            start,
            end: start + 3,
        })
    })
}

fn create_mention_node(text: &str) -> NodeRef {
    let node = apply_node_replacement(&MentionNode::new_node(text)).unwrap();
    node.set_mode("token").unwrap();
    node
}

/// `StatefulEntityNode` (`test-stateful-entity`): `stateConfigs:
/// [{flat: true, stateConfig: entityFlat}, entityOwn]`.
#[derive(Clone, Debug, Default)]
struct StatefulEntityNode {
    base: NodeBase,
    text: TextData,
}

impl LexicalNode for StatefulEntityNode {
    fn get_type(&self) -> &'static str {
        "test-stateful-entity"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn text(&self) -> Option<&TextData> {
        Some(&self.text)
    }
    fn text_mut(&mut self) -> Option<&mut TextData> {
        Some(&mut self.text)
    }
    fn is_text_entity(&self) -> bool {
        true
    }
    fn state_configs(&self) -> Vec<RequiredNodeStateConfig> {
        vec![
            ENTITY_FLAT.with(|c| RequiredNodeStateConfig::new(c, true)),
            ENTITY_OWN.with(|c| RequiredNodeStateConfig::new(c, false)),
        ]
    }
}

fn new_text_node<T: LexicalNode + Default>(
    text: &str,
    make: impl FnOnce(TextData) -> T,
) -> NodeRef {
    crate::lexical_utils::set_node_key(Box::new(make(TextData {
        text: text.to_owned(),
        ..TextData::default()
    })))
}

fn new_stateful_entity_node(text: &str) -> NodeRef {
    new_text_node(text, |text| StatefulEntityNode {
        text,
        ..StatefulEntityNode::default()
    })
}

type GetMatch = Box<dyn Fn(&str) -> Option<EntityMatch>>;

fn get_hash_match(text: &str) -> Option<EntityMatch> {
    thread_local! {
        static M: GetMatch = Box::new(regex_match(r"#\w+", ""));
    }
    M.with(|m| m(text))
}

fn editor_with(nodes: Vec<NodeClass>) -> Editor {
    let editor = create_editor(CreateEditorArgs {
        nodes,
        ..CreateEditorArgs::default()
    });
    mount_in_jsdom(&editor);
    editor
}

fn create_hashtag_editor(nodes: Vec<NodeClass>) -> Editor {
    let mut all = vec![NodeClass::of::<HashtagNode>()];
    all.extend(nodes);
    let editor = editor_with(all);
    let _ = register_lexical_text_entity(&editor, get_hashtag_match, "hashtag", |node| {
        create_hashtag_node(&node.get_text_content())
    });
    editor
}

type Row = (String, String, String);

fn row(t: &str, text: &str, note: &str) -> Row {
    (t.to_owned(), text.to_owned(), note.to_owned())
}

/// `[type, text, note]` of every text node.
fn read(editor: &Editor) -> Vec<Row> {
    let note = note();
    editor
        .read(|| {
            get_root()
                .get_all_text_nodes()
                .iter()
                .map(|n| {
                    (
                        n.get_type().to_owned(),
                        n.get_text_content(),
                        get_state(n, &note).unwrap(),
                    )
                })
                .collect()
        })
        .unwrap()
}

/// `update(editor, fn)`: `fn`, then `$setSelection(null)`, discrete.
fn update(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    editor
        .update(
            || {
                f()?;
                set_selection(None);
                Ok(())
            },
            discrete(),
        )
        .unwrap();
}

fn paragraph(children: &[NodeRef]) -> NodeRef {
    let paragraph = create_paragraph_node();
    paragraph.append(children).unwrap();
    paragraph
}

fn text_at(i: usize) -> NodeRef {
    get_root().get_all_text_nodes()[i].clone()
}

fn with_note(node: NodeRef, value: &str) -> NodeRef {
    set_state(&node, &note(), value.to_owned()).unwrap()
}

fn formatted(text: &str, format: &str) -> NodeRef {
    let node = create_text_node(text);
    node.set_format_type(format);
    node
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > #9703: a hashtag that stops matching keeps its NodeState when it becomes text (typing before it)
#[test]
fn a_hashtag_that_stops_matching_keeps_its_node_state_when_it_becomes_text() {
    let editor = create_hashtag_editor(vec![]);
    update(&editor, || {
        get_root().append(&[paragraph(&[
            create_text_node("one "),
            with_note(create_hashtag_node("#tag"), "c1"),
            create_text_node(" more"),
        ])])
    });
    update(&editor, || {
        text_at(1).select(Some(0), Some(0))?.insert_text("x")
    });
    assert_eq!(
        read(&editor),
        [
            row("text", "one x", ""),
            row("text", "#tag", "c1"),
            row("text", " more", "")
        ]
    );
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > deleting the # of a hashtag keeps its NodeState (reverse transform, no match)
#[test]
fn deleting_the_hash_of_a_hashtag_keeps_its_node_state() {
    let editor = create_hashtag_editor(vec![]);
    update(&editor, || {
        get_root().append(&[paragraph(&[
            create_text_node("one "),
            with_note(create_hashtag_node("#tag"), "c1"),
        ])])
    });
    update(&editor, || {
        text_at(1).select(Some(0), Some(1))?.remove_text()
    });
    assert_eq!(
        read(&editor),
        [row("text", "one ", ""), row("text", "tag", "c1")]
    );
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > a hashtag that becomes text merges into neighbors with the same NodeState
#[test]
fn a_hashtag_that_becomes_text_merges_into_neighbors_with_the_same_node_state() {
    let editor = create_hashtag_editor(vec![]);
    update(&editor, || {
        get_root().append(&[paragraph(&[
            with_note(create_text_node("one "), "c1"),
            with_note(create_hashtag_node("#tag"), "c1"),
            with_note(create_text_node(" more"), "c1"),
        ])])
    });
    update(&editor, || {
        text_at(1).select(Some(0), Some(0))?.insert_text("x")
    });
    assert_eq!(read(&editor), [row("text", "one x#tag more", "c1")]);
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > a hashtag with no NodeState becomes text and merges as before
#[test]
fn a_hashtag_with_no_node_state_becomes_text_and_merges_as_before() {
    let editor = create_hashtag_editor(vec![]);
    update(&editor, || {
        get_root().append(&[paragraph(&[
            create_text_node("one "),
            create_hashtag_node("#tag"),
            create_text_node(" more"),
        ])])
    });
    update(&editor, || {
        text_at(1).select(Some(0), Some(0))?.insert_text("x")
    });
    assert_eq!(read(&editor), [row("text", "one x#tag more", "")]);
    // No NodeState was created on the merged text
    editor.get_editor_state().read(|| {
        assert!(!text_at(0).export_json(false).contains_key("$"));
    });
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > plain text with NodeState that becomes a hashtag keeps it
#[test]
fn plain_text_with_node_state_that_becomes_a_hashtag_keeps_it() {
    let editor = create_hashtag_editor(vec![]);
    update(&editor, || {
        get_root().append(&[paragraph(&[with_note(create_text_node("a #"), "c1")])])
    });
    // The "#" was there: the hashtag is not only inserted text
    update(&editor, || {
        text_at(0).select(Some(3), Some(3))?.insert_text("tag")
    });
    assert_eq!(
        read(&editor),
        [row("text", "a ", "c1"), row("hashtag", "#tag", "c1")]
    );
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > a hashtag made only of text inserted into a node does not take its NodeState
#[test]
fn a_hashtag_made_only_of_text_inserted_into_a_node_does_not_take_its_node_state() {
    // The inserted characters land in the node and get a copy of its
    // NodeState, which is not theirs (as on main; MarkStateExtension does not
    // add text inserted at a comment's edge to the comment either)
    let cases = [
        (
            1,
            " #tag",
            [row("text", "a ", "c1"), row("hashtag", "#tag", "")],
        ),
        (
            0,
            "#tag ",
            [row("hashtag", "#tag", ""), row("text", " a", "c1")],
        ),
    ];
    for (at, inserted, expected) in cases {
        let editor = create_hashtag_editor(vec![]);
        update(&editor, || {
            get_root().append(&[paragraph(&[with_note(create_text_node("a"), "c1")])])
        });
        update(&editor, move || {
            text_at(0).select(Some(at), Some(at))?.insert_text(inserted)
        });
        assert_eq!(read(&editor), expected);
    }
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > state that createNode gives the entity wins over the text it replaces
#[test]
fn state_that_create_node_gives_the_entity_wins_over_the_text_it_replaces() {
    let editor = editor_with(vec![NodeClass::of::<HashtagNode>()]);
    let _ = register_lexical_text_entity(&editor, get_hashtag_match, "hashtag", |node| {
        with_note(create_hashtag_node(&node.get_text_content()), "entity")
    });
    update(&editor, || {
        let text = with_note(create_text_node("a #tag"), "text");
        let text = set_state(&text, &ids(), Ids(Rc::new(vec!["c1".to_owned()])))?;
        get_root().append(&[paragraph(&[text])])
    });
    editor.get_editor_state().read(|| {
        let nodes = get_root().get_all_text_nodes();
        let (text, hashtag) = (&nodes[0], &nodes[1]);
        assert_eq!(get_state(text, &note()).unwrap(), "text");
        assert_eq!(hashtag.get_type(), "hashtag");
        assert_eq!(get_state(hashtag, &note()).unwrap(), "entity");
        assert_eq!(*get_state(hashtag, &ids()).unwrap().0, ["c1"]);
    });
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > text inserted after a hashtag that becomes a hashtag does not take the NodeState copied from it
#[test]
fn text_inserted_after_a_hashtag_that_becomes_a_hashtag_does_not_take_the_node_state_copied_from_it()
 {
    // The hashtag's transform splits the inserted text off with a copy of
    // its NodeState (as before); a hashtag made from that text does not
    // take that copy, which is not the inserted text's own.
    let editor = create_hashtag_editor(vec![]);
    update(&editor, || {
        get_root().append(&[paragraph(&[
            create_text_node("see "),
            with_note(create_hashtag_node("#foo"), "c1"),
        ])])
    });
    update(&editor, || {
        text_at(1)
            .select(Some(4), Some(4))?
            .insert_text(", x #bar y")
    });
    let rows = read(&editor);
    assert_eq!(rows[1], row("hashtag", "#foo", "c1"));
    assert_eq!(
        rows.iter().find(|r| r.1 == "#bar"),
        Some(&row("hashtag", "#bar", ""))
    );
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > a reference-typed state value is carried as the same object
#[test]
fn a_reference_typed_state_value_is_carried_as_the_same_object() {
    let editor = create_hashtag_editor(vec![]);
    let value = Ids(Rc::new(vec!["c1".to_owned(), "c2".to_owned()]));
    let v = value.clone();
    update(&editor, move || {
        let hashtag = set_state(&create_hashtag_node("#tag"), &ids(), v)?;
        get_root().append(&[paragraph(&[create_text_node("one "), hashtag])])
    });
    update(&editor, || {
        text_at(1).select(Some(0), Some(1))?.remove_text()
    });
    editor.get_editor_state().read(|| {
        let node = text_at(1);
        assert_eq!(node.get_type(), "text");
        assert!(Rc::ptr_eq(&get_state(&node, &ids()).unwrap().0, &value.0));
    });
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > NodeState read from JSON (not parsed yet) is carried and exported
#[test]
fn node_state_read_from_json_is_carried_and_exported() {
    let editor = create_hashtag_editor(vec![]);
    let state = editor
        .parse_editor_state(
            r##"{"root":{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"one ","type":"text","version":1},{"$":{"comment":["c1"],"note":"n1"},"detail":0,"format":0,"mode":"normal","style":"","text":"#tag","type":"hashtag","version":1}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"##,
        )
        .unwrap();
    editor.set_editor_state(&state, None).unwrap();
    update(&editor, || {
        text_at(1).select(Some(0), Some(0))?.insert_text("x")
    });
    editor.get_editor_state().read(|| {
        let json = text_at(1).export_json(false);
        assert_eq!(json.get("type"), Some(&json::Value::String("text".into())));
        assert_eq!(json.get("text"), Some(&json::Value::String("#tag".into())));
        assert!(json_to_equal(
            json.get("$").unwrap(),
            &json::parse(r#"{"comment":["c1"],"note":"n1"}"#).unwrap()
        ));
        assert_eq!(get_state(&text_at(1), &note()).unwrap(), "n1");
    });
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > NodeState declared in the entity $config (nested and flat) is carried to the text
#[test]
fn node_state_declared_in_the_entity_config_is_carried_to_the_text() {
    let editor = editor_with(vec![NodeClass::of::<StatefulEntityNode>()]);
    let _ = register_lexical_text_entity(&editor, get_hash_match, "test-stateful-entity", |node| {
        apply_node_replacement(&new_stateful_entity_node(&node.get_text_content())).unwrap()
    });
    let own = ENTITY_OWN.with(Clone::clone);
    let flat = ENTITY_FLAT.with(Clone::clone);
    let (o, f) = (own.clone(), flat.clone());
    update(&editor, move || {
        let entity = new_stateful_entity_node("#tag");
        set_state(&entity, &o, "own".to_owned())?;
        set_state(&entity, &f, "flat".to_owned())?;
        let entity = with_note(entity, "c1");
        get_root().append(&[paragraph(&[create_text_node("one "), entity])])
    });
    update(&editor, || {
        text_at(1).select(Some(0), Some(1))?.remove_text()
    });
    editor.get_editor_state().read(|| {
        let node = text_at(1);
        assert_eq!(node.get_type(), "text");
        assert_eq!(get_state(&node, &note()).unwrap(), "c1");
        assert_eq!(get_state(&node, &own).unwrap(), "own");
        assert_eq!(get_state(&node, &flat).unwrap(), "flat");
        // TextNode does not declare `entityFlat` flat: it serializes nested
        let json = node.export_json(false);
        let state = json.get("$").and_then(json::Value::as_object).unwrap();
        for (k, v) in [("entityFlat", "flat"), ("entityOwn", "own"), ("note", "c1")] {
            assert_eq!(state.get(k), Some(&json::Value::String(v.into())), "{k}");
        }
    });
    // Exported and imported again, the text keeps it
    let json = editor.get_editor_state().to_json(false);
    let state = editor.parse_editor_state_json(&json).unwrap();
    editor.set_editor_state(&state, None).unwrap();
    editor.get_editor_state().read(|| {
        let node = text_at(1);
        assert_eq!(get_state(&node, &own).unwrap(), "own");
        assert_eq!(get_state(&node, &flat).unwrap(), "flat");
    });
}

thread_local! {
    // The entity and the text use different StateConfigs for the key "tag"
    static ENTITY_TAG: StateConfig<String> = string_state("tag");
    static TEXT_TAG: StateConfig<String> = create_state_with(
        "tag",
        StateValueConfig {
            unparse: Rc::new(|v: &String| {
                json::Value::String(v.strip_prefix("text:").unwrap_or(v).to_owned())
            }),
            ..StateValueConfig::new(|v: &json::Value| {
                v.as_str().map(|s| format!("text:{s}")).unwrap_or_default()
            })
        },
    );
}

/// `TagEntityNode` (`test-tag-entity`): `stateConfigs: [entityTag]`.
#[derive(Clone, Debug, Default)]
struct TagEntityNode {
    base: NodeBase,
    text: TextData,
}

impl LexicalNode for TagEntityNode {
    fn get_type(&self) -> &'static str {
        "test-tag-entity"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn text(&self) -> Option<&TextData> {
        Some(&self.text)
    }
    fn text_mut(&mut self) -> Option<&mut TextData> {
        Some(&mut self.text)
    }
    fn is_text_entity(&self) -> bool {
        true
    }
    fn state_configs(&self) -> Vec<RequiredNodeStateConfig> {
        vec![ENTITY_TAG.with(|c| RequiredNodeStateConfig::new(c, false))]
    }
}

/// `TagTextNode` (`test-tag-text`): `stateConfigs: [textTag]`.
#[derive(Clone, Debug, Default)]
struct TagTextNode {
    base: NodeBase,
    text: TextData,
}

impl LexicalNode for TagTextNode {
    fn get_type(&self) -> &'static str {
        "test-tag-text"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn text(&self) -> Option<&TextData> {
        Some(&self.text)
    }
    fn text_mut(&mut self) -> Option<&mut TextData> {
        Some(&mut self.text)
    }
    fn super_types(&self) -> Vec<&'static str> {
        vec!["text"]
    }
    fn state_configs(&self) -> Vec<RequiredNodeStateConfig> {
        vec![TEXT_TAG.with(|c| RequiredNodeStateConfig::new(c, false))]
    }
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > a key the plain text type declares with another config is parsed with that config
#[test]
fn a_key_the_plain_text_type_declares_with_another_config_is_parsed_with_that_config() {
    let editor = editor_with(vec![
        NodeClass::of::<TagEntityNode>(),
        NodeClass::of::<TagTextNode>(),
        LexicalNodeReplacement {
            replace: NodeClass::of::<TextNode>(),
            with: Rc::new(|node: &NodeRef| {
                Ok(new_text_node(node.text_data().get_text(), |text| {
                    TagTextNode {
                        text,
                        ..TagTextNode::default()
                    }
                }))
            }),
            with_klass: Some(NodeClass::of::<TagTextNode>()),
        }
        .into(),
    ]);
    let new_tag_entity = |text: &str| {
        new_text_node(text, |text| TagEntityNode {
            text,
            ..TagEntityNode::default()
        })
    };
    let _ = register_lexical_text_entity(&editor, get_hash_match, "test-tag-entity", move |node| {
        apply_node_replacement(&new_tag_entity(&node.get_text_content())).unwrap()
    });
    let entity_tag = ENTITY_TAG.with(Clone::clone);
    let text_tag = TEXT_TAG.with(Clone::clone);
    update(&editor, move || {
        let entity = new_tag_entity("#tag");
        set_state(&entity, &entity_tag, "v".to_owned())?;
        get_root().append(&[paragraph(&[create_text_node("one "), entity])])
    });
    update(&editor, || {
        get_root().get_all_text_nodes()[1]
            .select(Some(0), Some(1))?
            .remove_text()
    });
    editor.get_editor_state().read(|| {
        let node = get_root().get_all_text_nodes()[1].clone();
        assert_eq!(node.get_type(), "test-tag-text");
        assert_eq!(get_state(&node, &text_tag).unwrap(), "text:v");
    });
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > a hashtag inside a MarkNode stays inside it when it becomes text
#[test]
fn a_hashtag_inside_a_mark_node_stays_inside_it_when_it_becomes_text() {
    let editor = create_hashtag_editor(vec![NodeClass::of::<MarkNode>()]);
    update(&editor, || {
        let mark = create_mark_node(&["c1".to_owned()]);
        mark.append(&[with_note(create_hashtag_node("#tag"), "n1")])?;
        get_root().append(&[paragraph(&[
            create_text_node("one "),
            mark,
            create_text_node(" more"),
        ])])
    });
    update(&editor, || {
        let hashtag = get_root().get_all_text_nodes()[1].clone();
        hashtag.select(Some(0), Some(1))?.remove_text()
    });
    editor.get_editor_state().read(|| {
        let node = get_root().get_all_text_nodes()[1].clone();
        assert_eq!(node.get_type(), "text");
        assert_eq!(node.get_text_content(), "tag");
        assert_eq!(get_state(&node, &note()).unwrap(), "n1");
        let mark = node.get_parent().unwrap();
        assert!(is_mark_node(&mark));
        assert_eq!(mark.get_ids(), Some(vec!["c1".to_owned()]));
    });
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::registerLexicalTextEntity keeps NodeState > undo and redo of a demotion restore the entity and the text with their NodeState
#[test]
fn undo_and_redo_of_a_demotion_restore_the_entity_and_the_text_with_their_node_state() {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![
            crate::hashtag::hashtag_extension().into(),
            crate::history::history_extension().into(),
        ],
        ..LexicalExtension::<(), (), ()>::new("test", ())
    })
    .into()])
    .unwrap();
    mount_in_jsdom(&editor);
    let push = || UpdateOptions {
        discrete: true,
        tags: vec![HISTORY_PUSH_TAG.to_owned()],
        ..UpdateOptions::default()
    };
    editor
        .update(
            || {
                get_root().clear()?;
                get_root().append(&[paragraph(&[
                    create_text_node("one "),
                    with_note(create_hashtag_node("#tag"), "c1"),
                ])])?;
                text_at(0).select(Some(0), Some(0))?;
                Ok(())
            },
            push(),
        )
        .unwrap();
    let before = read(&editor);
    editor
        .update(
            || text_at(1).select(Some(0), Some(0))?.insert_text("x"),
            push(),
        )
        .unwrap();
    let after = read(&editor);
    assert_eq!(after, [row("text", "one x", ""), row("text", "#tag", "c1")]);
    editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
    assert_eq!(read(&editor), before);
    editor.dispatch_command(&REDO_COMMAND, ()).unwrap();
    assert_eq!(read(&editor), after);
    editor.dispose();
}

// every path that turns an entity into text keeps NodeState

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::every path that turns an entity into text keeps NodeState > the text transform: the entity before it stops matching with its text (prevMatch null)
#[test]
fn the_text_transform_the_entity_before_it_stops_matching_with_its_text() {
    // A keyword followed by a letter in another node: "cat" + "s" is no
    // keyword, so the text transform of "s" turns the keyword into text.
    let editor = editor_with(vec![NodeClass::of::<KeywordNode>()]);
    let _ = register_lexical_text_entity(&editor, get_keyword_match, "test-keyword", |node| {
        create_keyword_node(&node.get_text_content())
    });
    update(&editor, || {
        get_root().append(&[paragraph(&[
            create_text_node("a "),
            with_note(create_keyword_node("cat"), "k1"),
            create_text_node(" b"),
        ])])
    });
    update(&editor, || {
        text_at(2).select(Some(0), Some(0))?.insert_text("s")
    });
    assert_eq!(
        read(&editor),
        [
            row("text", "a ", ""),
            row("text", "cat", "k1"),
            row("text", "s b", "")
        ]
    );
}

fn create_mention_editor() -> Editor {
    let editor = editor_with(vec![NodeClass::of::<MentionNode>()]);
    let _ = register_lexical_text_entity(
        &editor,
        regex_match("@[a-z]+", ""),
        "test-mention",
        |node| create_mention_node(&node.get_text_content()),
    );
    editor
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::every path that turns an entity into text keeps NodeState > the text transform: a token entity (mention-like) that text follows > with the same NodeState as the text: it keeps it and takes the text
#[test]
fn a_token_entity_with_the_same_node_state_as_the_text_keeps_it_and_takes_the_text() {
    let editor = create_mention_editor();
    update(&editor, || {
        get_root().append(&[paragraph(&[
            create_text_node("hi "),
            with_note(create_mention_node("@bob"), "m1"),
            with_note(create_text_node("x"), "m1"),
        ])])
    });
    // Before this change, this looped forever: the entity became text
    // without its NodeState, which did not merge with "x".
    assert_eq!(
        read(&editor),
        [row("text", "hi ", ""), row("test-mention", "@bobx", "m1")]
    );
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::every path that turns an entity into text keeps NodeState > the text transform: a token entity (mention-like) that text follows > with other NodeState than the text: kept with its NodeState, the text stays apart
#[test]
fn a_token_entity_with_other_node_state_than_the_text_is_kept_and_the_text_stays_apart() {
    let editor = create_mention_editor();
    update(&editor, || {
        get_root().append(&[paragraph(&[
            create_text_node("hi "),
            with_note(create_mention_node("@bob"), "m1"),
            create_text_node("x"),
        ])])
    });
    assert_eq!(
        read(&editor),
        [
            row("text", "hi ", ""),
            row("test-mention", "@bob", "m1"),
            row("text", "x", "")
        ]
    );
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::every path that turns an entity into text keeps NodeState > the text transform: a token entity (mention-like) that text follows > typed text after it: the selection stays after the typed text
#[test]
fn typed_text_after_a_token_entity_the_selection_stays_after_the_typed_text() {
    let editor = create_mention_editor();
    update(&editor, || {
        get_root().append(&[paragraph(&[
            create_text_node("hi "),
            with_note(create_mention_node("@bob"), "m1"),
            create_text_node("!"),
        ])])
    });
    editor
        .update(
            // Paste "x yz" right after the mention
            || text_at(1).select(Some(4), Some(4))?.insert_text("x yz"),
            discrete(),
        )
        .unwrap();
    assert_eq!(
        read(&editor),
        [
            row("text", "hi ", ""),
            row("test-mention", "@bob", "m1"),
            row("text", "x yz!", "")
        ]
    );
    editor
        .read(|| {
            let selection = get_selection()
                .filter(crate::SelectionRef::is_range)
                .unwrap();
            let anchor = selection.point(Which::Anchor).unwrap();
            let focus = selection.point(Which::Focus).unwrap();
            assert_eq!(
                (anchor.get_node().unwrap().get_text_content(), anchor.offset),
                ("x yz!".to_owned(), 4)
            );
            assert_eq!(focus, anchor);
        })
        .unwrap();
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::every path that turns an entity into text keeps NodeState > the text transform: a token entity (mention-like) that text follows > with other NodeState than text that its match extends into, which keeps it apart: kept, each keeps its own
#[test]
fn a_token_entity_with_other_node_state_than_text_its_match_extends_into_each_keeps_its_own() {
    let editor = create_mention_editor();
    update(&editor, || {
        get_root().append(&[paragraph(&[
            create_text_node("hi "),
            with_note(create_mention_node("@bob"), "m1"),
            with_note(create_text_node("x"), "x1"),
        ])])
    });
    assert_eq!(
        read(&editor),
        [
            row("text", "hi ", ""),
            row("test-mention", "@bob", "m1"),
            row("text", "x", "x1")
        ]
    );
}

/// A `{id: string}` value: an object, compared by identity.
#[derive(Clone, Debug)]
struct Tag(Rc<String>);

impl StateValue for Tag {
    fn to_json(&self) -> json::Value {
        let mut o = json::Object::new();
        o.insert("id".to_owned(), json::Value::String((*self.0).clone()));
        json::Value::Object(o)
    }
    fn object_is(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::every path that turns an entity into text keeps NodeState > the text transform: a token entity (mention-like) that text follows > with a value object distinct from the text after it, equal as JSON: each keeps its own
#[test]
fn with_a_value_object_distinct_from_the_text_after_it_equal_as_json_each_keeps_its_own() {
    // Two value objects are two values unless their `isEqual` says
    // otherwise, as when text nodes merge: the token does not take "x".
    let tag = create_state("tag", |v: &json::Value| {
        Tag(Rc::new(
            v.as_object()
                .and_then(|o| o.get("id"))
                .and_then(json::Value::as_str)
                .unwrap_or("")
                .to_owned(),
        ))
    });
    let a = Tag(Rc::new("c1".to_owned()));
    let b = Tag(Rc::new("c1".to_owned()));
    let editor = create_mention_editor();
    let (t, a2, b2) = (tag.clone(), a.clone(), b.clone());
    update(&editor, move || {
        get_root().append(&[paragraph(&[
            create_text_node("hi "),
            set_state(&create_mention_node("@bob"), &t, a2)?,
            set_state(&create_text_node("x"), &t, b2)?,
        ])])
    });
    editor
        .read(|| {
            let nodes = get_root().get_all_text_nodes();
            assert_eq!(
                nodes
                    .iter()
                    .map(|n| (n.get_type(), n.get_text_content()))
                    .collect::<Vec<_>>(),
                [
                    ("text", "hi ".to_owned()),
                    ("test-mention", "@bob".to_owned()),
                    ("text", "x".to_owned())
                ]
            );
            assert!(Rc::ptr_eq(&get_state(&nodes[1], &tag).unwrap().0, &a.0));
            assert!(Rc::ptr_eq(&get_state(&nodes[2], &tag).unwrap().0, &b.0));
        })
        .unwrap();
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::every path that turns an entity into text keeps NodeState > the text transform: the entity after it stops matching (nextMatch null)
#[test]
fn the_text_transform_the_entity_after_it_stops_matching() {
    // "x" + "#tag": "x#tag" holds no hashtag, so the text transform of "x"
    // turns the hashtag after it into text.
    let editor = create_hashtag_editor(vec![]);
    update(&editor, || {
        get_root().append(&[paragraph(&[
            formatted("x", "bold"),
            with_note(create_hashtag_node("#tag"), "c1"),
        ])])
    });
    update(&editor, || {
        // Mark both dirty
        text_at(0).mark_dirty();
        text_at(1).mark_dirty();
        Ok(())
    });
    let rows = read(&editor);
    // On main's rules the hashtag either stays or becomes text; either way
    // its characters keep their NodeState.
    assert_eq!(
        rows.iter().find(|r| r.1 == "#tag").map(|r| r.2.as_str()),
        Some("c1")
    );
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::every path that turns an entity into text keeps NodeState > the reverse transform: adjacent entities both become text
#[test]
fn the_reverse_transform_adjacent_entities_both_become_text() {
    let editor = create_hashtag_editor(vec![]);
    update(&editor, || {
        get_root().append(&[paragraph(&[
            with_note(create_hashtag_node("#a"), "c1"),
            with_note(create_hashtag_node("#b"), "c2"),
        ])])
    });
    // Both become text, then the first is a hashtag again: each keeps its
    // NodeState (before, both lost it)
    assert_eq!(
        read(&editor),
        [row("hashtag", "#a", "c1"), row("text", "#b", "c2")]
    );
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::every path that turns an entity into text keeps NodeState > the reverse transform: an entity followed by an entity
#[test]
fn the_reverse_transform_an_entity_followed_by_an_entity() {
    let editor = create_hashtag_editor(vec![]);
    update(&editor, || {
        get_root().append(&[paragraph(&[
            create_text_node("one "),
            with_note(create_hashtag_node("#a"), "c1"),
        ])])
    });
    update(&editor, || {
        text_at(1).insert_after(&with_note(create_hashtag_node("#b"), "c2"), true)?;
        Ok(())
    });
    let rows = read(&editor);
    assert_eq!(
        rows.iter().find(|r| r.1 == "#a").map(|r| r.2.as_str()),
        Some("c1")
    );
    assert_eq!(
        rows.iter().find(|r| r.1 == "#b").map(|r| r.2.as_str()),
        Some("c2")
    );
}

// upstream: lexical-text/src/__tests__/unit/registerLexicalTextEntityNodeState.test.ts::every path that turns an entity into text keeps NodeState > joining two blocks across text of another format keeps the NodeState of every node
#[test]
fn joining_two_blocks_across_text_of_another_format_keeps_the_node_state_of_every_node() {
    let editor = create_hashtag_editor(vec![]);
    update(&editor, || {
        get_root().append(&[
            paragraph(&[
                create_text_node("one "),
                with_note(formatted("word", "italic"), "w"),
            ]),
            paragraph(&[
                with_note(create_hashtag_node("#tag"), "c1"),
                create_text_node(" more"),
            ]),
        ])
    });
    update(&editor, || {
        let last = get_root()
            .get_first_child_or_throw()?
            .get_last_descendant()
            .unwrap();
        let first = get_root()
            .get_last_child_or_throw()?
            .get_first_descendant()
            .unwrap();
        let s = create_range_selection();
        s.point_set(
            Which::Anchor,
            last.get_key(),
            last.get_text_content_size(),
            PointType::Text,
        )?;
        s.point_set(Which::Focus, first.get_key(), 0, PointType::Text)?;
        set_selection(Some(s.clone()));
        s.remove_text()
    });
    let rows = read(&editor);
    assert_eq!(
        rows.iter().find(|r| r.1 == "word").map(|r| r.2.as_str()),
        Some("w")
    );
    assert_eq!(
        rows.iter().find(|r| r.1 == "#tag").map(|r| r.2.as_str()),
        Some("c1")
    );
    editor.get_editor_state().read(|| {
        assert_eq!(get_root().get_text_content(), "one word#tag more");
        assert!(get_root().get_all_text_nodes().iter().all(NodeRef::is_text));
    });
}
