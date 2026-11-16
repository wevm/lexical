//! Ports of `packages/lexical-html/src/__tests__/unit/DOMImportExtension.test.ts`.
//!
//! Upstream's `AnchorRule` reads `el.href`, which jsdom resolves against
//! the document URL (`https://example.com/`); the Rust DOM has no URL
//! resolution, so the rule reads the `href` attribute and the anchor test
//! expects it unresolved. The regular expressions the selectors take are
//! [`RegexMatcher`](crate::html::import::sel::RegexMatcher) functions that
//! do what each pattern does.

#![cfg(all(feature = "extension", feature = "link"))]

use super::super::support::update;
use crate::constants::{IS_BOLD, IS_ITALIC};
use crate::extension::LexicalEditorWithDispose;
use crate::html::import::sel::{self, AttrMatch, RegExpMatch, RegexMatcher};
use crate::html::import::{
    DomImportConfigOverrides, DomImportContext, DomImportRule, DomImportRuleEntry,
    GenerateNodesFromDomOptions, ImportChildrenOpts, ImportSourceKind, block_schema, context_value,
    create_import_state, define_import_rule, define_overlay_rules, dom_import_extension,
    generate_nodes_from_dom_via_extension, get_import_context_value, import_source,
    import_text_format, inline_schema, parse_selector, with_import_context,
};
use crate::json::Value;
use crate::link::{LinkAttributes, create_link_node, is_link_node};
use crate::node_state::{StateConfig, create_state, get_state, set_state};
use crate::{
    AnyLexicalExtensionArgument, LexicalError, LexicalExtension, NodeClass, NodeRef,
    config_extension, create_paragraph_node, create_text_node, define_extension, get_editor,
    get_root,
};
use std::cell::RefCell;
use std::rc::Rc;

fn parse_string(v: &Value) -> Option<String> {
    v.as_str().map(str::to_owned)
}

thread_local! {
    static ID_STATE: StateConfig<Option<String>> = create_state("id", parse_string);
    static SOURCE_STATE: StateConfig<Option<String>> = create_state("source", parse_string);
}

fn id_state() -> StateConfig<Option<String>> {
    ID_STATE.with(Clone::clone)
}

fn source_state() -> StateConfig<Option<String>> {
    SOURCE_STATE.with(Clone::clone)
}

/// `/\S/`.
fn non_space() -> RegexMatcher {
    Rc::new(|v: &str| {
        let (index, c) = v.char_indices().find(|(_, c)| !c.is_whitespace())?;
        Some(RegExpMatch {
            groups: vec![Some(c.to_string())],
            index: v[..index].encode_utf16().count(),
            ..Default::default()
        })
    })
}

/// `/^(.+)$/`.
fn whole() -> RegexMatcher {
    Rc::new(|v: &str| {
        (!v.is_empty() && !v.contains(['\n', '\r', '\u{2028}', '\u{2029}'])).then(|| RegExpMatch {
            groups: vec![Some(v.to_owned()), Some(v.to_owned())],
            ..Default::default()
        })
    })
}

fn paragraph(text: &str) -> Vec<NodeRef> {
    let p = create_paragraph_node();
    p.append(&[create_text_node(text)]).unwrap();
    vec![p]
}

fn inline_children(
    ctx: &DomImportContext,
    el: &crate::dom::DomNode,
) -> Result<Vec<NodeRef>, LexicalError> {
    ctx.import_children(
        el,
        &ImportChildrenOpts {
            schema: Some(inline_schema()),
            ..Default::default()
        },
    )
}

fn id_attribute_rule() -> DomImportRule {
    define_import_rule(
        Some("test/id-decorator"),
        sel::any().attr("id", AttrMatch::Matches(non_space(), None)),
        |_, el, next| {
            let out = next()?;
            if out.len() == 1
                && let Some(id) = el.get_attribute("id").filter(|id| !id.is_empty())
            {
                set_state(&out[0], &id_state(), Some(id))?;
            }
            Ok(out)
        },
    )
}

fn anchor_rule() -> DomImportRule {
    define_import_rule(Some("test/anchor"), sel::tag(&["a"]), |ctx, el, next| {
        let Some(href) = el.get_attribute("href").filter(|h| !h.is_empty()) else {
            return next();
        };
        let link = create_link_node(&href, &LinkAttributes::default());
        link.splice(0, 0, &inline_children(ctx, el)?)?;
        Ok(vec![link])
    })
}

fn paragraph_rule() -> DomImportRule {
    define_import_rule(Some("test/paragraph"), sel::tag(&["p"]), |ctx, el, _| {
        let p = create_paragraph_node();
        p.splice(0, 0, &inline_children(ctx, el)?)?;
        Ok(vec![p])
    })
}

fn text_rule() -> DomImportRule {
    define_import_rule(Some("test/text"), sel::text(), |ctx, el, _| {
        let data = el.data();
        if data.is_empty() {
            return Ok(vec![]);
        }
        let text = create_text_node(&data);
        let format = ctx.get(&import_text_format());
        if format != 0 {
            text.set_format(f64::from(format));
        }
        Ok(vec![text])
    })
}

fn format_rule(name: &str, tags: &[&str], flag: i32) -> DomImportRule {
    define_import_rule(Some(name), sel::tag(tags), move |ctx, el, _| {
        let fmt = import_text_format();
        ctx.import_children(
            el,
            &ImportChildrenOpts {
                context: Some(vec![context_value(&fmt, ctx.get(&fmt) | flag)]),
                ..Default::default()
            },
        )
    })
}

fn bold_rule() -> DomImportRule {
    format_rule("test/bold", &["b", "strong"], IS_BOLD)
}

fn italic_rule() -> DomImportRule {
    format_rule("test/italic", &["em", "i"], IS_ITALIC)
}

/// Sibling-emitting: `<figure>` emits `[image-paragraph, caption-paragraph]`.
fn figure_rule() -> DomImportRule {
    define_import_rule(Some("test/figure"), sel::tag(&["figure"]), |ctx, el, _| {
        let children = el.children();
        let mut out = Vec::new();
        if let Some(img) = children.iter().find(|c| c.local_name() == "img") {
            out.extend(paragraph(&format!(
                "[img:{}]",
                img.get_attribute("alt").unwrap_or_default()
            )));
        }
        if let Some(cap) = children.iter().find(|c| c.local_name() == "figcaption") {
            let p = create_paragraph_node();
            p.splice(0, 0, &inline_children(ctx, cap)?)?;
            out.push(p);
        }
        Ok(out)
    })
}

/// Handles `<pre>` only when a child `<code>` carries a `language-` class.
fn code_rule() -> DomImportRule {
    define_import_rule(Some("test/code"), sel::tag(&["pre"]), |_, el, next| {
        let Some(code) = el
            .children()
            .into_iter()
            .find(|c| c.local_name() == "code")
            .filter(|c| c.class_name().contains("language-"))
        else {
            return next();
        };
        // `/(?:^|\s)language-(\S+)/`.
        let lang = code
            .class_name()
            .split_whitespace()
            .find_map(|c| c.strip_prefix("language-").filter(|l| !l.is_empty()))
            .unwrap_or("?")
            .to_owned();
        Ok(paragraph(&format!("[code:{lang}]")))
    })
}

fn capture(ctx: &DomImportContext, name: &str) -> String {
    ctx.captures[name].groups[1].clone().unwrap()
}

fn code_capture_rule() -> DomImportRule {
    define_import_rule(
        Some("test/code-capture"),
        sel::tag(&["pre"]).attr(
            "data-language",
            AttrMatch::Matches(whole(), Some("lang".into())),
        ),
        |ctx, _, _| Ok(paragraph(&format!("[capture:{}]", capture(ctx, "lang")))),
    )
}

fn source_aware_rule() -> DomImportRule {
    define_import_rule(
        Some("test/source-aware"),
        sel::tag(&["div"]),
        |ctx, el, _| {
            let p = create_paragraph_node();
            p.splice(0, 0, &inline_children(ctx, el)?)?;
            let source = match ctx.get(&import_source()) {
                ImportSourceKind::Paste => "paste",
                ImportSourceKind::Unknown => "unknown",
            };
            set_state(&p, &source_state(), Some(source.to_owned()))?;
            Ok(vec![p])
        },
    )
}

fn dom_import_rules(rules: Vec<DomImportRule>) -> AnyLexicalExtensionArgument {
    config_extension(
        &dom_import_extension(),
        [DomImportConfigOverrides {
            rules: Some(rules.into_iter().map(Into::into).collect()),
            ..Default::default()
        }],
    )
}

fn build_editor(
    name: &str,
    dependencies: Vec<AnyLexicalExtensionArgument>,
    extra: Option<AnyLexicalExtensionArgument>,
) -> LexicalEditorWithDispose {
    let host = define_extension(LexicalExtension {
        dependencies,
        nodes: vec![NodeClass::of::<crate::link::LinkNode>()],
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into();
    crate::extension::build_editor_from_extensions(std::iter::once(host).chain(extra)).unwrap()
}

/// `buildTestEditor(extraRules)`.
fn build_test_editor(extra_rules: Vec<DomImportRule>) -> LexicalEditorWithDispose {
    build_editor("test-host", vec![dom_import_rules(extra_rules)], None)
}

/// `$generate(html, options)`.
fn generate(
    html: &str,
    options: Option<&GenerateNodesFromDomOptions>,
) -> Result<Vec<NodeRef>, LexicalError> {
    let dom =
        crate::dom::parse_from_string(&format!("<!doctype html><html><body>{html}</body></html>"));
    generate_nodes_from_dom_via_extension(&dom, options)
}

/// `importInto(editor, html, options)`.
fn import_into(editor: &crate::Editor, html: &str, options: Option<GenerateNodesFromDomOptions>) {
    let html = html.to_owned();
    update(editor, move || {
        let nodes = generate(&html, options.as_ref())?;
        let root = get_root();
        root.clear()?;
        root.splice(0, 0, &nodes)
    });
}

/// `$rootParagraphs()`.
fn root_paragraphs() -> Vec<NodeRef> {
    get_root()
        .get_children()
        .into_iter()
        .filter(|n| n.get_type() == "paragraph")
        .collect()
}

fn root_texts(editor: &crate::Editor) -> Vec<String> {
    editor
        .read(|| {
            root_paragraphs()
                .iter()
                .map(NodeRef::get_text_content)
                .collect()
        })
        .unwrap()
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > basic anchor import + id decorator
#[test]
fn basic_anchor_import_and_id_decorator() {
    let editor = build_test_editor(vec![
        id_attribute_rule(),
        anchor_rule(),
        paragraph_rule(),
        text_rule(),
    ]);
    import_into(
        &editor,
        r#"<p><a id="x" href="https://example.com">link</a></p>"#,
        None,
    );
    editor
        .read(|| {
            let para = &root_paragraphs()[0];
            let link = para.get_first_child().unwrap();
            assert!(is_link_node(&link), "expected LinkNode");
            assert_eq!(get_state(&link, &id_state()).unwrap().as_deref(), Some("x"));
            // jsdom's `el.href` would be `https://example.com/` (see the
            // module comment).
            assert_eq!(link.get_url().as_deref(), Some("https://example.com"));
            let text = link.get_first_child().unwrap();
            assert!(text.is_text(), "expected TextNode");
            assert_eq!(text.get_text_content(), "link");
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > text format propagation via ImportTextFormat context
#[test]
fn text_format_propagation_via_import_text_format_context() {
    let editor = build_test_editor(vec![
        bold_rule(),
        italic_rule(),
        paragraph_rule(),
        text_rule(),
    ]);
    import_into(&editor, "<p>plain <b>bold <i>italicbold</i></b></p>", None);
    editor
        .read(|| {
            let texts = get_root().get_all_text_nodes();
            assert_eq!(texts.len(), 3);
            let find = |s: &str| {
                texts
                    .iter()
                    .find(|n| n.get_text_content() == s)
                    .unwrap_or_else(|| panic!("no TextNode {s:?}"))
                    .clone()
            };
            assert_eq!(find("plain ").text_data().get_format(), 0.0);
            assert!(find("bold ").has_format("bold"));
            assert!(!find("bold ").has_format("italic"));
            assert!(find("italicbold").has_format("bold"));
            assert!(find("italicbold").has_format("italic"));
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > RootSchema wraps stray inline runs in paragraphs
#[test]
fn root_schema_wraps_stray_inline_runs_in_paragraphs() {
    let editor = build_test_editor(vec![paragraph_rule(), text_rule(), bold_rule()]);
    import_into(&editor, "orphan <b>inlines</b> at root", None);
    editor
        .read(|| {
            assert_eq!(get_root().get_children_size(), 1);
            assert_eq!(
                root_paragraphs()[0].get_text_content(),
                "orphan inlines at root"
            );
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > sibling-emitting rule (<figure> -> two paragraphs)
#[test]
fn sibling_emitting_rule() {
    let editor = build_test_editor(vec![figure_rule(), paragraph_rule(), text_rule()]);
    import_into(
        &editor,
        r#"<figure><img src="x" alt="cat"/><figcaption>A cat.</figcaption></figure>"#,
        None,
    );
    assert_eq!(root_texts(&editor), ["[img:cat]", "A cat."]);
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > $next() deferral: code rule defers to next on non-language code
#[test]
fn next_deferral_code_rule_defers_on_non_language_code() {
    let editor = build_test_editor(vec![code_rule(), paragraph_rule(), text_rule()]);
    import_into(
        &editor,
        r#"<pre><code class="language-js">x</code></pre><pre><code>y</code></pre>"#,
        None,
    );
    assert_eq!(root_texts(&editor), ["[code:js]", "y"]);
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > regex capture is exposed on ctx.captures without re-running
#[test]
fn regex_capture_is_exposed_on_ctx_captures() {
    let editor = build_test_editor(vec![code_capture_rule()]);
    import_into(
        &editor,
        r#"<pre data-language="rust">irrelevant</pre>"#,
        None,
    );
    assert_eq!(root_texts(&editor)[0], "[capture:rust]");
    editor.dispose();
}

fn source_of_first_paragraph(editor: &crate::Editor) -> Option<String> {
    editor
        .read(|| get_state(&root_paragraphs()[0], &source_state()).unwrap())
        .unwrap()
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > per-call context: ImportSource flows to rule body
#[test]
fn per_call_context_import_source_flows_to_rule_body() {
    let editor = build_test_editor(vec![source_aware_rule(), text_rule()]);
    for _ in 0..2 {
        import_into(
            &editor,
            "<div>x</div>",
            Some(GenerateNodesFromDomOptions {
                context: Some(vec![context_value(
                    &import_source(),
                    ImportSourceKind::Paste,
                )]),
                ..Default::default()
            }),
        );
        assert_eq!(source_of_first_paragraph(&editor).as_deref(), Some("paste"));
    }
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > per-call context default is "unknown"
#[test]
fn per_call_context_default_is_unknown() {
    let editor = build_test_editor(vec![source_aware_rule(), text_rule()]);
    import_into(&editor, "<div>x</div>", None);
    assert_eq!(
        source_of_first_paragraph(&editor).as_deref(),
        Some("unknown")
    );
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > rule priority: the earlier entry in `rules` runs first; can call $next()
#[test]
fn rule_priority_earlier_entry_runs_first() {
    let editor = build_test_editor(vec![
        id_attribute_rule(),
        anchor_rule(),
        paragraph_rule(),
        text_rule(),
    ]);
    import_into(
        &editor,
        r#"<p><a id="link-x" href="/y">click</a></p>"#,
        None,
    );
    editor
        .read(|| {
            let link = root_paragraphs()[0].get_first_child().unwrap();
            assert!(is_link_node(&link), "expected LinkNode");
            assert_eq!(
                get_state(&link, &id_state()).unwrap().as_deref(),
                Some("link-x")
            );
        })
        .unwrap();
    editor.dispose();
}

fn trace_rule(visited: &Rc<RefCell<Vec<String>>>, rule_name: &str, name: &str) -> DomImportRule {
    let visited = visited.clone();
    let name = name.to_owned();
    define_import_rule(Some(rule_name), sel::tag(&["p"]), move |_, _, next| {
        visited.borrow_mut().push(name.clone());
        next()
    })
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > rules contributed by dependents outrank their dependencies
#[test]
fn rules_contributed_by_dependents_outrank_their_dependencies() {
    let visited = Rc::new(RefCell::new(Vec::<String>::new()));
    let trace = |name: &str| trace_rule(&visited, &format!("test/trace-{name}"), name);
    let make_extension = |name: &str, mut dependencies: Vec<AnyLexicalExtensionArgument>| {
        // Two rules from the same extension, to show that a contribution
        // is inlined as a contiguous chunk.
        dependencies.push(dom_import_rules(vec![
            trace(&format!("{name}-a")),
            trace(&format!("{name}-b")),
        ]));
        define_extension(LexicalExtension {
            dependencies,
            ..LexicalExtension::<(), (), ()>::new(format!("test-{name}"), ())
        })
        .into()
    };
    let leaf = make_extension("leaf", vec![]);
    let mid = make_extension("mid", vec![leaf]);
    // Configuration passed directly to the builder is merged last of all.
    let editor = build_editor(
        "test-root",
        vec![mid],
        Some(dom_import_rules(vec![trace("builder")])),
    );
    import_into(&editor, "<p>x</p>", None);
    assert_eq!(
        *visited.borrow(),
        ["builder", "mid-a", "mid-b", "leaf-a", "leaf-b"]
    );
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > among sibling dependencies, the later-listed one outranks
#[test]
fn among_sibling_dependencies_the_later_listed_one_outranks() {
    let visited = Rc::new(RefCell::new(Vec::<String>::new()));
    let make_extension = |name: &str| -> AnyLexicalExtensionArgument {
        define_extension(LexicalExtension {
            dependencies: vec![dom_import_rules(vec![trace_rule(
                &visited,
                &format!("test/sibling-{name}"),
                name,
            )])],
            ..LexicalExtension::<(), (), ()>::new(format!("test-sibling-{name}"), ())
        })
        .into()
    };
    let editor = build_editor(
        "test-sibling-root",
        vec![make_extension("first"), make_extension("second")],
        None,
    );
    import_into(&editor, "<p>x</p>", None);
    assert_eq!(*visited.borrow(), ["second", "first"]);
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > CSS parser: parseSelector("p.foo") matches as expected
#[test]
fn css_parser_parse_selector_p_foo() {
    let css_rule = define_import_rule(
        Some("test/css-pfoo"),
        parse_selector("p.foo").unwrap(),
        |_, _, _| Ok(paragraph("[matched]")),
    );
    let editor = build_test_editor(vec![css_rule, paragraph_rule(), text_rule()]);
    import_into(&editor, r#"<p class="foo">x</p><p>y</p>"#, None);
    let texts = root_texts(&editor);
    assert_eq!(texts[0], "[matched]");
    assert_eq!(texts[1], "y");
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > CSS parser via sel.css() chains with combinators
#[test]
fn css_parser_via_sel_css_chains_with_combinators() {
    let chained = define_import_rule(
        Some("test/css-combinator"),
        sel::css("span.highlight").unwrap().attr(
            "data-lang",
            AttrMatch::Matches(whole(), Some("lang".into())),
        ),
        |ctx, _, _| Ok(paragraph(&format!("[combo:{}]", capture(ctx, "lang")))),
    );
    let editor = build_test_editor(vec![chained]);
    import_into(
        &editor,
        r#"<span class="highlight" data-lang="go">x</span>"#,
        None,
    );
    assert_eq!(root_texts(&editor)[0], "[combo:go]");
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > CSS selector lists dispatch tag-restricted and unrestricted groups
#[test]
fn css_selector_lists_dispatch_tag_restricted_and_unrestricted_groups() {
    let imported_tags = |selector: &str| -> Vec<String> {
        let rule = define_import_rule(
            Some("test/css-selector-list"),
            sel::css(selector).unwrap(),
            |_, el, _| Ok(paragraph(&el.node_name().to_lowercase())),
        );
        let editor = build_test_editor(vec![rule]);
        import_into(
            &editor,
            r#"<p></p><div class="foo"></div><article></article>"#,
            None,
        );
        let texts = root_texts(&editor);
        editor.dispose();
        texts
    };
    assert_eq!(imported_tags("p, .foo"), ["p", "div"]);
    assert_eq!(imported_tags(".foo, p"), ["p", "div"]);
    assert_eq!(imported_tags("p, .foo, article"), ["p", "div", "article"]);
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > CSS parser rejects an empty selector or a hole in a selector list
#[test]
fn css_parser_rejects_an_empty_selector_or_a_hole() {
    for selector in ["", "   ", "h1,", ",h1", "h1,,h2", "h1, ,h2"] {
        let Err(error) = parse_selector(selector) else {
            panic!("{selector:?} parsed");
        };
        assert!(
            error.message().contains("expected a selector"),
            "{selector:?}: {}",
            error.message()
        );
    }
    // A lone `*` is the one group that legitimately has neither tag nor
    // refinement, so it and any list containing it still parse.
    assert!(parse_selector("*").is_ok());
    assert!(parse_selector("h1, *").is_ok());
    assert!(parse_selector("*.foo").is_ok());
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > isElementOfTag narrows correctly without instanceof
#[test]
fn is_element_of_tag_narrows_correctly() {
    let dom = crate::dom::parse_from_string(
        r#"<!doctype html><html><body><a href="x"></a><p></p></body></html>"#,
    );
    let children = dom.body().unwrap().children();
    let (anchor, para) = (children.first().unwrap(), children.last().unwrap());
    assert!(sel::is_element_of_tag(anchor, "a"));
    assert!(!sel::is_element_of_tag(anchor, "p"));
    assert!(sel::is_element_of_tag(para, "p"));
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::DOMImportExtension > compileImportRules: unknown tags hit wildcard bucket
#[test]
fn compile_import_rules_unknown_tags_hit_wildcard_bucket() {
    let editor = build_test_editor(vec![id_attribute_rule(), paragraph_rule(), text_rule()]);
    import_into(&editor, r#"<my-thing id="custom">z</my-thing>"#, None);
    editor
        .read(|| {
            let p = &root_paragraphs()[0];
            assert_eq!(p.get_text_content(), "z");
            let text = p.get_first_child().unwrap();
            assert!(text.is_text(), "expected TextNode");
            assert_eq!(
                get_state(&text, &id_state()).unwrap().as_deref(),
                Some("custom")
            );
        })
        .unwrap();
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::BlockSchema / InlineSchema > BlockSchema wraps inline runs in paragraphs
#[test]
fn block_schema_wraps_inline_runs_in_paragraphs() {
    let block_rule = define_import_rule(
        Some("test/section"),
        sel::tag(&["section"]),
        |ctx, el, _| {
            let n = ctx
                .import_children(
                    el,
                    &ImportChildrenOpts {
                        schema: Some(block_schema()),
                        ..Default::default()
                    },
                )?
                .len();
            Ok(paragraph(&format!("[section:{n}]")))
        },
    );
    let editor = build_test_editor(vec![block_rule, paragraph_rule(), text_rule()]);
    import_into(
        &editor,
        "<section>inline<p>block</p>more inline</section>",
        None,
    );
    // 3 children: paragraph("inline"), paragraph("block"), paragraph("more inline").
    assert_eq!(root_texts(&editor)[0], "[section:3]");
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::regression sanity for the existing $generateNodesFromDOM > importer state does not leak between $generateNodesFromDOM calls
#[test]
fn importer_state_does_not_leak_between_calls() {
    let editor = build_test_editor(vec![bold_rule(), paragraph_rule(), text_rule()]);
    update(&editor, || {
        let mut nodes = generate("<p><b>x</b></p>", None)?;
        nodes.extend(generate("<p>y</p>", None)?);
        let root = get_root();
        root.clear()?;
        root.splice(0, 0, &nodes)
    });
    editor
        .read(|| {
            let ps = root_paragraphs();
            let t1 = ps[0].get_first_child().unwrap();
            let t2 = ps[1].get_first_child().unwrap();
            assert!(t1.is_text() && t2.is_text(), "expected text nodes");
            assert!(t1.has_format("bold"));
            assert!(!t2.has_format("bold"));
        })
        .unwrap();
    editor.dispose();
}

fn wrap_rule(name: &str, overlay: Rc<crate::html::import::CompiledOverlayRules>) -> DomImportRule {
    define_import_rule(Some(name), sel::tag(&["wrap"]), move |ctx, el, _| {
        ctx.import_children(
            el,
            &ImportChildrenOpts {
                rules: Some(overlay.clone()),
                ..Default::default()
            },
        )
    })
}

fn marker_rule(name: &str, tag: &str, marker: &str) -> DomImportRule {
    let marker = marker.to_owned();
    define_import_rule(Some(name), sel::tag(&[tag]), move |_, _, _| {
        Ok(paragraph(&marker))
    })
}

fn deferring_rule(name: &str, tag: &str) -> DomImportRule {
    define_import_rule(Some(name), sel::tag(&[tag]), |_, _, next| next())
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::$importChildren `rules` overlay > overlay rules take precedence over main rules inside the subtree, and are absent outside
#[test]
fn overlay_rules_take_precedence_inside_the_subtree() {
    let overlay_hits = Rc::new(RefCell::new(0));
    let hits = overlay_hits.clone();
    let overlay = define_overlay_rules(&[define_import_rule(
        Some("test/overlay-x"),
        sel::tag(&["x"]),
        move |_, _, _| {
            *hits.borrow_mut() += 1;
            Ok(paragraph("[overlay-x]"))
        },
    )
    .into()]);
    let editor = build_test_editor(vec![
        wrap_rule("test/wrap", overlay),
        marker_rule("test/main-x", "x", "[main-x]"),
    ]);
    import_into(&editor, "<wrap><x></x></wrap><x></x>", None);
    assert_eq!(root_texts(&editor), ["[overlay-x]", "[main-x]"]);
    assert_eq!(*overlay_hits.borrow(), 1);
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::$importChildren `rules` overlay > overlay rule `$next()` falls through to the main dispatcher
#[test]
fn overlay_rule_next_falls_through_to_the_main_dispatcher() {
    let overlay = define_overlay_rules(&[deferring_rule("test/overlay-x-deferred", "x").into()]);
    let editor = build_test_editor(vec![
        wrap_rule("test/wrap", overlay),
        marker_rule("test/main-x", "x", "[main-x]"),
    ]);
    import_into(&editor, "<wrap><x></x></wrap>", None);
    assert_eq!(root_texts(&editor)[0], "[main-x]");
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::overlay composition via defineOverlayRules > inlines nested overlays in priority order (earlier entry wins)
#[test]
fn inlines_nested_overlays_in_priority_order() {
    let overlay_a = define_overlay_rules(&[marker_rule("test/A-x", "x", "[A]").into()]);
    let overlay_b = define_overlay_rules(&[
        marker_rule("test/B-x", "x", "[B-x]").into(),
        marker_rule("test/B-y", "y", "[B-y]").into(),
    ]);
    let merged = define_overlay_rules(&[
        DomImportRuleEntry::Overlay(overlay_a),
        DomImportRuleEntry::Overlay(overlay_b),
    ]);
    let editor = build_test_editor(vec![wrap_rule("test/wrap-merged", merged)]);
    import_into(&editor, "<wrap><x>x</x><y>y</y></wrap>", None);
    let texts = root_texts(&editor);
    // overlayA's x rule wins over overlayB's x rule (first-arg priority).
    assert_eq!(texts[0], "[A]");
    // overlayB's y rule has no competing rule and fires normally.
    assert_eq!(texts[1], "[B-y]");
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::overlay composition via defineOverlayRules > lower-priority overlay rule fires when the higher one calls $next()
#[test]
fn lower_priority_overlay_rule_fires_when_the_higher_one_calls_next() {
    let overlay_a = define_overlay_rules(&[deferring_rule("test/A-defers", "x").into()]);
    let overlay_b =
        define_overlay_rules(&[marker_rule("test/B-fallback", "x", "[B-fallback]").into()]);
    let merged = define_overlay_rules(&[overlay_a.into(), overlay_b.into()]);
    let editor = build_test_editor(vec![wrap_rule("test/wrap-defers", merged)]);
    import_into(&editor, "<wrap><x>x</x></wrap>", None);
    assert_eq!(root_texts(&editor)[0], "[B-fallback]");
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::ImportContext helpers > $getImportContextValue reads default outside an active import
#[test]
fn get_import_context_value_reads_default_outside_an_active_import() {
    let editor = build_test_editor(vec![]);
    let value = editor
        .read(|| get_import_context_value(&import_source(), &get_editor()))
        .unwrap();
    assert_eq!(value, ImportSourceKind::Unknown);
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::ImportContext helpers > createImportState creates a fresh state with its own default
#[test]
fn create_import_state_creates_a_fresh_state_with_its_own_default() {
    let my_state = create_import_state("test/my-state", || 42.0_f64);
    let editor = build_test_editor(vec![]);
    let value = editor
        .read(|| get_import_context_value(&my_state, &get_editor()))
        .unwrap();
    assert_eq!(value, 42.0);
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/DOMImportExtension.test.ts::ImportContext helpers > an import session chains to the ambient import context
#[test]
fn an_import_session_chains_to_the_ambient_import_context() {
    let my_state = create_import_state("test/ambient", || "default".to_owned());
    let probe_state = my_state.clone();
    let probe = define_import_rule(
        Some("test/ambient-probe"),
        sel::tag(&["cite"]),
        move |ctx, _, _| {
            Ok(vec![create_text_node(&format!(
                "saw:{}",
                ctx.get(&probe_state)
            ))])
        },
    );
    let editor = build_test_editor(vec![probe, paragraph_rule(), text_rule()]);
    update(&editor, move || {
        let text_of = |nodes: Vec<NodeRef>| {
            nodes
                .iter()
                .map(NodeRef::get_text_content)
                .collect::<String>()
        };
        let editor = get_editor();
        assert_eq!(
            text_of(generate("<p><cite></cite></p>", None)?),
            "saw:default"
        );
        with_import_context(
            &[context_value(&my_state, "outer".to_owned())],
            &editor,
            || {
                assert_eq!(
                    text_of(generate("<p><cite></cite></p>", None)?),
                    "saw:outer"
                );
                let options = GenerateNodesFromDomOptions {
                    context: Some(vec![context_value(&my_state, "per-call".to_owned())]),
                    ..Default::default()
                };
                assert_eq!(
                    text_of(generate("<p><cite></cite></p>", Some(&options))?),
                    "saw:per-call"
                );
                Ok(())
            },
        )?;
        assert_eq!(get_import_context_value(&my_state, &editor), "default");
        Ok(())
    });
    editor.dispose();
}
