//! Ports of `packages/lexical-html/src/__tests__/unit/DOMPreprocess.test.ts`.
//!
//! Upstream parses with `JSDOM` because jsdom's `DOMParser` gives no
//! `styleSheets`; the `html` feature's parser reads a document's `<style>`
//! elements either way.

#![cfg(feature = "extension")]

use super::super::support::update;
use super::support::{build, import_into_splice as import_into};
use crate::extension::LexicalEditorWithDispose;
use crate::html::import::{
    DomImportConfigOverrides, DomImportRule, DomPreprocessFn, GenerateNodesFromDomOptions,
    core_import_extension, create_import_state, define_import_rule, dom_import_extension,
    generate_nodes_from_dom_via_extension, sel,
};
use crate::{
    AnyLexicalExtensionArgument, LexicalExtension, config_extension, create_paragraph_node,
    create_text_node, define_extension, get_root,
};
use std::cell::RefCell;
use std::rc::Rc;

/// `configExtension(DOMImportExtension, {preprocess, rules})`.
fn dom_import(
    preprocess: Vec<DomPreprocessFn>,
    rules: Vec<DomImportRule>,
) -> AnyLexicalExtensionArgument {
    config_extension(
        &dom_import_extension(),
        [DomImportConfigOverrides {
            preprocess: Some(preprocess),
            rules: (!rules.is_empty()).then(|| rules.into_iter().map(Into::into).collect()),
            ..Default::default()
        }],
    )
}

fn host(preprocess: Vec<DomPreprocessFn>, rules: Vec<DomImportRule>) -> LexicalEditorWithDispose {
    build(
        "host",
        vec![
            core_import_extension().into(),
            dom_import(preprocess, rules),
        ],
    )
}

fn first_paragraph_text(editor: &crate::Editor) -> String {
    editor
        .read(|| {
            let para = get_root().get_first_child().expect("expected paragraph");
            assert_eq!(para.get_type(), "paragraph", "expected paragraph");
            para.get_text_content()
        })
        .unwrap()
}

type Log = Rc<RefCell<Vec<String>>>;

/// A preprocessor that logs `name` and goes on.
fn trace(log: &Log, name: String) -> DomPreprocessFn {
    let log = log.clone();
    Rc::new(move |_, _, next| {
        log.borrow_mut().push(name.clone());
        next()
    })
}

// upstream: lexical-html/src/__tests__/unit/DOMPreprocess.test.ts::DOMImportExtension preprocess > default $inlineStylesFromStyleSheets resolves <style> rules to inline styles
#[test]
fn default_inline_styles_from_style_sheets_resolves_style_rules() {
    let editor = build("host", vec![core_import_extension().into()]);
    import_into(
        &editor,
        r#"<style>.xl1 { font-weight: 700; }</style><p><span class="xl1">bold</span></p>"#,
    );
    editor
        .read(|| assert!(get_root().get_all_text_nodes()[0].has_format("bold")))
        .unwrap();
}

// upstream: lexical-html/src/__tests__/unit/DOMPreprocess.test.ts::DOMImportExtension preprocess > app preprocess can mutate the DOM before walking
#[test]
fn app_preprocess_can_mutate_the_dom_before_walking() {
    let strip_scripts: DomPreprocessFn = Rc::new(|dom, _, next| {
        let root = dom.body().unwrap_or_else(|| dom.clone());
        for el in root.query_selector_all("script").unwrap() {
            el.remove();
        }
        next()
    });
    let editor = host(vec![strip_scripts], vec![]);
    import_into(&editor, "<p>before<script>alert(1)</script>after</p>");
    assert_eq!(first_paragraph_text(&editor), "beforeafter");
}

// upstream: lexical-html/src/__tests__/unit/DOMPreprocess.test.ts::DOMImportExtension preprocess > preprocess can write to the session for the rest of the import
#[test]
fn preprocess_can_write_to_the_session_for_the_rest_of_the_import() {
    let source_state = create_import_state::<String>("test/import-source", || "unknown".to_owned());
    let state = source_state.clone();
    let read_source_meta: DomPreprocessFn = Rc::new(move |dom, ctx, next| {
        let root = dom.body().unwrap_or_else(|| dom.clone());
        if let Some(meta) = root
            .query_selector_all(r#"meta[name="lexical-source"]"#)
            .unwrap()
            .first()
            .filter(|m| m.is_element())
            && let Some(content) = meta.get_attribute("content").filter(|c| !c.is_empty())
        {
            ctx.session.set(&state, content);
        }
        next()
    });
    let source_aware_div_rule = define_import_rule(
        Some("test/source-aware-div"),
        sel::tag(&["div"]),
        move |ctx, _, _| {
            let p = create_paragraph_node();
            p.append(&[create_text_node(&format!("[{}]", ctx.get(&source_state)))])?;
            Ok(vec![p])
        },
    );
    let editor = host(vec![read_source_meta], vec![source_aware_div_rule]);
    import_into(
        &editor,
        r#"<meta name="lexical-source" content="paste"><div>x</div>"#,
    );
    assert_eq!(first_paragraph_text(&editor), "[paste]");
}

// upstream: lexical-html/src/__tests__/unit/DOMPreprocess.test.ts::DOMImportExtension preprocess > preprocess can write to the import session (shared with rules)
#[test]
fn preprocess_can_write_to_the_import_session() {
    let stylesheets = create_import_state::<Vec<String>>("test/stylesheets", Vec::new);
    let sheets = stylesheets.clone();
    let collect_style_sheets: DomPreprocessFn = Rc::new(move |dom, ctx, next| {
        let root = dom.body().unwrap_or_else(|| dom.clone());
        for el in root.query_selector_all("style").unwrap() {
            ctx.session.update(&sheets, |mut prev| {
                prev.push(el.text_content());
                prev
            });
        }
        next()
    });
    let article_rule = define_import_rule(
        Some("test/article"),
        sel::tag(&["article"]),
        move |ctx, _, _| {
            let sheets = ctx.session.get(&stylesheets);
            let p = create_paragraph_node();
            p.append(&[create_text_node(&format!("[saw {} sheets]", sheets.len()))])?;
            Ok(vec![p])
        },
    );
    let editor = host(vec![collect_style_sheets], vec![article_rule]);
    import_into(
        &editor,
        "<style>.a{}</style><style>.b{}</style><article>body</article>",
    );
    assert_eq!(first_paragraph_text(&editor), "[saw 2 sheets]");
}

// upstream: lexical-html/src/__tests__/unit/DOMPreprocess.test.ts::DOMImportExtension preprocess > middleware chain: a wrapper preprocess can defer to a lower one via $next()
#[test]
fn middleware_chain_a_wrapper_defers_to_a_lower_one() {
    let log: Log = Rc::default();
    let wrap = |name: &'static str| -> DomPreprocessFn {
        let log = log.clone();
        Rc::new(move |_, _, next| {
            log.borrow_mut().push(format!("{name}-before"));
            next()?;
            log.borrow_mut().push(format!("{name}-after"));
            Ok(())
        })
    };
    let editor = host(vec![wrap("inner"), wrap("wrapper")], vec![]);
    import_into(&editor, "<p>x</p>");
    assert_eq!(
        *log.borrow(),
        [
            "wrapper-before",
            "inner-before",
            "inner-after",
            "wrapper-after"
        ]
    );
}

// upstream: lexical-html/src/__tests__/unit/DOMPreprocess.test.ts::DOMImportExtension preprocess > per-call preprocess runs in addition to config-level ones
#[test]
fn per_call_preprocess_runs_in_addition_to_config_level_ones() {
    let log: Log = Rc::default();
    let editor = host(vec![trace(&log, "config".to_owned())], vec![]);
    let per_call = trace(&log, "per-call".to_owned());
    update(&editor, move || {
        let dom =
            crate::dom::parse_from_string("<!doctype html><html><body><p>x</p></body></html>");
        let nodes = generate_nodes_from_dom_via_extension(
            &dom,
            Some(&GenerateNodesFromDomOptions {
                preprocess: Some(vec![per_call]),
                ..Default::default()
            }),
        )?;
        let root = get_root();
        root.clear()?;
        root.splice(0, 0, &nodes)
    });
    assert_eq!(*log.borrow(), ["per-call", "config"]);
}

// upstream: lexical-html/src/__tests__/unit/DOMPreprocess.test.ts::DOMImportExtension preprocess > preprocessors contributed by dependents run before their dependencies
#[test]
fn preprocessors_of_dependents_run_before_their_dependencies() {
    let log: Log = Rc::default();
    let make_extension = |name: &str, dependencies: Vec<AnyLexicalExtensionArgument>| {
        let mut dependencies = dependencies;
        dependencies.push(dom_import(
            vec![
                trace(&log, format!("{name}-a")),
                trace(&log, format!("{name}-b")),
            ],
            vec![],
        ));
        define_extension(LexicalExtension {
            dependencies,
            ..LexicalExtension::<(), (), ()>::new(format!("preprocess-{name}"), ())
        })
    };
    let leaf = make_extension("leaf", vec![]);
    let mid = make_extension("mid", vec![leaf.into()]);
    let editor = build(
        "preprocess-root",
        vec![core_import_extension().into(), mid.into()],
    );
    import_into(&editor, "<p>x</p>");
    assert_eq!(*log.borrow(), ["mid-b", "mid-a", "leaf-b", "leaf-a"]);
}
