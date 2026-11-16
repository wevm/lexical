//! Ports of `packages/lexical/src/nodes/__tests__/unit/LexicalParagraphNode.test.ts`.

use crate::json::Value;
use crate::nodes::paragraph::ParagraphNode;
use crate::upstream_tests::support::{initialize_unit_test, panics, shape, update};
use crate::{CreateEditorArgs, NodeRef, create_paragraph_node, get_root};
use std::cell::RefCell;
use std::rc::Rc;

/// `editorConfig`: `{namespace: '', theme: {paragraph: 'my-paragraph-class'}}`.
#[cfg(feature = "html")]
fn editor_config() -> crate::EditorConfig {
    crate::EditorConfig {
        theme: crate::json::parse(r#"{"paragraph":"my-paragraph-class"}"#)
            .unwrap()
            .as_object()
            .unwrap()
            .clone(),
    }
}

fn is_paragraph_node(node: &NodeRef) -> bool {
    node.get_latest().downcast_ref::<ParagraphNode>().is_some()
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalParagraphNode.test.ts::LexicalParagraphNode tests > ParagraphNode.constructor
#[test]
fn paragraph_node_constructor() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let paragraph_node = create_paragraph_node();
        assert_eq!(paragraph_node.get_type(), "paragraph");
        assert_eq!(paragraph_node.get_text_content(), "");
        Ok(())
    });
    assert!(panics(create_paragraph_node));
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalParagraphNode.test.ts::LexicalParagraphNode tests > ParagraphNode.exportJSON() should return and object conforming to the expected schema
#[test]
fn paragraph_node_export_json() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let node = create_paragraph_node();
        assert_eq!(
            Value::Object(node.export_json(false)).to_string(),
            r#"{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}"#
        );
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalParagraphNode.test.ts::LexicalParagraphNode tests > ParagraphNode.createDOM()
#[cfg(feature = "html")]
#[test]
fn paragraph_node_create_dom() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let e = editor.clone();
    update(&editor, move || {
        let paragraph_node = create_paragraph_node();
        let node = paragraph_node.get_latest();
        assert_eq!(
            node.create_dom(&paragraph_node, &editor_config(), &e)?
                .outer_html(),
            r#"<p class="my-paragraph-class"></p>"#
        );
        assert_eq!(
            node.create_dom(&paragraph_node, &crate::EditorConfig::default(), &e)?
                .outer_html(),
            "<p></p>"
        );
        Ok(())
    });
}

/// `updateDOM` is its model half in Rust (rust/PARITY.md), which leaves
/// the element as it is.
// upstream: lexical/src/nodes/__tests__/unit/LexicalParagraphNode.test.ts::LexicalParagraphNode tests > ParagraphNode.updateDOM()
#[cfg(feature = "html")]
#[test]
fn paragraph_node_update_dom() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let e = editor.clone();
    update(&editor, move || {
        let paragraph_node = create_paragraph_node();
        let dom_element =
            paragraph_node
                .get_latest()
                .create_dom(&paragraph_node, &editor_config(), &e)?;
        assert_eq!(
            dom_element.outer_html(),
            r#"<p class="my-paragraph-class"></p>"#
        );
        let new_paragraph_node = create_paragraph_node();
        let result = new_paragraph_node
            .get_latest()
            .update_dom(&*paragraph_node.get_latest());
        assert!(!result);
        assert_eq!(
            dom_element.outer_html(),
            r#"<p class="my-paragraph-class"></p>"#
        );
        Ok(())
    });
}

/// Upstream reads the mounted editor's HTML; [`shape`] reads the committed
/// document it renders.
// upstream: lexical/src/nodes/__tests__/unit/LexicalParagraphNode.test.ts::LexicalParagraphNode tests > ParagraphNode.insertNewAfter()
#[test]
fn paragraph_node_insert_new_after() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let paragraph: Rc<RefCell<Option<NodeRef>>> = Rc::default();
    let p = paragraph.clone();
    update(&editor, move || {
        let paragraph_node = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph_node))?;
        *p.borrow_mut() = Some(paragraph_node);
        Ok(())
    });
    assert_eq!(shape(&editor), "<paragraph></paragraph>");
    let paragraph_node = paragraph.take().unwrap();
    let e = editor.clone();
    update(&editor, move || {
        let selection = paragraph_node.select(None, None)?;
        let result = paragraph_node.insert_new_after(&selection, false)?.unwrap();
        assert!(is_paragraph_node(&result));
        assert_eq!(
            result.element_data().get_direction(),
            paragraph_node.element_data().get_direction()
        );
        assert_eq!(shape(&e), "<paragraph></paragraph>");
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalParagraphNode.test.ts::LexicalParagraphNode tests > $createParagraphNode()
#[test]
fn create_paragraph_node_matches_new_paragraph_node() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        let paragraph_node = create_paragraph_node();
        let created_paragraph_node = create_paragraph_node();
        assert_eq!(paragraph_node.get_type(), created_paragraph_node.get_type());
        assert_eq!(
            paragraph_node.get_parent(),
            created_paragraph_node.get_parent()
        );
        assert_ne!(paragraph_node.get_key(), created_paragraph_node.get_key());
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalParagraphNode.test.ts::LexicalParagraphNode tests > $isParagraphNode()
#[test]
fn is_paragraph_node_of_a_paragraph() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    update(&editor, || {
        assert!(is_paragraph_node(&create_paragraph_node()));
        Ok(())
    });
}

// upstream: lexical/src/nodes/__tests__/unit/LexicalParagraphNode.test.ts::LexicalParagraphNode tests > ParagraphNode.importDOM handles both CSS text-align and legacy align attribute
#[cfg(feature = "html")]
#[test]
fn paragraph_node_import_dom_handles_text_align_and_align() {
    use crate::lexical_dom::{DomConversionNode, get_conversion_function, get_document};
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let e = editor.clone();
    update(&editor, move || {
        // `$runDOMConversion(editor, element)`.
        let convert = |element: &crate::dom::DomNode| -> NodeRef {
            let conversion = get_conversion_function(element, &e).expect("a conversion");
            match conversion(element).unwrap().expect("an output").node {
                DomConversionNode::One(node) => {
                    assert!(
                        is_paragraph_node(&node),
                        "Expected node to be a ParagraphNode"
                    );
                    node
                }
                _ => panic!("Expected a single node"),
            }
        };
        let p_align = get_document().create_element("p");
        p_align.set_attribute("align", "right");
        assert_eq!(convert(&p_align).element_data().get_format_type(), "right");

        let p_style = get_document().create_element("p");
        p_style.style().set_property("text-align", "center", "");
        assert_eq!(convert(&p_style).element_data().get_format_type(), "center");

        let p_conflict = get_document().create_element("p");
        p_conflict.set_attribute("align", "right");
        p_conflict.style().set_property("text-align", "left", "");
        assert_eq!(
            convert(&p_conflict).element_data().get_format_type(),
            "left"
        );

        let p_invalid = get_document().create_element("p");
        p_invalid.set_attribute("align", "super-weird-stuff");
        assert_eq!(convert(&p_invalid).element_data().get_format_type(), "");
        Ok(())
    });
}
