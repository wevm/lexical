//! Ports of `packages/lexical-html/src/__tests__/unit/ExportAfterFragment.test.ts`.

#![cfg(all(feature = "extension", feature = "rich-text"))]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::html::generate_html_from_nodes;
use crate::lexical_dom::{DomExportOutput, get_document};
use crate::nodes::element::ElementData;
use crate::{
    Editor, InitialEditorStateType, LexicalError, LexicalExtension, LexicalNode, NodeBase,
    NodeClass, NodeRef, create, create_text_node, define_extension, get_root,
};
use std::rc::Rc;

/// An element whose `exportDOM` returns `element` (`fragment` or `div`)
/// and an `after` that replaces it with a `<section data-replaced>`.
macro_rules! export_node {
    ($name:ident, $type:literal, $fragment:expr) => {
        #[derive(Clone, Debug, Default)]
        struct $name {
            base: NodeBase,
            element: ElementData,
        }

        impl LexicalNode for $name {
            fn get_type(&self) -> &'static str {
                $type
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
            fn create_dom(
                &self,
                _this: &NodeRef,
                _config: &crate::EditorConfig,
                _editor: &Editor,
            ) -> Result<crate::dom::DomNode, LexicalError> {
                Ok(get_document().create_element("div"))
            }
            fn export_dom(
                &self,
                _this: &NodeRef,
                _editor: &Editor,
            ) -> Result<DomExportOutput, LexicalError> {
                let doc = get_document();
                let element = if $fragment {
                    doc.create_document_fragment()
                } else {
                    doc.create_element("div")
                };
                let mut output = DomExportOutput::element(element);
                output.after = Some(Box::new(move |_| {
                    let replacement = doc.create_element("section");
                    replacement.set_attribute("data-replaced", "true");
                    Ok(Some(replacement))
                }));
                Ok(output)
            }
        }
    };
}

export_node!(FragmentExportNode, "fragment-export", true);
export_node!(ElementExportNode, "element-export", false);

fn build_editor(factory: fn() -> NodeRef) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| {
            let root = get_root();
            root.clear()?;
            let node = factory();
            node.append(&[create_text_node("body")])?;
            root.append(&[node])
        }))),
        dependencies: vec![crate::rich_text::rich_text_extension().into()],
        nodes: vec![
            NodeClass::of::<FragmentExportNode>(),
            NodeClass::of::<ElementExportNode>(),
        ],
        ..LexicalExtension::<(), (), ()>::new("[after-fragment]", ())
    })
    .into()])
    .unwrap()
}

fn html(editor: &Editor) -> String {
    editor
        .read(|| generate_html_from_nodes(editor, None))
        .unwrap()
        .unwrap()
}

// upstream: lexical-html/src/__tests__/unit/ExportAfterFragment.test.ts::DOMExportOutput.after returning a replacement > applies the replacement when element is a DocumentFragment
#[test]
fn applies_the_replacement_when_element_is_a_document_fragment() {
    let editor = build_editor(create::<FragmentExportNode>);
    let html = html(&editor);
    assert!(html.contains(r#"data-replaced="true""#), "{html}");
}

// upstream: lexical-html/src/__tests__/unit/ExportAfterFragment.test.ts::DOMExportOutput.after returning a replacement > applies the replacement when element is an HTMLElement (control)
#[test]
fn applies_the_replacement_when_element_is_an_html_element() {
    let editor = build_editor(create::<ElementExportNode>);
    let html = html(&editor);
    assert!(html.contains(r#"data-replaced="true""#), "{html}");
}
