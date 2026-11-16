//! Ports of `packages/lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts`.

use super::support::{Using, build, j, markdown_test_editor, markdown_test_extension, run_cases};
use crate::link::{
    AutoLinkAttributes, AutoLinkNode, LinkAttributes, create_auto_link_node, create_link_node,
    is_link_node,
};
use crate::list::{ListType, create_list_item_node, create_list_node, is_list_node};
use crate::markdown::{
    RegExp, RegExpMatch, TextMatchExport, TextMatchTransformer, Transformer, check_list,
    convert_selection_to_markdown_string, convert_to_markdown_string, link, transformers,
    unescape_text,
};
use crate::nodes::element::ElementData;
use crate::nodes::line_break::create_line_break_node;
use crate::upstream_tests::support::{update, update_with};
use crate::{
    AnyLexicalExtensionArgument, Editor, LexicalError, LexicalExtension, LexicalNode, NodeBase,
    NodeClass, NodeRef, create, create_paragraph_node, create_text_node, define_extension,
    get_root, get_selection,
};
use std::cell::Cell;
use std::rc::Rc;

const FILE: &str = "lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts";
const SOURCE: &str = include_str!("markdown_escaping.rs");

/// `TestInlineNode` (`markdown-test-inline`): an inline element that exports
/// only its children, like a comment mark.
#[derive(Clone, Debug, Default)]
struct TestInlineNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for TestInlineNode {
    fn get_type(&self) -> &'static str {
        "markdown-test-inline"
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

/// `TestDecoratorNode` (`markdown-test-decorator`): an inline decorator
/// exported as its text content, `foo`.
#[derive(Clone, Debug, Default)]
struct TestDecoratorNode {
    base: NodeBase,
}

impl LexicalNode for TestDecoratorNode {
    fn get_type(&self) -> &'static str {
        "markdown-test-decorator"
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
        true
    }
    fn get_text_content(&self) -> String {
        "foo".to_owned()
    }
}

/// `TestXDecoratorNode` (`markdown-test-x-decorator`): a
/// `TestDecoratorNode` whose text is `x`.
#[derive(Clone, Debug, Default)]
struct TestXDecoratorNode {
    base: NodeBase,
}

impl LexicalNode for TestXDecoratorNode {
    fn get_type(&self) -> &'static str {
        "markdown-test-x-decorator"
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
        true
    }
    fn get_text_content(&self) -> String {
        "x".to_owned()
    }
}

fn nodes_extension(name: &str, nodes: Vec<NodeClass>) -> AnyLexicalExtensionArgument {
    define_extension(LexicalExtension {
        nodes,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()
}

/// `buildEditorFromExtensions(MarkdownTestExtension, TestInlineExtension)`.
fn inline_editor() -> Using {
    build(vec![
        markdown_test_extension(),
        nodes_extension(
            "@lexical/markdown/test-inline",
            vec![NodeClass::of::<TestInlineNode>()],
        ),
    ])
}

/// `buildEditorFromExtensions(MarkdownTestExtension, TestDecoratorExtension)`.
fn decorator_editor() -> Using {
    build(vec![
        markdown_test_extension(),
        nodes_extension(
            "@lexical/markdown/test-decorator",
            vec![
                NodeClass::of::<TestDecoratorNode>(),
                NodeClass::of::<TestXDecoratorNode>(),
            ],
        ),
    ])
}

/// `buildEditorFromExtensions(MarkdownTestExtension, AutoLinkNodeExtension)`.
fn autolink_editor() -> Using {
    build(vec![
        markdown_test_extension(),
        nodes_extension(
            "@lexical/markdown/test-autolink",
            vec![NodeClass::of::<AutoLinkNode>()],
        ),
    ])
}

fn inline_node(children: Vec<NodeRef>) -> NodeRef {
    let node = create::<TestInlineNode>();
    node.append(&children).unwrap();
    node
}

fn text(s: &str) -> NodeRef {
    create_text_node(s)
}

fn styled(s: &str, style: &str) -> NodeRef {
    let node = create_text_node(s);
    node.set_style(style);
    node
}

fn formatted(s: &str, format: &str) -> NodeRef {
    let node = create_text_node(s);
    node.toggle_format(format);
    node
}

fn link_node(url: &str, children: Vec<NodeRef>) -> NodeRef {
    let node = create_link_node(url, &LinkAttributes::default());
    node.append(&children).unwrap();
    node
}

fn format_of(node: &NodeRef) -> f64 {
    node.text_data().get_format()
}

/// `$getRoot().clear().append($createParagraphNode().append(...children()))`
/// in a discrete update.
fn set_paragraph(editor: &Editor, children: impl FnOnce() -> Vec<NodeRef> + 'static) {
    update(editor, move || {
        let paragraph = create_paragraph_node();
        paragraph.append(&children())?;
        {
            let root = get_root();
            root.clear()?;
            root.append(&[paragraph])?;
        }
        Ok(())
    });
}

/// `$exportParagraph(...texts)`: a paragraph of text nodes, each styled
/// apart.
fn export_paragraph(editor: &Editor, texts: &[&str]) {
    let texts: Vec<String> = texts.iter().map(|s| (*s).to_owned()).collect();
    set_paragraph(editor, move || {
        texts
            .iter()
            .enumerate()
            .map(|(index, t)| styled(t, &format!("color: #{index}{index}{index}")))
            .collect()
    });
}

/// `editor.read(() => $convertToMarkdownString(transformers))`.
fn export_with(editor: &Editor, transformers: &[Transformer]) -> String {
    super::support::export(editor, transformers)
}

/// `editor.read(() => $convertToMarkdownString())`.
fn export(editor: &Editor) -> String {
    export_with(editor, &transformers())
}

/// `editor.update(() => $convertFromMarkdownString(markdown, transformers), {discrete: true})`.
fn import_with(editor: &Editor, markdown: &str, transformers: &[Transformer]) {
    super::support::import(editor, markdown, transformers);
}

/// `editor.update(() => $convertFromMarkdownString(markdown), {discrete: true})`.
fn import(editor: &Editor, markdown: &str) {
    import_with(editor, markdown, &transformers());
}

fn root_text(editor: &Editor) -> String {
    editor.read(|| get_root().get_text_content()).unwrap()
}

fn read<R: 'static>(editor: &Editor, f: impl FnOnce() -> R) -> R {
    editor.read(f).unwrap()
}

/// `[CHECK_LIST, ...TRANSFORMERS]`.
fn with_check_list() -> Vec<Transformer> {
    let mut all = vec![Transformer::Element(check_list())];
    all.extend(transformers());
    all
}

fn first_paragraph() -> NodeRef {
    let paragraph = get_root().get_first_child_or_throw().unwrap();
    assert!(paragraph.is_element());
    paragraph
}

/// Slices of UTF-16 code units, as JS's `slice`.
fn split_at(s: &str, at: usize) -> (String, String) {
    let units: Vec<u16> = s.encode_utf16().collect();
    (
        String::from_utf16(&units[..at]).unwrap(),
        String::from_utf16(&units[at..]).unwrap(),
    )
}

fn len16(s: &str) -> usize {
    s.encode_utf16().count()
}

/// JSON of a list of strings, as `%j` of an array.
fn ja(parts: &[&str]) -> String {
    format!(
        "[{}]",
        parts.iter().map(|p| j(p)).collect::<Vec<_>>().join(",")
    )
}

/// A text match transformer with only an `export`, matching nothing
/// (`/$^/`).
fn export_only(
    dependencies: Vec<&'static str>,
    export: impl Fn(&NodeRef, &dyn Fn(&NodeRef) -> Result<String, LexicalError>) -> Option<String>
    + 'static,
) -> Transformer {
    let export: TextMatchExport =
        Rc::new(move |node, export_children, _| Ok(export(node, export_children)));
    Transformer::TextMatch(Rc::new(TextMatchTransformer {
        dependencies,
        export: Some(export),
        import_reg_exp: Some(RegExp::of("$^", "")),
        reg_exp: RegExp::of("$^", ""),
        replace: None,
        get_end_index: None,
        trigger: None,
    }))
}

/// A `@\w+` text match transformer counting its `replace` calls, which sets
/// the node's text to `content` and returns it when `returns`.
fn mention(calls: Rc<Cell<usize>>, content: &'static str, returns: bool) -> Transformer {
    Transformer::TextMatch(Rc::new(TextMatchTransformer {
        dependencies: vec![],
        export: None,
        import_reg_exp: Some(RegExp::of(r"@\w+", "")),
        reg_exp: RegExp::of(r"@\w+$", ""),
        replace: Some(Rc::new(move |node: &NodeRef, _: &RegExpMatch| {
            calls.set(calls.get() + 1);
            node.set_text_content(content);
            Ok(returns.then(|| node.clone()))
        })),
        get_end_index: None,
        trigger: None,
    }))
}

fn case(name: String, f: impl FnOnce() + 'static) -> (String, Box<dyn FnOnce()>) {
    (name, Box::new(f))
}

const LITERALS: [(&str, &str); 8] = [
    ("<b>text</b>", "&#60;b>text&#60;/b>"),
    ("<https://example.com>", "&#60;https://example.com>"),
    ("<0@a.b>", "&#60;0@a.b>"),
    ("<+@a.b>", "&#60;+@a.b>"),
    ("&#65;", "&#38;#65;"),
    ("&#x41;", "&#38;#x41;"),
    ("[label](uri)", "[label&#93;(uri)"),
    ("[ref]: uri", "[ref&#93;: uri"),
];

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > escapes "<b>text</b>" across styled text nodes
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > escapes "<https://example.com>" across styled text nodes
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > escapes "<0@a.b>" across styled text nodes
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > escapes "<+@a.b>" across styled text nodes
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > escapes "&#65;" across styled text nodes
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > escapes "&#x41;" across styled text nodes
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > escapes "[label](uri)" across styled text nodes
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > escapes "[ref]: uri" across styled text nodes
#[test]
fn escapes_across_styled_text_nodes() {
    let cases = LITERALS
        .iter()
        .map(|&(t, expected)| {
            case(
                format!(
                    "literal Markdown text > escapes {} across styled text nodes",
                    j(t)
                ),
                move || {
                    let editor = markdown_test_editor();
                    for split in 0..=len16(t) {
                        let (a, b) = split_at(t, split);
                        set_paragraph(&editor, move || {
                            vec![styled(&a, "color: red"), styled(&b, "color: blue")]
                        });
                        let written = export(&editor);
                        assert_eq!(written, expected);
                        import(&editor, &written);
                        assert_eq!(root_text(&editor), t);
                    }
                },
            )
        })
        .collect();
    run_cases(SOURCE, FILE, "escapes_across_styled_text_nodes", cases);
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps legacy-readable text unchanged: "AT&T"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps legacy-readable text unchanged: "a < b"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps legacy-readable text unchanged: "&nbsp;"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps legacy-readable text unchanged: "&copy;"
#[test]
fn keeps_legacy_readable_text_unchanged() {
    let cases = ["AT&T", "a < b", "&nbsp;", "&copy;"]
        .into_iter()
        .map(|t| {
            case(
                format!(
                    "literal Markdown text > keeps legacy-readable text unchanged: {}",
                    j(t)
                ),
                move || {
                    let editor = markdown_test_editor();
                    set_paragraph(&editor, move || vec![text(t)]);
                    let written = export(&editor);
                    assert_eq!(written, t);
                    import(&editor, &written);
                    assert_eq!(root_text(&editor), t);
                },
            )
        })
        .collect();
    run_cases(SOURCE, FILE, "keeps_legacy_readable_text_unchanged", cases);
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves existing formatted markers after plain text: bold
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves existing formatted markers after plain text: italic
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves existing formatted markers after plain text: strikethrough
#[test]
fn preserves_existing_formatted_markers_after_plain_text() {
    let cases = ["bold", "italic", "strikethrough"]
        .into_iter()
        .map(|format| {
            case(
                format!("literal Markdown text > preserves existing formatted markers after plain text: {format}"),
                move || {
                    let editor = markdown_test_editor();
                    update(&editor, move || {
                        let paragraph = create_paragraph_node();
                        paragraph.append(&[text("a"), formatted("*", format)])?;
                        get_root().append(&[paragraph])?;
                        Ok(())
                    });
                    let written = export(&editor);
                    import(&editor, &written);
                    read(&editor, || {
                        assert_eq!(get_root().get_text_content(), "a*");
                        let nodes = get_root().get_all_text_nodes();
                        assert_eq!(format_of(&nodes[0]), 0.0);
                        assert!(nodes[1].has_format(format));
                    });
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "preserves_existing_formatted_markers_after_plain_text",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps literal delimiters beside escaped punctuation: "*"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps literal delimiters beside escaped punctuation: "**"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps literal delimiters beside escaped punctuation: "~~"
#[test]
fn keeps_literal_delimiters_beside_escaped_punctuation() {
    let cases = ["*", "**", "~~"]
        .into_iter()
        .map(|delimiter| {
            case(
                format!("literal Markdown text > keeps literal delimiters beside escaped punctuation: {}", j(delimiter)),
                move || {
                    let editor = markdown_test_editor();
                    for inside in ["[", "&", "<"] {
                        for outside in ["[", "&", "<"] {
                            let source = format!("a{delimiter}\\{inside}{delimiter}\\{outside}");
                            import(&editor, &source);
                            read(&editor, || {
                                assert_eq!(
                                    get_root().get_text_content(),
                                    format!("a{delimiter}{inside}{delimiter}{outside}")
                                );
                                for t in get_root().get_all_text_nodes() {
                                    assert_eq!(format_of(&t), 0.0);
                                }
                            });
                        }
                    }
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "keeps_literal_delimiters_beside_escaped_punctuation",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > guards ambiguous node boundaries: ["&","#65;"]
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > guards ambiguous node boundaries: ["&","copy;"]
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > guards ambiguous node boundaries: ["<","b>text</b>"]
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > guards ambiguous node boundaries: ["AT&","T"]
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > guards ambiguous node boundaries: ["a <"," b"]
#[test]
fn guards_ambiguous_node_boundaries() {
    let table: [(&[&str], &str); 5] = [
        (&["&", "#65;"], "&#38;#65;"),
        (&["&", "copy;"], "&#38;copy;"),
        (&["<", "b>text</b>"], "&#60;b>text&#60;/b>"),
        (&["AT&", "T"], "AT&#38;T"),
        (&["a <", " b"], "a &#60; b"),
    ];
    let cases = table
        .into_iter()
        .map(|(pieces, expected)| {
            case(
                format!(
                    "literal Markdown text > guards ambiguous node boundaries: {}",
                    ja(pieces)
                ),
                move || {
                    let editor = markdown_test_editor();
                    set_paragraph(&editor, move || {
                        pieces
                            .iter()
                            .enumerate()
                            .map(|(index, t)| {
                                styled(
                                    t,
                                    if index > 0 {
                                        "color: blue"
                                    } else {
                                        "color: red"
                                    },
                                )
                            })
                            .collect()
                    });
                    assert_eq!(export(&editor), expected);
                },
            )
        })
        .collect();
    run_cases(SOURCE, FILE, "guards_ambiguous_node_boundaries", cases);
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps imported literal text through repeated conversion: "\\<b>text\\</b>"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps imported literal text through repeated conversion: "&#60;b&#62;text&#60;/b&#62;"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps imported literal text through repeated conversion: "\\&#65;"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps imported literal text through repeated conversion: "\\&#x41;"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps imported literal text through repeated conversion: "&#38;#65;"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps imported literal text through repeated conversion: "&#x26;#x41;"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps imported literal text through repeated conversion: "\\[label\\](uri)"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps imported literal text through repeated conversion: "\\[label](uri)"
#[test]
fn keeps_imported_literal_text_through_repeated_conversion() {
    let table = [
        (r"\<b>text\</b>", "<b>text</b>"),
        ("&#60;b&#62;text&#60;/b&#62;", "<b>text</b>"),
        (r"\&#65;", "&#65;"),
        (r"\&#x41;", "&#x41;"),
        ("&#38;#65;", "&#65;"),
        ("&#x26;#x41;", "&#x41;"),
        (r"\[label\](uri)", "[label](uri)"),
        (r"\[label](uri)", "[label](uri)"),
    ];
    let cases = table
        .into_iter()
        .map(|(source, t)| {
            case(
                format!("literal Markdown text > keeps imported literal text through repeated conversion: {}", j(source)),
                move || {
                    let editor = markdown_test_editor();
                    let mut written = source.to_owned();
                    for _ in 0..3 {
                        import(&editor, &written);
                        assert_eq!(root_text(&editor), t);
                        written = export(&editor);
                    }
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "keeps_imported_literal_text_through_repeated_conversion",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > escapes only the selected literal content
#[test]
fn escapes_only_the_selected_literal_content() {
    let editor = markdown_test_editor();
    update(&editor, || {
        let t = text("before <b>&copy; [x](u)</b> after");
        let paragraph = create_paragraph_node();
        paragraph.append(std::slice::from_ref(&t))?;
        {
            let root = get_root();
            root.clear()?;
            root.append(&[paragraph])?;
        }
        let size = len16(&t.get_text_content());
        t.select(Some(7), Some(size - 6))?;
        Ok(())
    });
    let written = read(&editor, || {
        convert_selection_to_markdown_string(&transformers(), get_selection().as_ref(), false)
            .unwrap()
    });
    assert_eq!(written, "&#60;b>&copy; [x&#93;(u)&#60;/b>");
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves real link label "[label]"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves real link label "a]b"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves real link label "a[b"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves real link label "a](other)"
#[test]
fn preserves_real_link_label() {
    let cases = ["[label]", "a]b", "a[b", "a](other)"]
        .into_iter()
        .map(|label| {
            case(
                format!(
                    "literal Markdown text > preserves real link label {}",
                    j(label)
                ),
                move || {
                    let editor = markdown_test_editor();
                    set_paragraph(&editor, move || vec![link_node("uri", vec![text(label)])]);
                    let written = export(&editor);
                    import(&editor, &written);
                    read(&editor, || {
                        let l = first_paragraph().get_first_child_or_throw().unwrap();
                        assert!(is_link_node(&l));
                        assert_eq!(l.get_url().as_deref(), Some("uri"));
                        assert_eq!(l.get_text_content(), label);
                    });
                },
            )
        })
        .collect();
    run_cases(SOURCE, FILE, "preserves_real_link_label", cases);
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves existing backslashes in code-formatted link label "[\\]"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves existing backslashes in code-formatted link label "\\[x]"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves existing backslashes in code-formatted link label "[\\]`"
#[test]
fn preserves_existing_backslashes_in_code_formatted_link_label() {
    let cases = [r"[\]", r"\[x]", r"[\]`"]
        .into_iter()
        .map(|label| {
            case(
                format!(
                    "literal Markdown text > preserves existing backslashes in code-formatted link label {}",
                    j(label)
                ),
                move || {
                    let editor = markdown_test_editor();
                    set_paragraph(&editor, move || {
                        vec![link_node("uri", vec![formatted(label, "code")])]
                    });
                    let written = export(&editor);
                    import(&editor, &written);
                    read(&editor, || {
                        let l = first_paragraph().get_first_child_or_throw().unwrap();
                        assert!(is_link_node(&l));
                        let t = l.get_first_child_or_throw().unwrap();
                        assert!(t.is_text());
                        assert_eq!(t.get_text_content(), label);
                        assert!(t.has_format("code"));
                    });
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "preserves_existing_backslashes_in_code_formatted_link_label",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves mixed code/plain brackets in a link: "["
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves mixed code/plain brackets in a link: "]"
#[test]
fn preserves_mixed_code_plain_brackets_in_a_link() {
    let cases = [("[", "]"), ("]", "[")]
        .into_iter()
        .map(|(code, plain)| {
            case(
                format!(
                    "literal Markdown text > preserves mixed code/plain brackets in a link: {}",
                    j(code)
                ),
                move || {
                    let editor = markdown_test_editor();
                    set_paragraph(&editor, move || {
                        vec![link_node("uri", vec![formatted(code, "code"), text(plain)])]
                    });
                    let written = export(&editor);
                    import(&editor, &written);
                    read(&editor, || {
                        let paragraph = first_paragraph();
                        assert_eq!(paragraph.get_children_size(), 1);
                        let l = paragraph.get_first_child_or_throw().unwrap();
                        assert!(is_link_node(&l));
                        assert_eq!(l.get_url().as_deref(), Some("uri"));
                        let children: Vec<(String, bool)> = l
                            .get_children()
                            .iter()
                            .map(|n| {
                                assert!(n.is_text());
                                (n.get_text_content(), n.has_format("code"))
                            })
                            .collect();
                        assert_eq!(
                            children,
                            [(code.to_owned(), true), (plain.to_owned(), false)]
                        );
                    });
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "preserves_mixed_code_plain_brackets_in_a_link",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > continues searching after an escaped link opener
#[test]
fn continues_searching_after_an_escaped_link_opener() {
    let editor = markdown_test_editor();
    import(&editor, r"\[literal](a) [real](b)");
    read(&editor, || {
        let paragraph = first_paragraph();
        let links: Vec<Option<String>> = paragraph
            .get_children()
            .iter()
            .filter(|n| is_link_node(n))
            .map(NodeRef::get_url)
            .collect();
        assert_eq!(links, [Some("b".to_owned())]);
        assert_eq!(paragraph.get_text_content(), "[literal](a) real");
    });
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > does not let an escaped closer absorb a later real link
#[test]
fn does_not_let_an_escaped_closer_absorb_a_later_real_link() {
    let editor = markdown_test_editor();
    import(&editor, r"[a]\](u) [b](v)");
    read(&editor, || {
        let paragraph = first_paragraph();
        assert_eq!(paragraph.get_text_content(), "[a]](u) b");
        let links: Vec<(String, Option<String>)> = paragraph
            .get_children()
            .iter()
            .filter(|n| is_link_node(n))
            .map(|l| (l.get_text_content(), l.get_url()))
            .collect();
        assert_eq!(links, [("b".to_owned(), Some("v".to_owned()))]);
    });
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves copied LINK imports (cloned pattern: false, wrapped replace: false)
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves copied LINK imports (cloned pattern: false, wrapped replace: true)
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves copied LINK imports (cloned pattern: true, wrapped replace: false)
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves copied LINK imports (cloned pattern: true, wrapped replace: true)
#[test]
fn preserves_copied_link_imports() {
    let cases = [(false, false), (false, true), (true, false), (true, true)]
        .into_iter()
        .map(|(cloned_pattern, wrapped_replace)| {
            case(
                format!(
                    "literal Markdown text > preserves copied LINK imports (cloned pattern: {cloned_pattern}, wrapped replace: {wrapped_replace})"
                ),
                move || {
                    let editor = markdown_test_editor();
                    let builtin = link();
                    let export_link = builtin.export.clone().unwrap();
                    let pattern = builtin.import_reg_exp.clone().unwrap();
                    let replace_link = builtin.replace.clone().unwrap();
                    let custom = Rc::new(TextMatchTransformer {
                        export: Some(Rc::new(move |node, export_children, export_format| {
                            export_link(node, export_children, export_format)
                        })),
                        import_reg_exp: Some(if cloned_pattern {
                            RegExp::of(pattern.source(), pattern.flags())
                        } else {
                            pattern
                        }),
                        replace: Some(if wrapped_replace {
                            Rc::new(move |node: &NodeRef, m: &RegExpMatch| replace_link(node, m))
                        } else {
                            replace_link
                        }),
                        ..(*builtin).clone()
                    });
                    let all: Vec<Transformer> = transformers()
                        .into_iter()
                        .map(|t| match t {
                            Transformer::TextMatch(m) if Rc::ptr_eq(&m, &builtin) => {
                                Transformer::TextMatch(custom.clone())
                            }
                            t => t,
                        })
                        .collect();
                    set_paragraph(&editor, || {
                        let l = create_link_node(
                            "v",
                            &LinkAttributes {
                                title: Some(Some("title".to_owned())),
                                ..LinkAttributes::default()
                            },
                        );
                        l.append(&[text("b")]).unwrap();
                        vec![text("[a]](u) "), l]
                    });
                    let written = export_with(&editor, &all);
                    import_with(&editor, &written, &all);
                    read(&editor, || {
                        let children = first_paragraph().get_children();
                        assert!(children[0].is_text());
                        assert_eq!(children[0].get_text_content(), "[a]](u) ");
                        assert!(is_link_node(&children[1]));
                        assert_eq!(
                            (
                                children[1].get_text_content(),
                                children[1].get_url(),
                                children[1].get_title()
                            ),
                            ("b".to_owned(), Some("v".to_owned()), Some("title".to_owned()))
                        );
                    });
                },
            )
        })
        .collect();
    run_cases(SOURCE, FILE, "preserves_copied_link_imports", cases);
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > retains bold before an exported literal escape: "<"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > retains bold before an exported literal escape: "&"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > retains bold before an exported literal escape: "["
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > retains bold before an exported literal escape: "]"
#[test]
fn retains_bold_before_an_exported_literal_escape() {
    let cases = ["<", "&", "[", "]"]
        .into_iter()
        .map(|literal| {
            case(
                format!(
                    "literal Markdown text > retains bold before an exported literal escape: {}",
                    j(literal)
                ),
                move || {
                    let editor = markdown_test_editor();
                    set_paragraph(&editor, move || vec![formatted("!", "bold"), text(literal)]);
                    let written = export(&editor);
                    import(&editor, &written);
                    read(&editor, || {
                        let children: Vec<(String, bool)> = first_paragraph()
                            .get_children()
                            .iter()
                            .map(|n| {
                                assert!(n.is_text());
                                (n.get_text_content(), n.has_format("bold"))
                            })
                            .collect();
                        assert_eq!(
                            children,
                            [("!".to_owned(), true), (literal.to_owned(), false)]
                        );
                    });
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "retains_bold_before_an_exported_literal_escape",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves formatted backslashes next to plain text: bold
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves formatted backslashes next to plain text: italic
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves formatted backslashes next to plain text: strikethrough
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves formatted backslashes next to plain text: bold+italic
#[test]
fn preserves_formatted_backslashes_next_to_plain_text() {
    let cases = ["bold", "italic", "strikethrough", "bold+italic"]
        .into_iter()
        .map(|format| {
            case(
                format!(
                    "literal Markdown text > preserves formatted backslashes next to plain text: {format}"
                ),
                move || {
                    let editor = markdown_test_editor();
                    for before in [false, true] {
                        let t = if before { r"\path" } else { r"path\" };
                        let expected_format = update_with(&editor, move || {
                            let f = create_text_node(t);
                            if format == "bold+italic" {
                                f.toggle_format("bold");
                                f.toggle_format("italic");
                            } else {
                                f.toggle_format(format);
                            }
                            let expected_format = format_of(&f);
                            let plain = text("file");
                            let paragraph = create_paragraph_node();
                            paragraph.append(&if before { [plain, f] } else { [f, plain] })?;
                            { let root = get_root(); root.clear()?; root.append(&[paragraph])?; }
                            Ok(expected_format)
                        });
                        let written = export(&editor);
                        import(&editor, &written);
                        read(&editor, || {
                            let mut expected =
                                vec![(t.to_owned(), expected_format), ("file".to_owned(), 0.0)];
                            if before {
                                expected.reverse();
                            }
                            let children: Vec<(String, f64)> = first_paragraph()
                                .get_children()
                                .iter()
                                .map(|n| {
                                    assert!(n.is_text());
                                    (n.get_text_content(), format_of(n))
                                })
                                .collect();
                            assert_eq!(children, expected);
                        });
                    }
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "preserves_formatted_backslashes_next_to_plain_text",
        cases,
    );
}

/// The paragraph is a text node, `expected`, and a link to `url` reading
/// `label`.
fn assert_text_then_link(expected: &str, url: &str, label: &str) {
    let children = first_paragraph().get_children();
    assert!(children[0].is_text());
    assert_eq!(children[0].get_text_content(), expected);
    assert!(is_link_node(&children[1]));
    assert_eq!(children[1].get_url().as_deref(), Some(url));
    assert_eq!(children[1].get_text_content(), label);
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > decodes escaped text before a later link: "[a\\]\\](u) "
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > decodes escaped text before a later link: "[&#65;\\]\\](u) "
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > decodes escaped text before a later link: "[\\&#65;\\]\\](u) "
#[test]
fn decodes_escaped_text_before_a_later_link() {
    let table = [
        (r"[a\]\](u) ", "[a]](u) "),
        (r"[&#65;\]\](u) ", "[A]](u) "),
        (r"[\&#65;\]\](u) ", "[&#65;]](u) "),
    ];
    let cases = table
        .into_iter()
        .map(|(prefix, expected)| {
            case(
                format!(
                    "literal Markdown text > decodes escaped text before a later link: {}",
                    j(prefix)
                ),
                move || {
                    let editor = markdown_test_editor();
                    import(&editor, &format!("{prefix}[ok](v)"));
                    read(&editor, || assert_text_then_link(expected, "v", "ok"));
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "decodes_escaped_text_before_a_later_link",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves balanced bracket prefixes before a later link: "[a\\][]\\](u) "
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves balanced bracket prefixes before a later link: "[&#65;\\][]\\](u) "
#[test]
fn preserves_balanced_bracket_prefixes_before_a_later_link() {
    let table = [
        (r"[a\][]\](u) ", "[a][]](u) "),
        (r"[&#65;\][]\](u) ", "[A][]](u) "),
    ];
    let cases = table
        .into_iter()
        .map(|(prefix, expected)| {
            case(
                format!(
                    "literal Markdown text > preserves balanced bracket prefixes before a later link: {}",
                    j(prefix)
                ),
                move || {
                    let editor = markdown_test_editor();
                    import(&editor, &format!("{prefix}[ok](v)"));
                    read(&editor, || assert_text_then_link(expected, "v", "ok"));
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "preserves_balanced_bracket_prefixes_before_a_later_link",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves nested openers after an escaped prefix: "[[b](v)"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > preserves nested openers after an escaped prefix: "[a[b](v)"
#[test]
fn preserves_nested_openers_after_an_escaped_prefix() {
    let cases = [("[[b](v)", "["), ("[a[b](v)", "[a")]
        .into_iter()
        .map(|(suffix, prefix)| {
            case(
                format!(
                    "literal Markdown text > preserves nested openers after an escaped prefix: {}",
                    j(suffix)
                ),
                move || {
                    let editor = markdown_test_editor();
                    import(&editor, &format!(r"[a\]\](u) {suffix}"));
                    read(&editor, || {
                        assert_text_then_link(&format!("[a]](u) {prefix}"), "v", "b");
                    });
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "preserves_nested_openers_after_an_escaped_prefix",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps legacy grouping for malformed link candidates: "~~[]~~~](u)"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps legacy grouping for malformed link candidates: "~~[]~~~](u)" #2
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps legacy grouping for malformed link candidates: "~~[\\a]~~~](u)"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps legacy grouping for malformed link candidates: "~~[\\a]~~~](u)" #2
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps legacy grouping for malformed link candidates: "~~[\\]]~~~](u)"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps legacy grouping for malformed link candidates: "~~[\\]]~~~](u)" #2
#[test]
fn keeps_legacy_grouping_for_malformed_link_candidates() {
    let table = [
        ("~~[]~~~](u)", "~~[]~~~](u)", ""),
        ("~~[]~~~](u)", "~~[]~~~](u)", " [ok](v)"),
        (r"~~[\a]~~~](u)", r"~~[\a]~~~](u)", ""),
        (r"~~[\a]~~~](u)", r"~~[\a]~~~](u)", " [ok](v)"),
        (r"~~[\]]~~~](u)", "~~[]]~~~](u)", ""),
        (r"~~[\]]~~~](u)", "~~[]]~~~](u)", " [ok](v)"),
    ];
    let cases = table
        .into_iter()
        .map(|(source, expected, suffix)| {
            case(
                format!(
                    "literal Markdown text > keeps legacy grouping for malformed link candidates: {}{}",
                    j(source),
                    if suffix.is_empty() { "" } else { " #2" }
                ),
                move || {
                    let editor = markdown_test_editor();
                    import(&editor, &format!("{source}{suffix}"));
                    read(&editor, || {
                        let root = get_root();
                        assert_eq!(
                            root.get_text_content(),
                            format!("{expected}{}", if suffix.is_empty() { "" } else { " ok" })
                        );
                        for t in root.get_all_text_nodes() {
                            assert_eq!(format_of(&t), 0.0);
                        }
                        if !suffix.is_empty() {
                            let l = first_paragraph().get_last_child_or_throw().unwrap();
                            assert!(is_link_node(&l));
                            assert_eq!(l.get_url().as_deref(), Some("v"));
                            assert_eq!(l.get_text_content(), "ok");
                        }
                    });
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "keeps_legacy_grouping_for_malformed_link_candidates",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > runs import transformers after escaped link text: code
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > runs import transformers after escaped link text: custom
#[test]
fn runs_import_transformers_after_escaped_link_text() {
    let cases = ["code", "custom"]
        .into_iter()
        .map(|kind| {
            case(
                format!("literal Markdown text > runs import transformers after escaped link text: {kind}"),
                move || {
                    let editor = markdown_test_editor();
                    let calls = Rc::new(Cell::new(0));
                    let middle = if kind == "code" { "`code`" } else { "@name" };
                    let mut all = Vec::new();
                    if kind == "custom" {
                        all.push(mention(calls.clone(), "MENTION", true));
                    }
                    all.extend(transformers());
                    import_with(&editor, &format!(r"[a\]\](u) {middle} [link](v)"), &all);
                    read(&editor, || {
                        let root = get_root();
                        assert_eq!(
                            root.get_text_content(),
                            format!(
                                "[a]](u) {} link",
                                if kind == "code" { "code" } else { "MENTION" }
                            )
                        );
                        if kind == "code" {
                            assert_eq!(
                                root.get_all_text_nodes()
                                    .iter()
                                    .find(|n| n.has_format("code"))
                                    .map(NodeRef::get_text_content)
                                    .as_deref(),
                                Some("code")
                            );
                        }
                        let l = first_paragraph().get_last_child_or_throw().unwrap();
                        assert!(is_link_node(&l));
                        assert_eq!(l.get_url().as_deref(), Some("v"));
                        assert_eq!(l.get_text_content(), "link");
                    });
                    assert_eq!(calls.get(), usize::from(kind == "custom"));
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "runs_import_transformers_after_escaped_link_text",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > parses the text before an unbalanced link
#[test]
fn parses_the_text_before_an_unbalanced_link() {
    let editor = markdown_test_editor();
    // Code spans bind tighter than link brackets, so this is `[`, a code span
    // and a link.
    import(&editor, r"[`\`[x](u)");
    read(&editor, || {
        let children = first_paragraph().get_children();
        assert!(children[0].is_text());
        assert_eq!(children[0].get_text_content(), "[");
        assert!(children[1].is_text());
        assert!(children[1].has_format("code"));
        assert_eq!(children[1].get_text_content(), r"\");
        assert!(is_link_node(&children[2]));
        assert_eq!(children[2].get_url().as_deref(), Some("u"));
        assert_eq!(children[2].get_text_content(), "x");
    });
    assert_eq!(export(&editor), r"[`\`[x](u)");
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps the existing raw-source newline mode
#[test]
fn keeps_the_existing_raw_source_newline_mode() {
    let editor = markdown_test_editor();
    let source = r"\&#65; \<b> \[x](u)";
    set_paragraph(&editor, move || vec![text(source)]);
    let written = read(&editor, || {
        convert_to_markdown_string(&transformers(), None, true).unwrap()
    });
    assert_eq!(written, source);
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > does not escape code or authored custom transformer output
#[test]
fn does_not_escape_code_or_authored_custom_transformer_output() {
    let editor = markdown_test_editor();
    let raw = "<b>&copy; [x](u)</b>";
    set_paragraph(&editor, move || vec![formatted(raw, "code"), text("owned")]);
    let custom = export_only(vec![], move |node, _| {
        (node.get_text_content() == "owned").then(|| raw.to_owned())
    });
    let mut all = vec![custom];
    all.extend(transformers());
    assert_eq!(export_with(&editor, &all), format!("`{raw}`{raw}"));
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::literal Markdown text > keeps custom matchers responsible for their escape rules
#[test]
fn keeps_custom_matchers_responsible_for_their_escape_rules() {
    let editor = markdown_test_editor();
    let calls = Rc::new(Cell::new(0));
    import_with(&editor, r"\@name", &[mention(calls.clone(), "seen", false)]);
    assert_eq!(calls.get(), 1);
    assert_eq!(root_text(&editor), r"\seen");
}

/// Exports `t` split into two styled text nodes at each offset, expects
/// `expected`, and reads it back as `t`, calling `check` after each import.
fn round_trip_splits(t: &str, expected: &str, check: impl Fn()) {
    let editor = markdown_test_editor();
    for split in 0..=len16(t) {
        let (a, b) = split_at(t, split);
        export_paragraph(&editor, &[&a, &b]);
        let written = export(&editor);
        assert_eq!(written, expected);
        import(&editor, &written);
        assert_eq!(root_text(&editor), t);
        read(&editor, &check);
    }
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves "cleared review [1][2]" as typed
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves "Narrative claim [1]." as typed
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves "done [x] and [ ] todo" as typed
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves "array[0] and map[key]" as typed
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves "see [1]: the note" as typed
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves "a [label] (not a link)" as typed
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves "[label]\n(next line)" as typed
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves "nested [[1]] and [a [b] c]" as typed
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves "[" as typed
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves "]" as typed
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves "][" as typed
#[test]
fn leaves_brackets_as_typed() {
    let cases = [
        "cleared review [1][2]",
        "Narrative claim [1].",
        "done [x] and [ ] todo",
        "array[0] and map[key]",
        "see [1]: the note",
        "a [label] (not a link)",
        "[label]\n(next line)",
        "nested [[1]] and [a [b] c]",
        "[",
        "]",
        "][",
    ]
    .into_iter()
    .map(|t| {
        case(
            format!(
                "brackets that cannot become Markdown syntax > leaves {} as typed",
                j(t)
            ),
            move || round_trip_splits(t, t, || {}),
        )
    })
    .collect();
    run_cases(SOURCE, FILE, "leaves_brackets_as_typed", cases);
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the closing bracket of "[a](b)"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the closing bracket of "![a](b)"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the closing bracket of "[a]: b"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the closing bracket of "   [a]: b"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the closing bracket of "[a](b) and [c](d)"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the closing bracket of "x](y)"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the closing bracket of "[[1]](url)"
#[test]
fn escapes_the_closing_bracket_of() {
    let table = [
        ("[a](b)", "[a&#93;(b)"),
        ("![a](b)", "![a&#93;(b)"),
        ("[a]: b", "[a&#93;: b"),
        ("   [a]: b", "   [a&#93;: b"),
        ("[a](b) and [c](d)", "[a&#93;(b) and [c&#93;(d)"),
        ("x](y)", "x&#93;(y)"),
        ("[[1]](url)", "[[1]&#93;(url)"),
    ];
    let cases = table
        .into_iter()
        .map(|(t, expected)| {
            case(
                format!(
                    "brackets that cannot become Markdown syntax > escapes the closing bracket of {}",
                    j(t)
                ),
                move || {
                    round_trip_splits(t, expected, || {
                        assert!(!first_paragraph().get_children().iter().any(is_link_node));
                    });
                },
            )
        })
        .collect();
    run_cases(SOURCE, FILE, "escapes_the_closing_bracket_of", cases);
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes a definition label after a line break
#[test]
fn escapes_a_definition_label_after_a_line_break() {
    let editor = markdown_test_editor();
    export_paragraph(&editor, &["intro\n[a]: b"]);
    assert_eq!(export(&editor), "intro\n[a&#93;: b");
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > writes link text "[1]" as "[[1]](uri)"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > writes link text "claim [2]" as "[claim [2]](uri)"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > writes link text "[a [b] c]" as "[[a [b] c]](uri)"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > writes link text "a]b" as "[a&#93;b](uri)"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > writes link text "a[b" as "[a&#91;b](uri)"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > writes link text "[x](y)" as "[&#91;x&#93;(y)](uri)"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > writes link text "a](other)" as "[a&#93;(other)](uri)"
#[test]
fn writes_link_text() {
    let table = [
        ("[1]", "[[1]](uri)"),
        ("claim [2]", "[claim [2]](uri)"),
        ("[a [b] c]", "[[a [b] c]](uri)"),
        ("a]b", "[a&#93;b](uri)"),
        ("a[b", "[a&#91;b](uri)"),
        ("[x](y)", "[&#91;x&#93;(y)](uri)"),
        ("a](other)", "[a&#93;(other)](uri)"),
    ];
    let cases = table
        .into_iter()
        .map(|(label, expected)| {
            case(
                format!(
                    "brackets that cannot become Markdown syntax > writes link text {} as {}",
                    j(label),
                    j(expected)
                ),
                move || {
                    let editor = markdown_test_editor();
                    set_paragraph(&editor, move || vec![link_node("uri", vec![text(label)])]);
                    let written = export(&editor);
                    assert_eq!(written, expected);
                    import(&editor, &written);
                    read(&editor, || {
                        let paragraph = first_paragraph();
                        assert_eq!(paragraph.get_children_size(), 1);
                        let l = paragraph.get_first_child_or_throw().unwrap();
                        assert!(is_link_node(&l));
                        assert_eq!(l.get_url().as_deref(), Some("uri"));
                        assert_eq!(l.get_text_content(), label);
                    });
                },
            )
        })
        .collect();
    run_cases(SOURCE, FILE, "writes_link_text", cases);
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the task marker "[x] done" at the start of a line
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the task marker "[X] done" at the start of a line
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the task marker "[ ] todo" at the start of a line
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the task marker "[] todo" at the start of a line
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the task marker "  [x] indented" at the start of a line
#[test]
fn escapes_the_task_marker_at_the_start_of_a_line() {
    let table = [
        ("[x] done", "[x&#93; done"),
        ("[X] done", "[X&#93; done"),
        ("[ ] todo", "[ &#93; todo"),
        ("[] todo", "[&#93; todo"),
        ("  [x] indented", "  [x&#93; indented"),
    ];
    let cases = table
        .into_iter()
        .map(|(t, expected)| {
            case(
                format!(
                    "brackets that cannot become Markdown syntax > escapes the task marker {} at the start of a line",
                    j(t)
                ),
                move || {
                    let editor = markdown_test_editor();
                    for split in 0..=len16(t) {
                        let (a, b) = split_at(t, split);
                        export_paragraph(&editor, &[&a, &b]);
                        let written = export(&editor);
                        assert_eq!(written, expected);
                        import_with(&editor, &written, &with_check_list());
                        read(&editor, || {
                            let block = get_root().get_first_child_or_throw().unwrap();
                            assert!(!is_list_node(&block));
                            assert_eq!(block.get_text_content(), t);
                        });
                    }
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "escapes_the_task_marker_at_the_start_of_a_line",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes a task marker that starts a bullet item
#[test]
fn escapes_a_task_marker_that_starts_a_bullet_item() {
    let editor = markdown_test_editor();
    update(&editor, || {
        let item = create_list_item_node(None);
        item.append(&[text("[ ] todo")])?;
        let list = create_list_node(ListType::Bullet, 1.0);
        list.append(&[item])?;
        {
            let root = get_root();
            root.clear()?;
            root.append(&[list])?;
        }
        Ok(())
    });
    let written = export_with(&editor, &with_check_list());
    assert_eq!(written, "- [ &#93; todo");
    import_with(&editor, &written, &with_check_list());
    read(&editor, || {
        let list = get_root().get_first_child_or_throw().unwrap();
        assert!(is_list_node(&list));
        assert_eq!(list.get_list_type(), Some(ListType::Bullet));
        assert_eq!(list.get_text_content(), "[ ] todo");
    });
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > keeps a real check list item as a check list
#[test]
fn keeps_a_real_check_list_item_as_a_check_list() {
    let editor = markdown_test_editor();
    import_with(&editor, "- [ ] todo\n- [x] done", &with_check_list());
    assert_eq!(
        export_with(&editor, &with_check_list()),
        "- [ ] todo\n- [x] done"
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the definition label in ["[a\\","]: /url"]
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the definition label in ["[a](x]: /url"]
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the definition label in ["[foo","\n","bar]: /url"]
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the definition label in ["intro","\n","[a]: b"]
#[test]
fn escapes_the_definition_label_in() {
    let table: [(&[&str], &str); 4] = [
        // [a\ + ]: /url in two nodes: the backslash is exported doubled.
        (&[r"[a\", "]: /url"], r"[a\\&#93;: /url"),
        // The first ] is escaped, so the second still closes a label.
        (&["[a](x]: /url"], "[a&#93;(x&#93;: /url"),
        (&["[foo", "\n", "bar]: /url"], "[foo\nbar&#93;: /url"),
        (&["intro", "\n", "[a]: b"], "intro\n[a&#93;: b"),
    ];
    let cases = table
        .into_iter()
        .map(|(parts, expected)| {
            case(
                format!(
                    "brackets that cannot become Markdown syntax > escapes the definition label in {}",
                    ja(parts)
                ),
                move || {
                    let editor = markdown_test_editor();
                    set_paragraph(&editor, move || {
                        parts
                            .iter()
                            .enumerate()
                            .map(|(index, part)| {
                                if *part == "\n" {
                                    create_line_break_node()
                                } else {
                                    styled(part, &format!("color: #{index}{index}{index}"))
                                }
                            })
                            .collect()
                    });
                    assert_eq!(export(&editor), expected);
                },
            )
        })
        .collect();
    run_cases(SOURCE, FILE, "escapes_the_definition_label_in", cases);
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > treats the start of an exported selection as a line start
#[test]
fn treats_the_start_of_an_exported_selection_as_a_line_start() {
    let editor = markdown_test_editor();
    update(&editor, || {
        let intro = text("intro ");
        let definition = styled("[a]: /url", "color: red");
        let paragraph = create_paragraph_node();
        paragraph.append(&[intro, definition.clone()])?;
        {
            let root = get_root();
            root.clear()?;
            root.append(&[paragraph])?;
        }
        let size = len16(&definition.get_text_content());
        definition.select(Some(0), Some(size))?;
        Ok(())
    });
    let written = read(&editor, || {
        convert_selection_to_markdown_string(&transformers(), get_selection().as_ref(), false)
            .unwrap()
    });
    assert_eq!(written, "[a&#93;: /url");
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > across inline elements > escapes a bracket inside an inline element followed by (
#[test]
fn escapes_a_bracket_inside_an_inline_element_followed_by_paren() {
    let editor = inline_editor();
    set_paragraph(&editor, || {
        vec![inline_node(vec![text("[a]")]), text("(b)")]
    });
    let written = export(&editor);
    assert_eq!(written, "[a&#93;(b)");
    import(&editor, &written);
    assert_eq!(root_text(&editor), "[a](b)");
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > across inline elements > escapes a pair inside link text that an inline element splits from (
#[test]
fn escapes_a_pair_inside_link_text_that_an_inline_element_splits_from_paren() {
    let editor = inline_editor();
    set_paragraph(&editor, || {
        vec![link_node(
            "url",
            vec![inline_node(vec![text("[x]")]), text("(y)")],
        )]
    });
    let written = export(&editor);
    assert_eq!(written, "[&#91;x&#93;(y)](url)");
    import(&editor, &written);
    read(&editor, || {
        let l = first_paragraph().get_first_child_or_throw().unwrap();
        assert!(is_link_node(&l));
        assert_eq!(l.get_url().as_deref(), Some("url"));
        assert_eq!(l.get_text_content(), "[x](y)");
    });
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > across inline elements > escapes a definition label split by an inline element: ["[a]",": b"] (inline first)
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > across inline elements > escapes a definition label split by an inline element: ["[a","]: /url"] (inline second)
#[test]
fn escapes_a_definition_label_split_by_an_inline_element() {
    let table: [(&[&str; 2], &str); 2] = [
        (&["[a]", ": b"], "inline first"),
        (&["[a", "]: /url"], "inline second"),
    ];
    let cases = table
        .into_iter()
        .map(|(parts, which)| {
            case(
                format!(
                    "brackets that cannot become Markdown syntax > across inline elements > escapes a definition label split by an inline element: {} ({which})",
                    ja(parts)
                ),
                move || {
                    let editor = inline_editor();
                    set_paragraph(&editor, move || {
                        let [first, second] = *parts;
                        if which == "inline first" {
                            vec![inline_node(vec![text(first)]), text(second)]
                        } else {
                            vec![text(first), inline_node(vec![text(second)])]
                        }
                    });
                    assert!(export(&editor).contains("&#93;:"));
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "escapes_a_definition_label_split_by_an_inline_element",
        cases,
    );
}

/// `$describe(node)`.
fn describe_node(node: &NodeRef) -> String {
    if is_link_node(node) {
        format!(
            "{{{}:{}}}",
            node.get_url().unwrap_or_default(),
            node.get_children()
                .iter()
                .map(describe_node)
                .collect::<String>()
        )
    } else if node.is_text() {
        format!("<{}:{}>", format_of(node), node.get_text_content())
    } else if node.is_element() {
        node.get_children().iter().map(describe_node).collect()
    } else {
        node.get_text_content()
    }
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > round trips text with brackets before a link > imports "Claim [1] is *important*, see [source](https://example.com)" and exports it again unchanged
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > round trips text with brackets before a link > imports "[1] **bold** [link](u)" and exports it again unchanged
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > round trips text with brackets before a link > imports "[1] `c` [link](u)" and exports it again unchanged
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > round trips text with brackets before a link > imports "**[1]** [l](u)" and exports it again unchanged
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > round trips text with brackets before a link > imports "[a](b) **x** [l](u)" and exports it again unchanged
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > round trips text with brackets before a link > imports "[a&#93;(b) **x** [l](u)" and exports it again unchanged
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > round trips text with brackets before a link > imports "see [1] and ~~[2]~~ [link](u)" and exports it again unchanged
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > round trips text with brackets before a link > imports "[h[ello](u)" and exports it again unchanged
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > round trips text with brackets before a link > imports "[foo [bar](/uri)&#93;(/uri)" and exports it again unchanged
#[test]
fn imports_and_exports_it_again_unchanged() {
    let table = [
        (
            "Claim [1] is *important*, see [source](https://example.com)",
            "<0:Claim [1] is ><2:important><0:, see >{https://example.com:<0:source>}",
        ),
        (
            "[1] **bold** [link](u)",
            "<0:[1] ><1:bold><0: >{u:<0:link>}",
        ),
        ("[1] `c` [link](u)", "<0:[1] ><16:c><0: >{u:<0:link>}"),
        ("**[1]** [l](u)", "<1:[1]><0: >{u:<0:l>}"),
        ("[a](b) **x** [l](u)", "{b:<0:a>}<0: ><1:x><0: >{u:<0:l>}"),
        ("[a&#93;(b) **x** [l](u)", "<0:[a](b) ><1:x><0: >{u:<0:l>}"),
        (
            "see [1] and ~~[2]~~ [link](u)",
            "<0:see [1] and ><4:[2]><0: >{u:<0:link>}",
        ),
        ("[h[ello](u)", "<0:[h>{u:<0:ello>}"),
        (
            "[foo [bar](/uri)&#93;(/uri)",
            "<0:[foo >{/uri:<0:bar>}<0:](/uri)>",
        ),
    ];
    let cases = table
        .into_iter()
        .map(|(markdown, nodes)| {
            case(
                format!(
                    "brackets that cannot become Markdown syntax > round trips text with brackets before a link > imports {} and exports it again unchanged",
                    j(markdown)
                ),
                move || {
                    let editor = markdown_test_editor();
                    import(&editor, markdown);
                    assert_eq!(read(&editor, || describe_node(&get_root())), nodes);
                    let written = export(&editor);
                    assert_eq!(written, markdown);
                    import(&editor, &written);
                    assert_eq!(read(&editor, || describe_node(&get_root())), nodes);
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "imports_and_exports_it_again_unchanged",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > around a decorator > escapes the brackets around [Function TestDecoratorNode]
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > around a decorator > escapes the brackets around [Function TestXDecoratorNode]
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > around a decorator > escapes the brackets around [Function TestDecoratorNode] #2
#[test]
fn escapes_the_brackets_around_a_decorator() {
    let table: [(bool, &str, &str, &str); 3] = [
        (false, "]: done", "&#91;foo&#93;: done", ""),
        (true, "] todo", "&#91;x&#93; todo", ""),
        (false, "](b)", "&#91;foo&#93;(b)", " #2"),
    ];
    let cases = table
        .into_iter()
        .map(|(x, after, expected, suffix)| {
            case(
                format!(
                    "brackets that cannot become Markdown syntax > around a decorator > escapes the brackets around [Function {}]{suffix}",
                    if x { "TestXDecoratorNode" } else { "TestDecoratorNode" }
                ),
                move || {
                    let editor = decorator_editor();
                    set_paragraph(&editor, move || {
                        let decorator = if x {
                            create::<TestXDecoratorNode>()
                        } else {
                            create::<TestDecoratorNode>()
                        };
                        vec![text("["), decorator, text(after)]
                    });
                    assert_eq!(export_with(&editor, &with_check_list()), expected);
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "escapes_the_brackets_around_a_decorator",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > around a decorator > leaves a bracket away from a decorator as typed
#[test]
fn leaves_a_bracket_away_from_a_decorator_as_typed() {
    let editor = decorator_editor();
    set_paragraph(&editor, || {
        vec![
            create::<TestDecoratorNode>(),
            text(" cleared review [1][2]"),
        ]
    });
    assert_eq!(export(&editor), "foo cleared review [1][2]");
}

/// A paragraph of a plain text node and a code one, exported.
fn export_text_then_code(parts: [&'static str; 2]) -> String {
    let editor = markdown_test_editor();
    set_paragraph(&editor, move || {
        vec![text(parts[0]), formatted(parts[1], "code")]
    });
    export(&editor)
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the [ of a label that inline code ["[a","]:b"] would close
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the [ of a label that inline code ["[a b","c]: d"] would close
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the [ of a label that inline code ["[a","b]"] would close
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes the [ of a label that inline code ["[a","[b]: c"] would close
#[test]
fn escapes_the_open_bracket_of_a_label_that_inline_code_would_close() {
    let table = [
        (["[a", "]:b"], "&#91;a`]:b`"),
        (["[a b", "c]: d"], "&#91;a b`c]: d`"),
        (["[a", "b]"], "[a`b]`"),
        (["[a", "[b]: c"], "[a`[b]: c`"),
    ];
    let cases = table
        .into_iter()
        .map(|(parts, expected)| {
            case(
                format!(
                    "brackets that cannot become Markdown syntax > escapes the [ of a label that inline code {} would close",
                    ja(&parts)
                ),
                move || assert_eq!(export_text_then_code(parts), expected),
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "escapes_the_open_bracket_of_a_label_that_inline_code_would_close",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes a line-start [ whose label an escaped ] leaves open to inline code: ["[a](b) ","]:x"]
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes a line-start [ whose label an escaped ] leaves open to inline code: ["[] ","]:url"]
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes a line-start [ whose label an escaped ] leaves open to inline code: ["[x] see ","]:url"]
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes a line-start [ whose label an escaped ] leaves open to inline code: ["[a]: b ","]:c"]
#[test]
fn escapes_a_line_start_bracket_whose_label_an_escaped_bracket_leaves_open_to_inline_code() {
    let table = [
        (["[a](b) ", "]:x"], "&#91;a&#93;(b) `]:x`"),
        (["[] ", "]:url"], "&#91;&#93; `]:url`"),
        (["[x] see ", "]:url"], "&#91;x&#93; see `]:url`"),
        (["[a]: b ", "]:c"], "&#91;a&#93;: b `]:c`"),
    ];
    let cases = table
        .into_iter()
        .map(|(parts, expected)| {
            case(
                format!(
                    "brackets that cannot become Markdown syntax > escapes a line-start [ whose label an escaped ] leaves open to inline code: {}",
                    ja(&parts)
                ),
                move || assert_eq!(export_text_then_code(parts), expected),
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "escapes_a_line_start_bracket_whose_label_an_escaped_bracket_leaves_open_to_inline_code",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > does not read a link whose syntax is inside inline code
#[test]
fn does_not_read_a_link_whose_syntax_is_inside_inline_code() {
    let editor = markdown_test_editor();
    set_paragraph(&editor, || {
        vec![text("see ["), formatted("](", "code"), text("x) now")]
    });
    let written = export(&editor);
    assert_eq!(written, "see [`](`x) now");
    import(&editor, &written);
    read(&editor, || {
        let paragraph = first_paragraph();
        assert!(!paragraph.get_children().iter().any(is_link_node));
        assert_eq!(paragraph.get_text_content(), "see [](x) now");
    });
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes a task marker after a decorator that may end a line
#[test]
fn escapes_a_task_marker_after_a_decorator_that_may_end_a_line() {
    let editor = decorator_editor();
    set_paragraph(&editor, || {
        vec![text("a "), create::<TestDecoratorNode>(), text("[x] done")]
    });
    assert_eq!(export(&editor), "a foo[x&#93; done");
}

fn is_test_inline(node: &NodeRef) -> bool {
    node.get_type() == "markdown-test-inline"
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > custom transformers > applies the link text rules inside an element a transformer writes as [text](url)
#[test]
fn applies_the_link_text_rules_inside_an_element_a_transformer_writes_as_a_link() {
    let editor = inline_editor();
    set_paragraph(&editor, || vec![inline_node(vec![text("a]")])]);
    let test_link = export_only(vec!["markdown-test-inline"], |node, export_children| {
        is_test_inline(node).then(|| format!("[{}](custom)", export_children(node).unwrap()))
    });
    let mut all = vec![test_link];
    all.extend(transformers());
    assert_eq!(export_with(&editor, &all), "[a&#93;](custom)");
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > custom transformers > escapes at the edge of an element a transformer writes with "(note)" after it
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > custom transformers > escapes at the edge of an element a transformer writes with ": note" after it
#[test]
fn escapes_at_the_edge_of_an_element_a_transformer_writes() {
    let table = [
        ("(note)", "see [", "a]", "see [a&#93;(note)"),
        (": note", "", "[a]", "&#91;a&#93;: note"),
    ];
    let cases = table
        .into_iter()
        .map(|(suffix, before, inner, expected)| {
            case(
                format!(
                    "brackets that cannot become Markdown syntax > custom transformers > escapes at the edge of an element a transformer writes with {} after it",
                    j(suffix)
                ),
                move || {
                    let note = export_only(vec!["markdown-test-inline"], move |node, export_children| {
                        is_test_inline(node)
                            .then(|| format!("{}{suffix}", export_children(node).unwrap()))
                    });
                    let editor = inline_editor();
                    set_paragraph(&editor, move || {
                        let mut children = Vec::new();
                        if !before.is_empty() {
                            children.push(text(before));
                        }
                        children.push(inline_node(vec![text(inner)]));
                        children
                    });
                    let mut all = vec![note];
                    all.extend(transformers());
                    assert_eq!(export_with(&editor, &all), expected);
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "escapes_at_the_edge_of_an_element_a_transformer_writes",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > custom transformers > reads a link a transformer writes in another syntax as unknown
#[test]
fn reads_a_link_a_transformer_writes_in_another_syntax_as_unknown() {
    let editor = markdown_test_editor();
    set_paragraph(&editor, || {
        vec![text("["), link_node("u", vec![text("a")]), text("]: /x")]
    });
    let html_link = export_only(vec![], |node, export_children| {
        is_link_node(node).then(|| {
            format!(
                "<a href=\"{}\">{}</a>",
                node.get_url().unwrap_or_default(),
                export_children(node).unwrap()
            )
        })
    });
    let mut all = vec![html_link];
    all.extend(transformers());
    assert_eq!(
        export_with(&editor, &all),
        "&#91;<a href=\"u\">a</a>&#93;: /x"
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > reads typed control characters as text
#[test]
fn reads_typed_control_characters_as_text() {
    let editor = markdown_test_editor();
    export_paragraph(&editor, &["[a\u{0001}]: /url"]);
    assert_eq!(export(&editor), "[a\u{0001}&#93;: /url");
}

fn auto_link(url: &str, label: &str) -> NodeRef {
    let node = create_auto_link_node(url, &AutoLinkAttributes::default());
    node.append(&[text(label)]).unwrap();
    node
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > links not written in brackets > escapes a definition label around autolinks
#[test]
fn escapes_a_definition_label_around_autolinks() {
    let editor = autolink_editor();
    set_paragraph(&editor, || {
        vec![
            text("[see "),
            auto_link("https://example.com", "https://example.com"),
            text(" today]: "),
            auto_link("https://example.com", "https://example.com"),
        ]
    });
    assert_eq!(
        export(&editor),
        "[see https://example.com today&#93;: https://example.com"
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > links not written in brackets > escapes a bracket at the end of an autolink followed by (
#[test]
fn escapes_a_bracket_at_the_end_of_an_autolink_followed_by_paren() {
    let editor = autolink_editor();
    set_paragraph(&editor, || {
        vec![
            auto_link("https://example.com", "https://example.com/[a]"),
            text("(b)"),
        ]
    });
    let written = export(&editor);
    assert_eq!(written, "https://example.com/[a&#93;(b)");
    import(&editor, &written);
    read(&editor, || {
        assert!(!first_paragraph().get_children().iter().any(is_link_node));
    });
}

/// `TRANSFORMERS.filter(transformer => transformer !== LINK)`.
fn without_link() -> Vec<Transformer> {
    let builtin = link();
    transformers()
        .into_iter()
        .filter(|t| !matches!(t, Transformer::TextMatch(m) if Rc::ptr_eq(m, &builtin)))
        .collect()
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > links not written in brackets > treats link text as literal text when LINK is not exported
#[test]
fn treats_link_text_as_literal_text_when_link_is_not_exported() {
    let editor = markdown_test_editor();
    set_paragraph(&editor, || {
        vec![link_node("url", vec![text("[a]")]), text("(b)")]
    });
    assert_eq!(export_with(&editor, &without_link()), "[a&#93;(b)");
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > links not written in brackets > escapes a definition label across a link when LINK is not exported
#[test]
fn escapes_a_definition_label_across_a_link_when_link_is_not_exported() {
    let editor = markdown_test_editor();
    set_paragraph(&editor, || {
        vec![
            text("["),
            link_node("url", vec![text("a")]),
            text("]: /url"),
        ]
    });
    assert_eq!(export_with(&editor, &without_link()), "[a&#93;: /url");
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes "- [x] done" where a container marker precedes it
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes "+ [ ] todo" where a container marker precedes it
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes "* [] todo" where a container marker precedes it
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes "1. [x] done" where a container marker precedes it
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes "> [a]: b" where a container marker precedes it
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes "- [a]: b" where a container marker precedes it
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes "2) [a]: b" where a container marker precedes it
#[test]
fn escapes_where_a_container_marker_precedes_it() {
    let table = [
        ("- [x] done", "- [x&#93; done"),
        ("+ [ ] todo", "+ [ &#93; todo"),
        ("* [] todo", r"\* [] todo"),
        ("1. [x] done", "1. [x&#93; done"),
        ("> [a]: b", "> [a&#93;: b"),
        ("- [a]: b", "- [a&#93;: b"),
        ("2) [a]: b", "2) [a&#93;: b"),
    ];
    let cases = table
        .into_iter()
        .map(|(t, expected)| {
            case(
                format!(
                    "brackets that cannot become Markdown syntax > escapes {} where a container marker precedes it",
                    j(t)
                ),
                move || {
                    let editor = markdown_test_editor();
                    export_paragraph(&editor, &[t]);
                    assert_eq!(export_with(&editor, &with_check_list()), expected);
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "escapes_where_a_container_marker_precedes_it",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves "a- [x] done" as typed
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves "-[x] done" as typed
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves "1.[a]: b" as typed
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > leaves "x > [a]: b" as typed
#[test]
fn leaves_container_like_text_as_typed() {
    let cases = ["a- [x] done", "-[x] done", "1.[a]: b", "x > [a]: b"]
        .into_iter()
        .map(|t| {
            case(
                format!(
                    "brackets that cannot become Markdown syntax > leaves {} as typed",
                    j(t)
                ),
                move || {
                    let editor = markdown_test_editor();
                    export_paragraph(&editor, &[t]);
                    assert_eq!(export(&editor), t);
                },
            )
        })
        .collect();
    run_cases(SOURCE, FILE, "leaves_container_like_text_as_typed", cases);
}

/// Imports `markdown`, within 10 s, and checks the paragraph has no link.
fn import_in_time_without_links(markdown: &str) {
    let editor = markdown_test_editor();
    let start = std::time::Instant::now();
    import(&editor, markdown);
    assert!(start.elapsed() < std::time::Duration::from_secs(10));
    read(&editor, || {
        assert!(!first_paragraph().get_children().iter().any(is_link_node));
    });
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > imports a code span full of [ before a link in linear time
#[test]
fn imports_a_code_span_full_of_brackets_before_a_link_in_linear_time() {
    import_in_time_without_links(&format!("`{}`](u)", "[".repeat(80000)));
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes a task marker indented with Unicode whitespace: " [x] done"
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > escapes a task marker indented with Unicode whitespace: "　[ ] todo"
#[test]
fn escapes_a_task_marker_indented_with_unicode_whitespace() {
    let table = [
        ("\u{00A0}[x] done", "\u{00A0}[x&#93; done"),
        ("\u{3000}[ ] todo", "\u{3000}[ &#93; todo"),
    ];
    let cases = table
        .into_iter()
        .map(|(t, expected)| {
            case(
                format!(
                    "brackets that cannot become Markdown syntax > escapes a task marker indented with Unicode whitespace: {}",
                    j(t)
                ),
                move || {
                    let editor = markdown_test_editor();
                    export_paragraph(&editor, &[t]);
                    let written = export_with(&editor, &with_check_list());
                    assert_eq!(written, expected);
                    import_with(&editor, &written, &with_check_list());
                    assert_eq!(root_text(&editor), t);
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "escapes_a_task_marker_indented_with_unicode_whitespace",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > imports a line of many code spans holding link syntax
#[test]
fn imports_a_line_of_many_code_spans_holding_link_syntax() {
    import_in_time_without_links(&"[a`](x)` ".repeat(2000));
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > exports many styled nodes in linear time
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > exports one long node in linear time
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > exports many brackets in one node in linear time
#[test]
fn exports_in_linear_time() {
    type Children = fn() -> Vec<NodeRef>;
    let table: [(&str, Children); 3] = [
        ("many styled nodes", || {
            (0..8000)
                .map(|index| styled("xa]:", &format!("color: #{}00", index % 2)))
                .collect()
        }),
        ("one long node", || vec![text(&"xa]:".repeat(20000))]),
        ("many brackets in one node", || {
            vec![text(&"[a] ".repeat(20000))]
        }),
    ];
    let cases = table
        .into_iter()
        .map(|(name, children)| {
            case(
                format!(
                    "brackets that cannot become Markdown syntax > exports {name} in linear time"
                ),
                move || {
                    let editor = markdown_test_editor();
                    set_paragraph(&editor, children);
                    let start = std::time::Instant::now();
                    export(&editor);
                    // Linear exports take tens of milliseconds; a quadratic
                    // one takes tens of seconds.
                    assert!(start.elapsed() < std::time::Duration::from_secs(10));
                },
            )
        })
        .collect();
    run_cases(SOURCE, FILE, "exports_in_linear_time", cases);
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::brackets that cannot become Markdown syntax > reads brackets written as references by earlier versions
#[test]
fn reads_brackets_written_as_references_by_earlier_versions() {
    let editor = markdown_test_editor();
    import(&editor, "cleared review &#91;1&#93;&#91;2&#93;");
    assert_eq!(root_text(&editor), "cleared review [1][2]");
}

// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes "&#65;" once
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes "&#x41;" once
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes "&#X41;" once
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes "\\&#65;" once
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes "\\&#x41;" once
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes "&#38;#65;" once
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes "&#x26;#x41;" once
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes "\\\\&#65;" once
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes "&copy;" once
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes "&#0;" once
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes "&#xD800;" once
// upstream: lexical-markdown/src/__tests__/unit/MarkdownEscaping.test.ts::one-pass numeric reference decoding > decodes "&#x110000;" once
#[test]
fn decodes_once() {
    let table = [
        ("&#65;", "A"),
        ("&#x41;", "A"),
        ("&#X41;", "A"),
        (r"\&#65;", "&#65;"),
        (r"\&#x41;", "&#x41;"),
        ("&#38;#65;", "&#65;"),
        ("&#x26;#x41;", "&#x41;"),
        (r"\\&#65;", r"\A"),
        ("&copy;", "&copy;"),
        ("&#0;", "\u{FFFD}"),
        ("&#xD800;", "\u{FFFD}"),
        ("&#x110000;", "\u{FFFD}"),
    ];
    let cases = table
        .into_iter()
        .map(|(source, expected)| {
            case(
                format!(
                    "one-pass numeric reference decoding > decodes {} once",
                    j(source)
                ),
                move || assert_eq!(unescape_text(source), expected),
            )
        })
        .collect();
    run_cases(SOURCE, FILE, "decodes_once", cases);
}
