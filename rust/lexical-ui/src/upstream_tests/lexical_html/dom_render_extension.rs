//! Ports of `packages/lexical-html/src/__tests__/unit/DOMRenderExtension.test.ts`.
//!
//! Upstream compares with `expectHtmlToBeEqual` (both sides through
//! prettier, which orders the attributes); here both sides go through
//! `organize_attributes`, and the expected HTML is written without the
//! formatting whitespace prettier drops. The tests that read the reconciled DOM are
//! skipped (rust/parity/skips.json).

#![cfg(feature = "extension")]

use super::support::organize_attributes;
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::html::{
    DomRenderConfigOverrides, DomRenderMatch, DomRenderMatchConfig, NodeMatch, dom_override,
    dom_render_extension, generate_dom_from_root, generate_html_from_nodes,
    get_render_context_value, render_context_root,
};
use crate::lexical_dom::{DomExportOutput, get_document};
use crate::node_state::StateConfig;
use crate::nodes::line_break::create_line_break_node;
use crate::nodes::text::{TextData, TextNode};
use crate::{
    InitialEditorStateType, LexicalError, LexicalExtension, LexicalNode, NodeBase, NodeClass,
    config_extension, create, create_paragraph_node, create_state, create_text_node,
    define_extension, get_root, get_state, set_state,
};
use std::rc::Rc;

thread_local! {
    static ID_STATE: StateConfig<Option<String>> =
        create_state("id", |v| v.as_str().map(str::to_owned));
}

fn id_state() -> StateConfig<Option<String>> {
    ID_STATE.with(Clone::clone)
}

/// `TextNodeA`: `text-a`, extending `TextNode`.
#[derive(Clone, Debug, Default)]
struct TextNodeA {
    base: NodeBase,
    text: TextData,
}

impl LexicalNode for TextNodeA {
    fn get_type(&self) -> &'static str {
        "text-a"
    }
    fn super_types(&self) -> Vec<&'static str> {
        vec!["text"]
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
}

fn build(
    name: &str,
    initial: impl Fn() -> Result<(), LexicalError> + 'static,
    overrides: Vec<Rc<DomRenderMatch>>,
    nodes: Vec<NodeClass>,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| initial()))),
        dependencies: vec![config_extension(
            &dom_render_extension(),
            [DomRenderConfigOverrides {
                overrides: Some(overrides),
                ..Default::default()
            }],
        )],
        nodes,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

/// `$generateDOMFromRoot(document.createElement('div')).innerHTML`.
fn dom_from_root(editor: &crate::Editor) -> String {
    editor
        .read(|| {
            let container = get_document().create_element("div");
            generate_dom_from_root(&container, &get_root(), editor)?;
            Ok::<_, LexicalError>(container.inner_html())
        })
        .unwrap()
        .unwrap()
}

// upstream: lexical-html/src/__tests__/unit/DOMRenderExtension.test.ts::DOMRenderExtension > $createDOM overrides apply to HTML export (with root: false)
#[test]
fn create_dom_overrides_apply_to_html_export_without_root() {
    let editor = build(
        "export-create-dom",
        || {
            let bold = create_text_node("bold");
            bold.set_format_type("bold");
            let paragraph = create_paragraph_node();
            paragraph.append(&[create_text_node("plain"), create_line_break_node(), bold])?;
            get_root().append(&[paragraph])
        },
        vec![dom_override(
            None,
            DomRenderMatchConfig {
                create_dom: Some(Rc::new(|node, _, next, _| {
                    let dom = next()?;
                    dom.set_attribute("data-node-type", node.get_type());
                    Ok(dom)
                })),
                ..Default::default()
            },
            Default::default(),
        )],
        vec![],
    );
    let html = editor
        .read(|| generate_html_from_nodes(&editor, None))
        .unwrap()
        .unwrap();
    assert_eq!(
        organize_attributes(&html),
        organize_attributes(concat!(
            r#"<p data-node-type="paragraph">"#,
            r#"<span style="white-space: pre-wrap;" data-node-type="text">plain</span>"#,
            r#"<br data-node-type="linebreak">"#,
            r#"<b><strong style="white-space: pre-wrap;" data-node-type="text">bold</strong></b>"#,
            "</p>"
        ))
    );
}

/// `/^\s|\s$|\s\s/.test(text)`.
fn has_significant_whitespace(text: &str) -> bool {
    text.starts_with(char::is_whitespace)
        || text.ends_with(char::is_whitespace)
        || text
            .chars()
            .zip(text.chars().skip(1))
            .any(|(a, b)| a.is_whitespace() && b.is_whitespace())
}

// upstream: lexical-html/src/__tests__/unit/DOMRenderExtension.test.ts::DOMRenderExtension > can override DOM export
#[test]
fn can_override_dom_export() {
    let editor = build(
        "root",
        || {
            let root = get_root();
            set_state(&root, &id_state(), Some("root".to_owned()))?;
            let paragraph = create_paragraph_node();
            set_state(&paragraph, &id_state(), Some("paragraph".to_owned()))?;
            let text = create_text_node("text!");
            set_state(&text, &id_state(), Some("text".to_owned()))?;
            paragraph.append(&[text])?;
            root.append(&[paragraph])
        },
        vec![
            dom_override(
                None,
                DomRenderMatchConfig {
                    export_dom: Some(Rc::new(|_, node, next, _| {
                        let result: DomExportOutput = next()?;
                        if let (Some(id), Some(element)) = (
                            get_state(node, &id_state())?,
                            result.element.as_ref().filter(|e| e.is_element()),
                        ) {
                            element.set_attribute("id", &id);
                        }
                        Ok(result)
                    })),
                    ..Default::default()
                },
                Default::default(),
            ),
            dom_override(
                Some(vec![NodeMatch::class::<TextNode>()]),
                DomRenderMatchConfig {
                    export_dom: Some(Rc::new(|_, _, next, editor| {
                        let result: DomExportOutput = next()?;
                        if let Some(element) = result.element.as_ref().filter(|e| e.is_element())
                            && get_render_context_value(&render_context_root(), editor)
                            && element.style().get_property_value("white-space") == "pre-wrap"
                            && !has_significant_whitespace(&element.text_content())
                        {
                            element.style().remove_property("white-space");
                            if element.style().css_text().is_empty() {
                                element.remove_attribute("style");
                            }
                        }
                        Ok(result)
                    })),
                    ..Default::default()
                },
                Default::default(),
            ),
        ],
        vec![],
    );
    assert_eq!(
        organize_attributes(&dom_from_root(&editor)),
        organize_attributes(
            r#"<div id="root" role="textbox"><p id="paragraph"><span id="text">text!</span></p></div>"#
        )
    );
}

// upstream: lexical-html/src/__tests__/unit/DOMRenderExtension.test.ts::DOMRenderExtension > type merge
#[test]
fn type_merge() {
    let set_data =
        |name: &'static str, value: fn(&dyn LexicalNode) -> String| DomRenderMatchConfig {
            export_dom: Some(Rc::new(move |node, _, next, _| {
                let r: DomExportOutput = next()?;
                if let Some(element) = r.element.as_ref().filter(|e| e.is_element()) {
                    element.set_attribute(name, &value(node));
                }
                Ok(r)
            })),
            ..Default::default()
        };
    let editor = build(
        "root",
        || {
            let a = create::<TextNodeA>();
            a.set_text_content("text a");
            let plain = create_text_node("");
            plain.set_text_content("plain text");
            let paragraph = create_paragraph_node();
            paragraph.append(&[a, create_line_break_node(), plain])?;
            get_root().append(&[paragraph])
        },
        vec![
            dom_override(
                Some(vec![NodeMatch::class::<TextNode>()]),
                DomRenderMatchConfig {
                    export_dom: Some(Rc::new(|node, _, _, _| {
                        let span = get_document().create_element("span");
                        span.append_text(&node.get_text_content());
                        Ok(DomExportOutput::element(span))
                    })),
                    ..Default::default()
                },
                Default::default(),
            ),
            dom_override(
                Some(vec![NodeMatch::class::<TextNodeA>()]),
                set_data("data-lexical-type", |node| node.get_type().to_owned()),
                Default::default(),
            ),
            dom_override(
                Some(vec![NodeMatch::class::<TextNode>()]),
                set_data("data-did-override", |_| "true".to_owned()),
                Default::default(),
            ),
        ],
        vec![NodeClass::of::<TextNodeA>()],
    );
    assert_eq!(
        organize_attributes(&dom_from_root(&editor)),
        organize_attributes(concat!(
            r#"<div role="textbox"><p>"#,
            r#"<span data-did-override="true" data-lexical-type="text-a">text a</span>"#,
            "<br>",
            r#"<span data-did-override="true">plain text</span>"#,
            "</p></div>"
        ))
    );
}
