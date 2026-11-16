//! Ports of `packages/lexical-html/src/__tests__/unit/DOMRenderConditionalOverrides.test.ts`.
//!
//! `makeEditor`'s line break override has its `$createDOM` only: Rust's
//! render config has no `$updateDOM` (rust/PARITY.md), and neither ported
//! test reaches it. The tests that read the reconciled DOM are skipped
//! (rust/parity/skips.json).

#![cfg(feature = "extension")]

use super::super::support::set_test_root_element;
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::html::import::context_value;
use crate::html::{
    DomOverrideOptions, DomRenderConfigOverrides, DomRenderMatchConfig, NodeMatch,
    RenderStateConfig, create_render_state, dom_override, dom_render_extension,
    generate_html_from_nodes, set_render_context_value, with_render_context,
};
use crate::lexical_dom::get_document;
use crate::nodes::line_break::{LineBreakNode, create_line_break_node};
use crate::{
    Editor, InitialEditorStateType, LexicalExtension, config_extension, create_paragraph_node,
    create_text_node, define_extension, get_root,
};
use std::rc::Rc;

thread_local! {
    static WRAP_DISABLED: RenderStateConfig<bool> = create_render_state("wrapDisabled", || false);
    static TERSE: RenderStateConfig<bool> = create_render_state("terse", || false);
}

fn wrap_disabled() -> RenderStateConfig<bool> {
    WRAP_DISABLED.with(Clone::clone)
}
fn terse() -> RenderStateConfig<bool> {
    TERSE.with(Clone::clone)
}

const WRAP_ATTR: &str = "data-wrap";

/// `makeEditor()`.
fn make_editor() -> LexicalEditorWithDispose {
    let wrap = dom_override(
        Some(vec![NodeMatch::class::<LineBreakNode>()]),
        DomRenderMatchConfig {
            create_dom: Some(Rc::new(|_, _, next, _| {
                let inner = next()?;
                let wrapper = get_document().create_element("span");
                wrapper.set_attribute(WRAP_ATTR, "true");
                wrapper.append_child(&inner);
                Ok(wrapper)
            })),
            ..Default::default()
        },
        DomOverrideOptions {
            disabled_for_editor: Some(Rc::new(|ctx| ctx.get(&wrap_disabled()))),
            ..Default::default()
        },
    );
    let tag = dom_override(
        None,
        DomRenderMatchConfig {
            export_dom: Some(Rc::new(|_, _, next, _| {
                let rval = next()?;
                if let Some(element) = rval.element.as_ref().filter(|e| e.is_element()) {
                    element.set_attribute("data-terse", "true");
                }
                Ok(rval)
            })),
            ..Default::default()
        },
        DomOverrideOptions {
            disabled_for_session: Some(Rc::new(|ctx| !ctx.get(&terse()))),
            ..Default::default()
        },
    );
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(|_| {
            let paragraph = create_paragraph_node();
            paragraph.append(&[
                create_text_node("a"),
                create_line_break_node(),
                create_text_node("b"),
            ])?;
            get_root().append(&[paragraph])
        }))),
        dependencies: vec![config_extension(
            &dom_render_extension(),
            [DomRenderConfigOverrides {
                overrides: Some(vec![wrap, tag]),
                ..Default::default()
            }],
        )],
        ..LexicalExtension::<(), (), ()>::new("test", ())
    })
    .into()])
    .unwrap()
}

/// `getLineBreakKey(editor)`.
fn get_line_break_key(editor: &Editor) -> String {
    editor
        .read(|| {
            let paragraph = get_root().get_first_child_or_throw().unwrap();
            paragraph
                .get_children()
                .into_iter()
                .find(|c| c.get_type() == "linebreak")
                .expect("Expected a LineBreakNode in the first paragraph")
                .get_key()
                .to_owned()
        })
        .unwrap()
}

// upstream: lexical-html/src/__tests__/unit/DOMRenderConditionalOverrides.test.ts::DOMRender conditional overrides > disabledForEditor re-render reuses node instances (no node map clone)
#[test]
fn disabled_for_editor_re_render_reuses_node_instances() {
    let editor = make_editor();
    set_test_root_element(&editor);
    let lb_key = get_line_break_key(&editor);
    let key = lb_key.clone();
    let before = editor
        .read(move || crate::get_node_by_key(&key).unwrap().get_latest())
        .unwrap();
    set_render_context_value(&wrap_disabled(), true, &editor).unwrap();
    editor
        .read(move || {
            let after = crate::get_node_by_key(&lb_key).unwrap().get_latest();
            assert!(Rc::ptr_eq(&after, &before));
        })
        .unwrap();
}

// upstream: lexical-html/src/__tests__/unit/DOMRenderConditionalOverrides.test.ts::DOMRender conditional overrides > disabledForSession gates export participation
#[test]
fn disabled_for_session_gates_export_participation() {
    let editor = make_editor();
    set_test_root_element(&editor);
    let cold = editor
        .read(|| generate_html_from_nodes(&editor, None))
        .unwrap()
        .unwrap();
    assert!(!cold.contains("data-terse"), "{cold}");
    let terse_html = editor
        .read(|| {
            with_render_context(&[context_value(&terse(), true)], &editor, || {
                generate_html_from_nodes(&editor, None)
            })
        })
        .unwrap()
        .unwrap();
    assert!(terse_html.contains("data-terse"), "{terse_html}");
}
