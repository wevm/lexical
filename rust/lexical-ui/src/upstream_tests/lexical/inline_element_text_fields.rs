//! Ports of `packages/lexical/src/__tests__/unit/InlineElementTextFields.test.ts`.
//! An element's textFormat and textStyle are persisted only when it has no
//! text a reconcile would derive them from (#7968): at any depth for an
//! inline element, among its direct children for a block.
#![cfg(feature = "extension")]

use crate::extension::build_editor_from_extensions;
use crate::json::{Value, parse};
use crate::node::NodeBase;
use crate::nodes::line_break::create_line_break_node;
use crate::{
    ElementData, InitialEditorStateType, LexicalExtension, LexicalNode, NodeClass, NodeRef,
    UpdateOptions, create, create_paragraph_node, create_text_node, define_extension, get_root,
    parse_serialized_node,
};

#[derive(Clone, Debug, Default)]
struct InlineWrapperNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for InlineWrapperNode {
    fn get_type(&self) -> &'static str {
        "test-inline-wrapper"
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
    fn is_inline(&self) -> bool {
        true
    }
}

fn wrapper(children: &[NodeRef]) -> NodeRef {
    let node = create::<InlineWrapperNode>();
    node.set_text_format(1.0);
    node.set_text_style("color: red");
    node.append(children).unwrap();
    node
}

fn with_editor(f: impl FnOnce() -> Result<(), crate::LexicalError> + 'static) {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        nodes: vec![NodeClass::of::<InlineWrapperNode>()],
        ..LexicalExtension::<(), (), ()>::new("[inline-element-text-fields]", ())
    })
    .into()])
    .unwrap();
    editor
        .update(
            f,
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
    editor.dispose();
}

/// `{textFormat, textStyle}` of `node.exportJSON()`; `None` is `undefined`.
type Fields = (Option<Value>, Option<Value>);

fn of(json: &crate::json::Object) -> Fields {
    let get = |key: &str| json.get(key).filter(|v| !v.is_undefined()).cloned();
    (get("textFormat"), get("textStyle"))
}

fn fields(node: &NodeRef) -> Fields {
    of(&node.export_json(false))
}

/// What ElementNode persists for a block (`getSerializedTextFormat`,
/// `getSerializedTextStyle`): its export before `ParagraphNode`'s backfill
/// from its first text child (#7971).
fn serialized(node: &NodeRef) -> Fields {
    of(&crate::schema::export_node_json_once(
        &*node.get_latest(),
        false,
    ))
}

fn persisted() -> Fields {
    (Some(1.0.into()), Some("color: red".into()))
}

const OMITTED: Fields = (None, None);

fn paragraph_of(children: &[NodeRef]) -> NodeRef {
    let paragraph = create_paragraph_node();
    paragraph.append(children).unwrap();
    paragraph
}

// upstream: lexical/src/__tests__/unit/InlineElementTextFields.test.ts::textFormat and textStyle of an inline element > are omitted when its text sits in an inline element inside it
#[test]
fn inline_are_omitted_when_its_text_sits_in_an_inline_element_inside_it() {
    with_editor(|| {
        let inner = wrapper(&[create_text_node("text")]);
        let outer = wrapper(std::slice::from_ref(&inner));
        let outermost = wrapper(&[wrapper(&[wrapper(&[create_text_node("deep")])])]);
        get_root().append(&[paragraph_of(&[outer.clone(), outermost.clone()])])?;
        assert_eq!(fields(&outer), OMITTED);
        assert_eq!(fields(&inner), OMITTED);
        assert_eq!(fields(&outermost), OMITTED);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/InlineElementTextFields.test.ts::textFormat and textStyle of an inline element > are omitted when it holds text of its own, as before
#[test]
fn inline_are_omitted_when_it_holds_text_of_its_own() {
    with_editor(|| {
        let w = wrapper(&[create_text_node("text")]);
        get_root().append(&[paragraph_of(std::slice::from_ref(&w))])?;
        assert_eq!(fields(&w), OMITTED);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/InlineElementTextFields.test.ts::textFormat and textStyle of an inline element > are persisted when it holds no text at any depth, as before
#[test]
fn inline_are_persisted_when_it_holds_no_text_at_any_depth() {
    with_editor(|| {
        let empty = wrapper(&[]);
        let line_break = wrapper(&[create_line_break_node()]);
        let nested = wrapper(&[wrapper(&[create_line_break_node()])]);
        get_root().append(&[paragraph_of(&[
            empty.clone(),
            line_break.clone(),
            nested.clone(),
        ])])?;
        assert_eq!(fields(&empty), persisted());
        assert_eq!(fields(&line_break), persisted());
        assert_eq!(fields(&nested), persisted());
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/InlineElementTextFields.test.ts::textFormat and textStyle of an inline element > a stored value is read and no longer written once it has text inside
#[test]
fn inline_a_stored_value_is_read_and_no_longer_written_once_it_has_text_inside() {
    with_editor(|| {
        let stored = parse(
            r#"{"children":[{"children":[{"detail":0,"format":1,"mode":"normal","style":"color: red","text":"text","type":"text","version":1}],"direction":null,"format":"","indent":0,"textFormat":1,"textStyle":"color: red","type":"test-inline-wrapper","version":1}],"direction":null,"format":"","indent":0,"textFormat":1,"textStyle":"color: red","type":"test-inline-wrapper","version":1}"#,
        )
        .unwrap();
        let outer = parse_serialized_node(&stored)?;
        {
            let latest = outer.get_latest();
            let element = latest.element().unwrap();
            assert_eq!(element.get_text_format(), 1.0);
            assert_eq!(element.get_text_style(), "color: red");
        }
        get_root().append(&[paragraph_of(std::slice::from_ref(&outer))])?;
        assert_eq!(fields(&outer), OMITTED);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/InlineElementTextFields.test.ts::textFormat and textStyle of a block > are persisted when its text is all inside inline elements, as before
#[test]
fn block_are_persisted_when_its_text_is_all_inside_inline_elements() {
    with_editor(|| {
        let paragraph = create_paragraph_node();
        paragraph.set_text_format(1.0);
        paragraph.set_text_style("color: red");
        paragraph.append(&[wrapper(&[create_text_node("text")])])?;
        get_root().append(std::slice::from_ref(&paragraph))?;
        assert_eq!(serialized(&paragraph), persisted());
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/InlineElementTextFields.test.ts::textFormat and textStyle of a block > are omitted when it holds text of its own, as before
#[test]
fn block_are_omitted_when_it_holds_text_of_its_own() {
    with_editor(|| {
        let paragraph = create_paragraph_node();
        paragraph.set_text_format(1.0);
        paragraph.set_text_style("color: red");
        paragraph.append(&[
            create_text_node("text"),
            wrapper(&[create_text_node("more")]),
        ])?;
        get_root().append(std::slice::from_ref(&paragraph))?;
        assert_eq!(serialized(&paragraph), OMITTED);
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/InlineElementTextFields.test.ts::serializing inline elements nested inside each other > a change in an update is seen by the next serialization in it
#[test]
fn a_change_in_an_update_is_seen_by_the_next_serialization_in_it() {
    with_editor(|| {
        let text = create_text_node("text");
        let inner = wrapper(std::slice::from_ref(&text));
        let outer = wrapper(std::slice::from_ref(&inner));
        get_root().append(&[paragraph_of(std::slice::from_ref(&outer))])?;
        assert_eq!(fields(&outer), OMITTED);
        text.remove(false)?;
        inner.append(&[create_line_break_node()])?;
        assert_eq!(fields(&outer), persisted());
        inner.append(&[create_text_node("again")])?;
        assert_eq!(fields(&outer), OMITTED);
        Ok(())
    });
}
