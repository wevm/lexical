//! Ports of `packages/lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts`.
//!
//! Upstream's editor also has `TableExtension` (and a theme for its
//! wrapper, which only HTML export reads); here it has it with the `table`
//! feature, and the case that imports a table inside a `<pre>` needs it.
//! The GitHub code tables are `CodeExtension`'s own rules, which win over
//! `TableExtension`'s. The `test.each` table is generated from the upstream
//! `CASES`.

#![cfg(all(
    feature = "html",
    feature = "extension",
    feature = "rich-text",
    feature = "list",
    feature = "link"
))]

use super::super::support::update;
use crate::code::{code_extension, is_code_highlight_node, is_code_node};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::html::import::{
    DomImportConfigOverrides, DomImportRule, define_import_rule, dom_import_extension,
    generate_nodes_from_dom_via_extension, sel,
};
use crate::node::NodeClone;
use crate::nodes::element::ElementData;
use crate::nodes::paragraph::ParagraphNode;
use crate::{
    Editor, LexicalError, LexicalExtension, LexicalNode, LexicalNodeReplacement, NodeBase,
    NodeClass, NodeRef, config_extension, create, create_paragraph_node, create_text_node,
    define_extension,
};
use std::rc::Rc;

/// `SemanticParagraphNode`: a `ParagraphNode` whose text is bracketed.
#[derive(Clone, Debug, Default)]
struct SemanticParagraphNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for SemanticParagraphNode {
    fn get_type(&self) -> &'static str {
        "semantic-paragraph"
    }
    fn super_types(&self) -> Vec<&'static str> {
        vec!["paragraph"]
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
    fn get_text_content(&self) -> String {
        format!(
            "[{}]",
            crate::nodes::element::get_text_content(self.as_node())
        )
    }
    fn get_text_content_size(&self) -> usize {
        crate::utf16::len(&self.get_text_content())
    }
}

/// `InlineSemanticParagraphNode`: an inline `SemanticParagraphNode`.
#[derive(Clone, Debug, Default)]
struct InlineSemanticParagraphNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for InlineSemanticParagraphNode {
    fn get_type(&self) -> &'static str {
        "inline-semantic-paragraph"
    }
    fn super_types(&self) -> Vec<&'static str> {
        vec!["semantic-paragraph", "paragraph"]
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
    fn get_text_content(&self) -> String {
        format!(
            "[{}]",
            crate::nodes::element::get_text_content(self.as_node())
        )
    }
    fn get_text_content_size(&self) -> usize {
        crate::utf16::len(&self.get_text_content())
    }
}

/// `TextDecoratorNode`: a block decorator whose text is `[attachment]`.
#[derive(Clone, Debug, Default)]
struct TextDecoratorNode {
    base: NodeBase,
}

impl LexicalNode for TextDecoratorNode {
    fn get_type(&self) -> &'static str {
        "text-decorator"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn is_decorator(&self) -> bool {
        true
    }
    fn is_inline(&self) -> bool {
        false
    }
    fn get_text_content(&self) -> String {
        "[attachment]".to_owned()
    }
    fn create_dom(
        &self,
        _this: &NodeRef,
        _config: &crate::EditorConfig,
        _editor: &Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        Ok(crate::lexical_dom::get_document().create_element("div"))
    }
}

/// `buildEditor(rules, replaceParagraphs)`.
fn build_editor(rules: Vec<DomImportRule>, replace_paragraphs: bool) -> LexicalEditorWithDispose {
    let mut nodes = vec![
        NodeClass::of::<SemanticParagraphNode>(),
        NodeClass::of::<InlineSemanticParagraphNode>(),
        NodeClass::of::<TextDecoratorNode>(),
    ];
    if replace_paragraphs {
        nodes.push(
            LexicalNodeReplacement {
                replace: NodeClass::of::<ParagraphNode>(),
                with: Rc::new(|_: &NodeRef| Ok(create::<SemanticParagraphNode>())),
                with_klass: Some(NodeClass::of::<SemanticParagraphNode>()),
            }
            .into(),
        );
    }
    #[allow(unused_mut)]
    let mut dependencies: Vec<crate::AnyLexicalExtensionArgument> =
        vec![crate::rich_text::rich_text_extension().into()];
    #[cfg(feature = "table")]
    dependencies.push(crate::table::table_extension().into());
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: [
            dependencies,
            vec![
                crate::list::list_extension().into(),
                crate::link::link_extension().into(),
                code_extension().into(),
                config_extension(
                    &dom_import_extension(),
                    [DomImportConfigOverrides {
                        rules: Some(rules.into_iter().map(Into::into).collect()),
                        ..Default::default()
                    }],
                ),
            ],
        ]
        .concat(),
        nodes,
        ..LexicalExtension::<(), (), ()>::new("[code-content-import]", ())
    })
    .into()])
    .unwrap()
}

/// `$generateNodesFromDOMViaExtension(parse(html))`.
fn generate(html: &str) -> Result<Vec<NodeRef>, LexicalError> {
    let dom =
        crate::dom::parse_from_string(&format!("<!doctype html><html><body>{html}</body></html>"));
    generate_nodes_from_dom_via_extension(&dom, None)
}

/// `$expectCodeContent(node, text)`.
fn expect_code_content(node: &NodeRef, text: &str) {
    assert!(is_code_node(node));
    assert_eq!(node.get_text_content(), text);
    let children = node.get_children();
    assert!(children.iter().all(|child| is_code_highlight_node(child)
        || child.get_type() == "tab"
        || child.get_type() == "linebreak"));
    for child in children.iter().filter(|c| is_code_highlight_node(c)) {
        let content = child.get_text_content();
        assert!(!content.contains(['\r', '\n', '\t']), "{content:?}");
    }
}

/// `$import` and `$expectCodeContent` of `html` in `editor`.
fn expect_first_code_content(editor: &Editor, html: &'static str, text: &'static str) {
    update(editor, move || {
        let nodes = generate(html)?;
        expect_code_content(&nodes[0], text);
        Ok(())
    });
}

/// `CASES` (but the table case): `(html, text)`.
const CASES: &[(&str, &str)] = &[
    ("<pre><div>alpha</div><div>beta</div></pre>", "alpha\nbeta"),
    ("<pre><p><br></p></pre>", ""),
    ("<pre><p><br></p><p><br></p><p><br></p></pre>", "\n\n"),
    ("<pre><p><br></p><p>alpha</p></pre>", "\nalpha"),
    (
        "<pre><p>alpha</p><p><br></p><p>beta</p></pre>",
        "alpha\n\nbeta",
    ),
    ("<pre><p>alpha</p><p><br></p></pre>", "alpha\n"),
    ("<pre>alpha<p>beta</p>gamma</pre>", "alpha\nbeta\ngamma"),
    (
        "<pre><section><div>alpha</div><div>beta</div></section><p>gamma</p></pre>",
        "alpha\nbeta\ngamma",
    ),
    (
        "<code><div>alpha<br></div><div>beta</div></code>",
        "alpha\nbeta",
    ),
    (
        "<code><p>alpha</p><p><br></p><p>beta</p></code>",
        "alpha\n\nbeta",
    ),
    (
        "<div style=\"font-family: monospace\"><section>alpha</section><section>beta</section></div>",
        "alpha\nbeta",
    ),
    (
        "<div style=\"font-family: monospace\"><p>alpha</p><p><br></p><p>beta</p></div>",
        "alpha\n\nbeta",
    ),
    (
        "<table class=\"js-file-line-container\"><tr><td class=\"js-file-line\"><div>alpha</div></td></tr><tr><td class=\"js-file-line\"><div>beta</div></td></tr></table>",
        "alpha\nbeta",
    ),
    (
        "<table class=\"js-file-line-container\"><tr><td class=\"js-file-line\"><p>alpha</p><p><br></p><p>beta</p></td></tr></table>",
        "alpha\n\nbeta",
    ),
    (
        "<table class=\"js-file-line-container\"><tr><td class=\"js-file-line\"><p><br></p></td></tr></table>",
        "",
    ),
    ("<pre>\talpha\nbeta</pre>", "\talpha\nbeta"),
    (
        "<pre>a<code>alpha\nbeta</code><p>after</p>b</pre>",
        "a\nalpha\nbeta\nafter\nb",
    ),
    ("<pre><h2>heading</h2><p>body</p></pre>", "heading\nbody"),
    (
        "<pre><a href=\"https://example.com/\">alpha</a><p>beta</p></pre>",
        "alpha\nbeta",
    ),
    (
        "<pre><ul><li>alpha</li><li>beta</li></ul><p>after</p></pre>",
        "alpha\n\nbeta\nafter",
    ),
    (
        "<pre>alpha<br>beta\tgamma<br><br>delta</pre>",
        "alpha\nbeta\tgamma\n\ndelta",
    ),
    (
        "<div style=\"font-family:monospace;white-space:pre\"><div>\talpha</div><br><div>beta</div></div>",
        "\talpha\n\nbeta",
    ),
    (
        "<div style=\"font-family:monospace;white-space:pre\">\talpha</div><br style=\"font-family:monospace;white-space:pre\"><div style=\"font-family:monospace;white-space:pre\">beta</div>",
        "\talpha\n\nbeta",
    ),
    (
        "<div style=\"font-family:monospace\"><div>alpha</div><div>beta</div></div>",
        "alpha\nbeta",
    ),
];

// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > PRE with two nonempty DIV lines
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > PRE with an explicitly empty paragraph
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > PRE with consecutive empty paragraphs
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > PRE with a leading empty paragraph
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > PRE with an empty paragraph between lin…
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > PRE with a trailing empty paragraph
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > PRE with mixed inline and block segments
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > PRE with nested transparent block wrapp…
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > multiline CODE with DIV lines
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > multiline CODE with an empty paragraph
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > monospace DIV with SECTION lines
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > monospace DIV with an empty paragraph
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > GitHub code table with DIV lines in sep…
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > GitHub code table with an empty paragra…
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > GitHub code table with an explicitly em…
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > PRE with flat text tabs and linebreaks …
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > nested multiline CODE alongside paragra…
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > heading followed by a paragraph
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > link text followed by a paragraph
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > list text preserves its own item separa…
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > hard linebreaks and tabs
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > VS Code Chrome wrapper
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > VS Code Safari sibling run
// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > plain monospace DIV with nested DIV lin…
#[test]
fn code_import_normalizes_content_to_supported_code_children() {
    assert_eq!(CASES.len(), 24);
    for &(html, text) in CASES {
        let editor = build_editor(vec![], false);
        update(&editor, move || {
            let nodes = generate(html)?;
            assert_eq!(nodes.len(), 1, "{html}");
            expect_code_content(&nodes[0], text);
            Ok(())
        });
    }
}

// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > table text preserves its own cell separ…
#[cfg(feature = "table")]
#[test]
fn table_text_preserves_its_own_cell_separators() {
    let editor = build_editor(vec![], false);
    update(&editor, || {
        let nodes =
            generate("<pre><table><tr><td>alpha</td><td>beta</td></tr></table><p>after</p></pre>")?;
        assert_eq!(nodes.len(), 1);
        expect_code_content(&nodes[0], "alpha\n\nbeta\nafter");
        Ok(())
    });
}

/// `test.each(['quote', 'semantic-paragraph', 'inline-semantic-paragraph'])`.
fn preserves_text_semantics(ty: &'static str) {
    let editor = build_editor(
        vec![define_import_rule(
            Some("custom-code-child"),
            sel::tag(&["div"]).class_all(&["custom-block"]),
            move |_, _, _| {
                let custom = match ty {
                    "quote" => crate::rich_text::create_quote_node(false),
                    "semantic-paragraph" => create::<SemanticParagraphNode>(),
                    _ => create::<InlineSemanticParagraphNode>(),
                };
                custom.append(&[create_text_node("kept")])?;
                Ok(if ty == "inline-semantic-paragraph" {
                    vec![
                        create_text_node("before"),
                        custom,
                        create_text_node("after"),
                    ]
                } else {
                    let plain = create_paragraph_node();
                    plain.append(&[create_text_node("plain")])?;
                    vec![custom, plain]
                })
            },
        )],
        false,
    );
    expect_first_code_content(
        &editor,
        r#"<pre><div class="custom-block">ignored</div></pre>"#,
        match ty {
            "quote" => "kept\nplain",
            "semantic-paragraph" => "[kept]\nplain",
            _ => "before[kept]after",
        },
    );
}

// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > preserves text semantics from a custom quote node
#[test]
fn preserves_text_semantics_from_a_custom_quote_node() {
    preserves_text_semantics("quote");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > preserves text semantics from a custom semantic-paragraph node
#[test]
fn preserves_text_semantics_from_a_custom_semantic_paragraph_node() {
    preserves_text_semantics("semantic-paragraph");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > preserves text semantics from a custom inline-semantic-paragraph node
#[test]
fn preserves_text_semantics_from_a_custom_inline_semantic_paragraph_node() {
    preserves_text_semantics("inline-semantic-paragraph");
}

// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > uses configured paragraph replacements for their text semantics
#[test]
fn uses_configured_paragraph_replacements_for_their_text_semantics() {
    let editor = build_editor(vec![], true);
    expect_first_code_content(
        &editor,
        "<pre><p>alpha</p><p><br></p><p>beta</p></pre>",
        "[alpha]\n[]\n[beta]",
    );
}

// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > normalizes raw CRLF and tabs returned by a custom importer
#[test]
fn normalizes_raw_crlf_and_tabs_returned_by_a_custom_importer() {
    let editor = build_editor(
        vec![define_import_rule(
            Some("custom-raw-code-text"),
            sel::tag(&["span"]).class_all(&["custom-text"]),
            |_, _, _| Ok(vec![create_text_node("alpha\r\n\tbeta")]),
        )],
        false,
    );
    expect_first_code_content(
        &editor,
        r#"<pre><span class="custom-text">ignored</span></pre>"#,
        "alpha\n\tbeta",
    );
}

// upstream: lexical-code-core/src/__tests__/unit/CodeContentImport.test.ts::code import normalizes content to supported code children > uses a block decorator text fallback between inline runs
#[test]
fn uses_a_block_decorator_text_fallback_between_inline_runs() {
    let editor = build_editor(
        vec![define_import_rule(
            Some("custom-code-attachment"),
            sel::tag(&["span"]).class_all(&["attachment"]),
            |_, _, _| Ok(vec![create::<TextDecoratorNode>()]),
        )],
        false,
    );
    expect_first_code_content(
        &editor,
        r#"<pre>before<span class="attachment"></span>after</pre>"#,
        "before\n[attachment]\nafter",
    );
}
