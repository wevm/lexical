//! Ports of `packages/lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts`,
//! less its `IMPORT_AND_EXPORT` table, which the `markdown/import-export`
//! vector covers (rust/parity/skips.json).

use super::support::{Cases, j, run_cases};
use crate::caret::{CaretDirection, TextOffset, get_caret_range, get_text_point_caret};
use crate::caret_utils::set_selection_from_caret_range;
use crate::code::create_code_node;
use crate::constants::{TEXT_TYPE_TO_FORMAT, lookup};
use crate::link::{LinkAttributes, create_link_node, is_link_node};
use crate::list::{ListType, create_list_item_node, create_list_node, is_list_node};
use crate::markdown::{
    ElementTransformer, MultilineElementTransformer, RegExp, RegExpEnd, RegExpMatch,
    TextFormatTransformer, TextMatchTransformer, Transformer, check_list,
    convert_from_markdown_string, convert_selection_to_markdown_string, convert_to_markdown_string,
    generate_nodes_from_markdown_string, hard_line_break_state, heading, italic_star, link,
    list_marker_state, normalize_markdown, parse_markdown_hard_line_break,
    register_markdown_shortcuts, transformers,
};
use crate::nodes::line_break::create_line_break_node;
use crate::rich_text::{
    HeadingTagType, create_heading_node, create_quote_node, is_heading_node, is_quote_node,
};
use crate::upstream_tests::support::{update, update_with};
use crate::{
    COMPOSITION_END_TAG, CreateEditorArgs, Editor, KEY_ENTER_COMMAND, LexicalError, NodeClass,
    NodeRef, add_update_tag, copy_node, create_editor, create_paragraph_node, create_text_node,
    get_root, get_selection, get_state, set_composition_key, set_state,
};
use std::rc::Rc;

const FILE: &str = "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts";
const SOURCE: &str = include_str!("lexical_markdown.rs");

/// The nodes `createTestEditor` registers by default: heading, list, list
/// item, quote, code and link.
fn default_nodes() -> Vec<NodeClass> {
    [
        crate::rich_text::rich_text_nodes(),
        crate::list::list_nodes(),
        crate::code::code_nodes(),
        crate::link::link_nodes(),
    ]
    .concat()
}

/// `createTestEditor(config)`: a headless editor whose `onError` throws,
/// with the default nodes or `nodes`.
fn create_test_editor(nodes: Option<Vec<NodeClass>>) -> Editor {
    create_editor(CreateEditorArgs {
        nodes: nodes.unwrap_or_else(default_nodes),
        ..CreateEditorArgs::default()
    })
}

fn editor() -> Editor {
    create_test_editor(None)
}

fn read<R: 'static>(editor: &Editor, f: impl FnOnce() -> R) -> R {
    editor.read(f).unwrap()
}

/// `editor.read(() => $generateHtmlFromNodes(editor))`.
#[cfg(feature = "html")]
fn html(editor: &Editor) -> String {
    let e = editor.clone();
    read(editor, move || {
        crate::html::generate_html_from_nodes(&e, None).unwrap()
    })
}

/// `$convertFromMarkdownString(markdown, transformers, undefined,
/// shouldPreserveNewLines, shouldMergeAdjacentLines)` in a discrete update.
fn import_full(
    editor: &Editor,
    markdown: &str,
    transformers: &[Transformer],
    should_preserve_new_lines: bool,
    should_merge_adjacent_lines: bool,
) {
    let markdown = markdown.to_owned();
    let transformers = transformers.to_vec();
    update(editor, move || {
        convert_from_markdown_string(
            &markdown,
            &transformers,
            None,
            should_preserve_new_lines,
            should_merge_adjacent_lines,
        )
    });
}

/// `$convertFromMarkdownString(markdown, TRANSFORMERS)` in a discrete
/// update.
fn import(editor: &Editor, markdown: &str) {
    import_full(editor, markdown, &transformers(), false, false);
}

/// `editor.read(() => $convertToMarkdownString(transformers, undefined,
/// shouldPreserveNewLines))`.
fn export_full(
    editor: &Editor,
    transformers: &[Transformer],
    should_preserve_new_lines: bool,
) -> String {
    let transformers = transformers.to_vec();
    read(editor, move || {
        convert_to_markdown_string(&transformers, None, should_preserve_new_lines).unwrap()
    })
}

/// `editor.read(() => $convertToMarkdownString(TRANSFORMERS))`.
fn export(editor: &Editor) -> String {
    export_full(editor, &transformers(), false)
}

/// `editor.read(() => $convertSelectionToMarkdownString(TRANSFORMERS,
/// $getSelection()))`.
fn export_selection(editor: &Editor) -> String {
    read(editor, || {
        convert_selection_to_markdown_string(&transformers(), get_selection().as_ref(), false)
            .unwrap()
    })
}

fn format(name: &str) -> f64 {
    f64::from(lookup(TEXT_TYPE_TO_FORMAT, name).unwrap())
}

fn format_of(node: &NodeRef) -> f64 {
    node.text_data().get_format()
}

fn text(s: &str) -> NodeRef {
    create_text_node(s)
}

/// `$createTextNode(text).setFormat(format)`.
fn formatted(s: &str, f: f64) -> NodeRef {
    let node = create_text_node(s);
    node.set_format(f);
    node
}

fn with(node: NodeRef, children: Vec<NodeRef>) -> NodeRef {
    node.append(&children).unwrap();
    node
}

fn paragraph(children: Vec<NodeRef>) -> NodeRef {
    with(create_paragraph_node(), children)
}

fn link_node(url: &str, children: Vec<NodeRef>) -> NodeRef {
    with(create_link_node(url, &LinkAttributes::default()), children)
}

/// `$getRoot().clear().append(...nodes)`.
fn set_root(nodes: Vec<NodeRef>) -> Result<(), LexicalError> {
    let root = get_root();
    root.clear()?;
    root.append(&nodes)
}

/// Types each character of `text` at the selection in its own discrete
/// update.
fn type_chars(editor: &Editor, text: &str) {
    for ch in text.chars() {
        update(editor, move || {
            if let Some(selection) = get_selection().filter(|s| s.is_range()) {
                selection.insert_text(&ch.to_string())?;
            }
            Ok(())
        });
    }
}

/// A paragraph appended to the root, selected at its end, and `text`
/// inserted there in one discrete update.
fn insert_in_new_paragraph(editor: &Editor, text: &'static str) {
    update(editor, move || {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.select_end()?;
        if let Some(selection) = get_selection().filter(|s| s.is_range()) {
            selection.insert_text(text)?;
        }
        Ok(())
    });
}

fn dispatch_enter(editor: &Editor) {
    let e = editor.clone();
    update(editor, move || {
        e.dispatch_command(&KEY_ENTER_COMMAND, None)?;
        Ok(())
    });
}

fn case(name: String, f: impl FnOnce() + 'static) -> (String, Box<dyn FnOnce()>) {
    (name, Box::new(f))
}

/// `CANCELED_HEADING_REPLACE_EXAMPLE`: a heading element transformer whose
/// `replace` returns `false`.
fn canceled_heading_replace_example() -> Transformer {
    Transformer::Element(Rc::new(ElementTransformer {
        dependencies: vec!["heading"],
        export: Rc::new(|_, _, _| Ok(None)),
        reg_exp: RegExp::of(r"^(#{1,6})\s", ""),
        replace: Rc::new(|_, _, _, _| Ok(false)),
        trigger_on_enter: false,
    }))
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should not remove leading node and transform if replace returns false
#[cfg(feature = "html")]
#[test]
fn should_not_remove_leading_node_and_transform_if_replace_returns_false() {
    let editor = editor();
    let _unregister =
        register_markdown_shortcuts(&editor, &[canceled_heading_replace_example()]).unwrap();
    insert_in_new_paragraph(&editor, "#");
    type_chars(&editor, " ");
    assert_eq!(
        html(&editor),
        "<p><span style=\"white-space: pre-wrap;\"># </span></p>"
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should remove leading node and execute transform if replace does not return false
#[cfg(feature = "html")]
#[test]
fn should_remove_leading_node_and_execute_transform_if_replace_does_not_return_false() {
    let editor = editor();
    let _unregister =
        register_markdown_shortcuts(&editor, &[Transformer::Element(heading())]).unwrap();
    insert_in_new_paragraph(&editor, "#");
    type_chars(&editor, " ");
    assert_eq!(html(&editor), "<h1><br></h1>");
}

/// Types `shortcut` at the start of `Welcome to the playground` in the
/// block `block` makes, with the default transformers' shortcuts, and
/// checks the block is kept, as `is_block` tells, holding the typed text.
fn typed_shortcut_preserves(
    shortcut: &str,
    block: fn() -> NodeRef,
    is_block: fn(&NodeRef) -> bool,
) {
    let editor = editor();
    let _unregister = register_markdown_shortcuts(&editor, &transformers()).unwrap();
    update(&editor, move || {
        let t = text("Welcome to the playground");
        get_root().append(&[with(block(), vec![t.clone()])])?;
        t.select(Some(0), Some(0))?;
        Ok(())
    });
    type_chars(&editor, shortcut);
    let shortcut = shortcut.to_owned();
    read(&editor, move || {
        let first = get_root().get_first_child().unwrap();
        assert!(is_block(&first));
        assert_eq!(
            first.get_text_content(),
            format!("{shortcut}Welcome to the playground")
        );
    });
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a heading when typing the "1. " list shortcut
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a heading when typing the "- " list shortcut
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a heading when typing the "* " list shortcut
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a heading when typing the "+ " list shortcut
#[test]
fn should_preserve_a_heading_when_typing_the_list_shortcut() {
    let cases = ["1. ", "- ", "* ", "+ "]
        .into_iter()
        .map(|shortcut| {
            case(
                format!("Markdown > should preserve a heading when typing the \"{shortcut}\" list shortcut"),
                move || {
                    typed_shortcut_preserves(
                        shortcut,
                        || create_heading_node(HeadingTagType::H1),
                        is_heading_node,
                    );
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "should_preserve_a_heading_when_typing_the_list_shortcut",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a quote when typing the "# " shortcut (#7407)
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a quote when typing the "## " shortcut (#7407)
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a quote when typing the "###### " shortcut (#7407)
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a quote when typing the "1. " shortcut (#7407)
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a quote when typing the "- " shortcut (#7407)
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a quote when typing the "* " shortcut (#7407)
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a quote when typing the "+ " shortcut (#7407)
#[test]
fn should_preserve_a_quote_when_typing_the_shortcut() {
    let cases = ["# ", "## ", "###### ", "1. ", "- ", "* ", "+ "]
        .into_iter()
        .map(|shortcut| {
            case(
                format!("Markdown > should preserve a quote when typing the \"{shortcut}\" shortcut (#7407)"),
                move || {
                    typed_shortcut_preserves(shortcut, || create_quote_node(false), is_quote_node);
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "should_preserve_a_quote_when_typing_the_shortcut",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > should preserve a quote when the code fence shortcut is typed in it (#7407)
#[test]
fn should_preserve_a_quote_when_the_code_fence_shortcut_is_typed_in_it() {
    let editor = editor();
    let _unregister = register_markdown_shortcuts(&editor, &transformers()).unwrap();
    update(&editor, || {
        let t = text("");
        get_root().append(&[with(create_quote_node(false), vec![t.clone()])])?;
        t.select(Some(0), Some(0))?;
        Ok(())
    });
    type_chars(&editor, "```");
    editor.dispatch_command(&KEY_ENTER_COMMAND, None).unwrap();
    read(&editor, || {
        let quote = get_root().get_first_child().unwrap();
        assert!(is_quote_node(&quote));
        assert_eq!(quote.get_text_content(), "```");
    });
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > can round-trip nested fenced code blocks (4 backticks wrapping 3 backticks)
#[test]
fn can_round_trip_nested_fenced_code_blocks() {
    let markdown = "````markdown\n# Example\n\nRun this:\n```bash\nnpm install\n```\n````";
    let editor = editor();
    import(&editor, markdown);
    assert_eq!(export(&editor), markdown);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > can round-trip deeply nested fenced code blocks (5 backticks wrapping 4 backticks)
#[test]
fn can_round_trip_deeply_nested_fenced_code_blocks() {
    let markdown = "`````text\nTop Level 5 ticks\n````markdown\nLevel 2 (4 ticks)\n```javascript\nconsole.log(\"Deepest (3 ticks)\");\n```\n````\n`````";
    let editor = editor();
    import(&editor, markdown);
    assert_eq!(export(&editor), markdown);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > computes fence dynamically when code block content contains backticks
#[test]
fn computes_fence_dynamically_when_code_block_content_contains_backticks() {
    let editor = create_test_editor(Some(crate::code::code_nodes()));
    update(&editor, || {
        // A CodeNode without fence state (the default ```), whose content
        // holds ```.
        let code_block_node = create_code_node(Some("markdown"), None);
        get_root().append(&[code_block_node])?;
        get_root()
            .select_end()?
            .insert_raw_text("```js\nconsole.log(\"hello\");\n```")
    });
    // The fence is ```` since the content holds ```.
    assert_eq!(
        export(&editor),
        "````markdown\n```js\nconsole.log(\"hello\");\n```\n````"
    );
}

type Run = (String, f64);

fn runs(table: &[(&str, f64)]) -> Vec<Run> {
    table.iter().map(|(t, f)| ((*t).to_owned(), *f)).collect()
}

/// `$textRuns()`: the document as merged (text, format) runs.
fn text_runs() -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();
    for node in get_root().get_all_text_nodes() {
        let f = format_of(&node);
        let t = node.get_text_content();
        match out.last_mut() {
            Some(last) if last.1 == f => last.0.push_str(&t),
            _ => out.push((t, f)),
        }
    }
    out
}

/// `expectRoundTrip(runs, expected)`.
fn expect_round_trip(table: &[(&str, f64)], expected: &str) {
    let runs = runs(table);
    let editor = editor();
    let nodes = runs.clone();
    update(&editor, move || {
        set_root(vec![paragraph(
            nodes.iter().map(|(t, f)| formatted(t, *f)).collect(),
        )])
    });
    let markdown = export(&editor);
    assert_eq!(markdown, expected);
    let reimported = self::editor();
    import(&reimported, &markdown);
    assert_eq!(read(&reimported, text_runs), runs);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > overlapping inline formats (#4895) > round-trips bold overlapping italic (the issue example)
#[test]
fn round_trips_bold_overlapping_italic() {
    let (bold, italic) = (format("bold"), format("italic"));
    expect_round_trip(
        &[
            ("he", 0.0),
            ("llo", bold),
            ("wor", f64::from(bold as i32 | italic as i32)),
            ("ld", italic),
            ("!", 0.0),
        ],
        "he**llo*wor****ld*!",
    );
}

fn or(a: f64, b: f64) -> f64 {
    f64::from(a as i32 | b as i32)
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > overlapping inline formats (#4895) > round-trips italic overlapping bold
#[test]
fn round_trips_italic_overlapping_bold() {
    let (bold, italic) = (format("bold"), format("italic"));
    expect_round_trip(
        &[
            ("a", 0.0),
            ("b", italic),
            ("c", or(italic, bold)),
            ("d", bold),
            ("e", 0.0),
        ],
        "a*b**c*****d**e",
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > overlapping inline formats (#4895) > round-trips italic then bold italic between punctuation
#[test]
fn round_trips_italic_then_bold_italic_between_punctuation() {
    let (bold, italic) = (format("bold"), format("italic"));
    expect_round_trip(
        &[(")", italic), ("(", bold), (")", or(bold, italic))],
        "*)***(*)***",
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > overlapping inline formats (#4895) > round-trips strikethrough overlapping bold
#[test]
fn round_trips_strikethrough_overlapping_bold() {
    let (bold, strike) = (format("bold"), format("strikethrough"));
    expect_round_trip(
        &[("a", strike), ("b", or(strike, bold)), ("c", bold)],
        "~~a**b**~~**c**",
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > overlapping inline formats (#4895) > round-trips a code span inside a bold run
#[test]
fn round_trips_a_code_span_inside_a_bold_run() {
    let (bold, code) = (format("bold"), format("code"));
    expect_round_trip(
        &[("a", bold), ("b", or(bold, code)), ("c", bold)],
        "**a`b`c**",
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > overlapping inline formats (#4895) > imports a partially consumed delimiter run like CommonMark
#[test]
fn imports_a_partially_consumed_delimiter_run_like_commonmark() {
    let (bold, italic) = (format("bold"), format("italic"));
    let editor = editor();
    import(&editor, "he**llo*wor****ld*!");
    assert_eq!(
        read(&editor, text_runs),
        runs(&[
            ("he", 0.0),
            ("llo", bold),
            ("wor", or(bold, italic)),
            ("ld", italic),
            ("!", 0.0),
        ])
    );
}

type Char = (String, f64, Option<String>);

/// `importMarkdown(markdown)`.
fn import_markdown(markdown: &str) -> Editor {
    let editor = editor();
    import(&editor, markdown);
    editor
}

/// `$chars()`: the format and link URL of every non-whitespace character.
fn chars() -> Vec<Char> {
    let mut out = Vec::new();
    for node in get_root().get_all_text_nodes() {
        let url = node
            .get_parent()
            .filter(is_link_node)
            .and_then(|p| p.get_url());
        for ch in node.get_text_content().chars() {
            let s = ch.to_string();
            if !crate::js_trim(&s).is_empty() {
                out.push((s, format_of(&node), url.clone()));
            }
        }
    }
    out
}

fn root_text(editor: &Editor) -> String {
    read(editor, || get_root().get_text_content())
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > round-trips "**_a_ [b](u)**" with a stable export
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > round-trips "**_a_ [b](u) c**" with a stable export
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > round-trips "*a **b** [c](u)*" with a stable export
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > round-trips "**a [b](u)**" with a stable export
#[test]
fn round_trips_with_a_stable_export() {
    let cases = [
        "**_a_ [b](u)**",
        "**_a_ [b](u) c**",
        "*a **b** [c](u)*",
        "**a [b](u)**",
    ]
    .into_iter()
    .map(|original| {
        case(
            format!("Markdown > formatted whitespace-only text (#9403) > round-trips \"{original}\" with a stable export"),
            move || {
                let first = import_markdown(original);
                let chars_first = read(&first, chars);
                // Every delimiter was consumed and the link was parsed.
                assert!(!chars_first.iter().any(|(c, _, _)| c == "*" || c == "_"));
                assert!(chars_first.iter().any(|(_, _, url)| url.is_some()));
                let exported = export(&first);
                let second = import_markdown(&exported);
                assert_eq!(read(&second, chars), chars_first);
                assert_eq!(root_text(&second), root_text(&first));
                assert_eq!(export(&second), exported);
            },
        )
    })
    .collect();
    run_cases(SOURCE, FILE, "round_trips_with_a_stable_export", cases);
}

/// An expected export: exact, or a pattern.
enum Expected {
    Is(&'static str),
    Matches(&'static str),
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > closes before unformatted text
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > closes at the end of a link
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > closes before consecutive formatted spaces
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > closes before the trailing space of the text it formats
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > does not reopen a tag it kept open
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > keeps a tag open across consecutive formatted spaces
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > closes before a formatted line break
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > leaves the formats of inline code that holds only a space off
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > formatted whitespace-only text (#9403) > keeps a tag open across inline code that holds only a space
#[test]
fn formatted_whitespace_only_text_exports() {
    type Nodes = fn() -> Vec<NodeRef>;
    let table: [(&str, Nodes, Expected); 9] = [
        (
            "closes before unformatted text",
            || {
                vec![
                    formatted("a", or(format("bold"), format("italic"))),
                    formatted(" ", format("bold")),
                    formatted("b", 0.0),
                ]
            },
            Expected::Is("***a*** b"),
        ),
        (
            "closes at the end of a link",
            || {
                vec![
                    link_node(
                        "u",
                        vec![
                            formatted("a", or(format("bold"), format("italic"))),
                            formatted(" ", format("bold")),
                        ],
                    ),
                    formatted("c", 0.0),
                ]
            },
            Expected::Is("[***a*** ](u)c"),
        ),
        (
            "closes before consecutive formatted spaces",
            || {
                vec![
                    formatted("a", format("bold")),
                    formatted(" ", or(format("bold"), format("italic"))),
                    formatted(" ", or(format("bold"), format("strikethrough"))),
                    formatted("c", 0.0),
                ]
            },
            Expected::Is("**a**  c"),
        ),
        (
            "closes before the trailing space of the text it formats",
            || {
                vec![
                    formatted("a ", format("bold")),
                    formatted(" ", or(format("bold"), format("italic"))),
                    formatted("c", 0.0),
                ]
            },
            Expected::Is("**a**  c"),
        ),
        (
            "does not reopen a tag it kept open",
            || {
                vec![
                    formatted("a", format("bold")),
                    formatted(" ", or(format("bold"), format("italic"))),
                    formatted("c", format("bold")),
                ]
            },
            Expected::Is("**a c**"),
        ),
        (
            "keeps a tag open across consecutive formatted spaces",
            || {
                vec![
                    formatted("a", format("bold")),
                    formatted(" ", or(format("bold"), format("italic"))),
                    formatted(" ", or(format("bold"), format("strikethrough"))),
                    formatted("c", format("bold")),
                ]
            },
            Expected::Is("**a  c**"),
        ),
        (
            "closes before a formatted line break",
            || {
                vec![
                    formatted("a", format("bold")),
                    formatted("\n", or(format("bold"), format("italic"))),
                    formatted("c", format("bold")),
                ]
            },
            Expected::Is("**a**\n**c**"),
        ),
        (
            "leaves the formats of inline code that holds only a space off",
            || {
                vec![
                    formatted("a", 0.0),
                    formatted(" ", or(format("bold"), format("code"))),
                    formatted("b", 0.0),
                ]
            },
            Expected::Matches("^a` +`b$"),
        ),
        (
            "keeps a tag open across inline code that holds only a space",
            || {
                vec![
                    formatted("a", format("bold")),
                    formatted(" ", or(format("bold"), format("code"))),
                    formatted("b", format("bold")),
                ]
            },
            Expected::Matches(r"^\*\*a` +`b\*\*$"),
        ),
    ];
    let cases = table
        .into_iter()
        .map(|(name, nodes, expected)| {
            case(
                format!("Markdown > formatted whitespace-only text (#9403) > {name}"),
                move || {
                    let editor = editor();
                    update(&editor, move || set_root(vec![paragraph(nodes())]));
                    let exported = export(&editor);
                    match expected {
                        Expected::Is(s) => assert_eq!(exported, s),
                        Expected::Matches(p) => {
                            assert!(RegExp::of(p, "").test(&exported), "{exported:?} !~ {p}");
                        }
                    }
                    assert_eq!(
                        read(&import_markdown(&exported), chars),
                        read(&editor, chars)
                    );
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "formatted_whitespace_only_text_exports",
        cases,
    );
}

fn list_nodes_editor() -> Editor {
    create_test_editor(Some(crate::list::list_nodes()))
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > list marker > should remember marker used on import
#[test]
fn should_remember_marker_used_on_import() {
    let editor = list_nodes_editor();
    import_full(&editor, "+ hello", &transformers(), true, false);
    read(&editor, || {
        let node = get_root().get_first_child().unwrap();
        assert!(is_list_node(&node));
        assert_eq!(get_state(&node, &list_marker_state()).unwrap(), "+");
    });
}

/// Types `typed` then a space in a fresh paragraph with `CHECK_LIST`'s
/// shortcut, and returns the list marker state of the root's first child.
fn check_list_marker_after_typing(typed: &'static str) -> String {
    let editor = list_nodes_editor();
    let _unregister =
        register_markdown_shortcuts(&editor, &[Transformer::Element(check_list())]).unwrap();
    update(&editor, move || {
        set_root(vec![create_paragraph_node()])?;
        get_root().select_end()?.insert_text(typed)
    });
    update(&editor, || get_selection().unwrap().insert_text(" "));
    read(&editor, || {
        let node = get_root().get_first_child().unwrap();
        assert!(is_list_node(&node));
        get_state(&node, &list_marker_state()).unwrap()
    })
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > list marker > should not use [ as a marker for an implicit check list
#[test]
fn should_not_use_bracket_as_a_marker_for_an_implicit_check_list() {
    assert_eq!(check_list_marker_after_typing("[]"), "-");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > list marker > should remember the marker for checkbox with an explicit marker
#[test]
fn should_remember_the_marker_for_checkbox_with_an_explicit_marker() {
    assert_eq!(check_list_marker_after_typing("* []"), "*");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > list marker > should remember marker used on export
#[test]
fn should_remember_marker_used_on_export() {
    let editor = list_nodes_editor();
    update(&editor, || {
        let list_node = create_list_node(ListType::Number, 1.0);
        set_state(&list_node, &list_marker_state(), "+".to_owned())?;
        let list_item_node = with(create_list_item_node(None), vec![text("hello")]);
        list_node.append(&[list_item_node])?;
        list_node.set_list_type(ListType::Bullet);
        get_root().select(None, None)?;
        crate::insert_nodes(vec![list_node])
    });
    assert_eq!(export_full(&editor, &transformers(), true), "+ hello");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > Enter key triggers > should create an empty code block when ``` is typed and Enter is pressed
#[cfg(feature = "html")]
#[test]
fn should_create_an_empty_code_block_when_fence_is_typed_and_enter_is_pressed() {
    let editor = editor();
    let _unregister = register_markdown_shortcuts(&editor, &transformers()).unwrap();
    insert_in_new_paragraph(&editor, "```");
    dispatch_enter(&editor);
    assert_eq!(html(&editor), "<pre spellcheck=\"false\"></pre>");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > Enter key triggers > should create a code block with language when ```javascript is typed and Enter is pressed
#[cfg(feature = "html")]
#[test]
fn should_create_a_code_block_with_language_when_fence_and_language_is_typed_and_enter_is_pressed()
{
    let editor = editor();
    let _unregister = register_markdown_shortcuts(&editor, &transformers()).unwrap();
    insert_in_new_paragraph(&editor, "```javascript");
    dispatch_enter(&editor);
    assert_eq!(
        html(&editor),
        "<pre spellcheck=\"false\" data-language=\"javascript\"></pre>"
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > Enter key triggers > should not transform on Enter when replace returns false
#[cfg(feature = "html")]
#[test]
fn should_not_transform_on_enter_when_replace_returns_false() {
    let canceled_code = Transformer::MultilineElement(Rc::new(MultilineElementTransformer {
        handle_import_after_start_match: None,
        dependencies: vec!["code"],
        export: None,
        reg_exp_start: RegExp::of(r"^`{3,}(\w+)?", ""),
        reg_exp_end: Some(RegExpEnd {
            optional: true,
            reg_exp: RegExp::of(r"^[ \t]*`{3,}$", ""),
        }),
        replace: Rc::new(|_, _, _, _, _, _| Ok(false)),
    }));
    let editor = editor();
    let _unregister = register_markdown_shortcuts(&editor, &[canceled_code]).unwrap();
    insert_in_new_paragraph(&editor, "```");
    dispatch_enter(&editor);
    assert_eq!(
        html(&editor),
        "<p><span style=\"white-space: pre-wrap;\">```</span></p>"
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > Enter key triggers > should transform element markdown on Enter when trailing space was not required (custom element transformer)
#[cfg(feature = "html")]
#[test]
fn should_transform_element_markdown_on_enter_when_trailing_space_was_not_required() {
    let element_triggered_fence = Transformer::Element(Rc::new(ElementTransformer {
        dependencies: vec!["code"],
        export: Rc::new(|_, _, _| Ok(None)),
        reg_exp: RegExp::of(r"^`{3,}(\w+)?$", ""),
        replace: Rc::new(
            |parent_node: &NodeRef, children: &[NodeRef], m: &RegExpMatch, is_import: bool| {
                let node = create_code_node(m.groups.get(1).cloned().flatten().as_deref(), None);
                node.append(children)?;
                parent_node.replace(&node, false)?;
                if !is_import {
                    node.select(Some(0), Some(0))?;
                }
                Ok(true)
            },
        ),
        trigger_on_enter: true,
    }));
    let editor = editor();
    let _unregister = register_markdown_shortcuts(&editor, &[element_triggered_fence]).unwrap();
    insert_in_new_paragraph(&editor, "```");
    dispatch_enter(&editor);
    assert_eq!(html(&editor), "<pre spellcheck=\"false\"></pre>");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > Enter key triggers > should transform heading on Enter when a line was inserted at once (no trailing space listener trigger)
#[cfg(feature = "html")]
#[test]
fn should_transform_heading_on_enter_when_a_line_was_inserted_at_once() {
    let editor = editor();
    let _unregister =
        register_markdown_shortcuts(&editor, &[Transformer::Element(heading())]).unwrap();
    insert_in_new_paragraph(&editor, "## ");
    dispatch_enter(&editor);
    assert_eq!(html(&editor), "<h2><br></h2>");
}

/// `$getTextNode()`: the paragraph's first child, a text node.
fn first_text_node() -> NodeRef {
    let paragraph = get_root().get_first_child_or_throw().unwrap();
    assert_eq!(
        paragraph.get_type(),
        "paragraph",
        "expected ParagraphNode at root[0]"
    );
    let text_node = paragraph.get_first_child_or_throw().unwrap();
    assert!(text_node.is_text(), "expected TextNode at paragraph[0]");
    text_node
}

/// The composition tests' editor, holding "`hello" with the caret at its
/// end.
fn composition_editor() -> Editor {
    let editor = editor();
    let _unregister = register_markdown_shortcuts(&editor, &transformers()).unwrap();
    update(&editor, || {
        let p = create_paragraph_node();
        get_root().append(std::slice::from_ref(&p))?;
        p.append(&[text("`hello")])?;
        p.select_end()?;
        Ok(())
    });
    editor
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > composition-end trigger characters (#7026) > applies inline code when the closing backtick is committed via compositionend
#[cfg(feature = "html")]
#[test]
fn applies_inline_code_when_the_closing_backtick_is_committed_via_compositionend() {
    let editor = composition_editor();
    // compositionupdate in progress: the backtick is in the text, the caret
    // after it, and the editor composing.
    update(&editor, || {
        let text_node = first_text_node();
        set_composition_key(Some(text_node.get_key()));
        text_node.set_text_content("`hello`");
        text_node.select(Some(7), Some(7))?;
        Ok(())
    });
    // compositionend's flush.
    update(&editor, || {
        set_composition_key(None);
        add_update_tag(COMPOSITION_END_TAG);
        first_text_node().mark_dirty();
        Ok(())
    });
    assert_eq!(
        html(&editor),
        "<p><code spellcheck=\"false\" style=\"white-space: pre-wrap;\"><span>hello</span></code></p>"
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Markdown > composition-end trigger characters (#7026) > applies inline code when compositionend commits the trigger as a multi-character chunk
#[cfg(feature = "html")]
#[test]
fn applies_inline_code_when_compositionend_commits_the_trigger_as_a_multi_character_chunk() {
    let editor = composition_editor();
    update(&editor, || {
        set_composition_key(Some(first_text_node().get_key()));
        Ok(())
    });
    update(&editor, || {
        set_composition_key(None);
        add_update_tag(COMPOSITION_END_TAG);
        let text_node = first_text_node();
        text_node.set_text_content("`helloa`");
        text_node.select(Some(8), Some(8))?;
        Ok(())
    });
    assert_eq!(
        html(&editor),
        "<p><code spellcheck=\"false\" style=\"white-space: pre-wrap;\"><span>helloa</span></code></p>"
    );
}

/// `HIGHLIGHT_TEXT_MATCH_IMPORT`: `LINK` importing `$text$` as highlighted
/// text.
fn highlight_text_match_import() -> Transformer {
    Transformer::TextMatch(Rc::new(TextMatchTransformer {
        import_reg_exp: Some(RegExp::of(r"\$([^$]+?)\$", "")),
        replace: Some(Rc::new(|text_node: &NodeRef, _m: &RegExpMatch| {
            text_node.toggle_format("highlight");
            Ok(None)
        })),
        ..(*link()).clone()
    }))
}

const MERGE_SOURCE: &str = "
A1
A2

A3

```md
B1
B2

B3
```

C1
C2

C3

```js
D1
D2

D3
```

```single line code```

E1
E2

E3
";

// upstream: "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > should combine lines separated by a single \n unless they are in a codeblock"
#[test]
fn should_combine_lines_separated_by_a_single_newline_unless_they_are_in_a_codeblock() {
    assert_eq!(
        normalize_markdown(MERGE_SOURCE, true),
        "
A1 A2

A3

```md
B1
B2

B3
```

C1 C2

C3

```js
D1
D2

D3
```

```single line code```

E1 E2

E3
"
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > keeps fence-like lines that carry an info string as code content
#[test]
fn keeps_fence_like_lines_that_carry_an_info_string_as_code_content() {
    let markdown = "\n```ts\nconst a = 1\n```js\nconst b = 2\n```\n\nAfter\n";
    assert_eq!(normalize_markdown(markdown, true), markdown);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > does not close a longer fence on a shorter inner fence
#[test]
fn does_not_close_a_longer_fence_on_a_shorter_inner_fence() {
    let markdown = "\n````\n```\ninner\n```\n````\n";
    assert_eq!(normalize_markdown(markdown, true), markdown);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > closes a shorter opening fence with a longer closing fence
#[test]
fn closes_a_shorter_opening_fence_with_a_longer_closing_fence() {
    let markdown = "```\nline one\nline two\n````\nafter one\nafter two";
    assert_eq!(
        normalize_markdown(markdown, true),
        "```\nline one\nline two\n````\nafter one after two"
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > keeps content unmerged in an unclosed code block
#[test]
fn keeps_content_unmerged_in_an_unclosed_code_block() {
    let markdown = "```\nline one\nline two";
    assert_eq!(normalize_markdown(markdown, true), markdown);
}

const TABLE: &str = "\n| a | b |\n| --- | --- |\n| c | d |\n";

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > tables
#[test]
fn normalize_merge_tables() {
    assert_eq!(normalize_markdown(TABLE, true), TABLE);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > merges adjacent plain text lines with a single space
#[test]
fn merges_adjacent_plain_text_lines_with_a_single_space() {
    assert_eq!(normalize_markdown("Hello\nworld", true), "Hello world");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > merges while trimming the next line and inserting a single space
#[test]
fn merges_while_trimming_the_next_line_and_inserting_a_single_space() {
    assert_eq!(
        normalize_markdown("Hello\n   world   ", true),
        "Hello world"
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > does not merge across HTML-like tags (opening, content, closing, after)
#[test]
fn does_not_merge_across_html_like_tags() {
    let md = "<div>\ncontent\n</div>\nafter";
    assert_eq!(normalize_markdown(md, true), md);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > does not merge the fence line with the first line after a code block
#[test]
fn does_not_merge_the_fence_line_with_the_first_line_after_a_code_block() {
    assert_eq!(
        normalize_markdown("```\ncode\n```\nNext line", true),
        "```\ncode\n```\nNext line"
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > preserves hard-break trailing spaces when merging adjacent lines
#[test]
fn preserves_hard_break_trailing_spaces_when_merging_adjacent_lines() {
    let md = "foo  \nbar";
    assert_eq!(normalize_markdown(md, true), md);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > preserves exact hard-break trailing spaces when merging adjacent lines
#[test]
fn preserves_exact_hard_break_trailing_spaces_when_merging_adjacent_lines() {
    let md = "foo   \nbar";
    assert_eq!(normalize_markdown(md, true), md);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > preserves backslash hard-breaks when merging adjacent lines
#[test]
fn preserves_backslash_hard_breaks_when_merging_adjacent_lines() {
    let md = "foo\\\nbar";
    assert_eq!(normalize_markdown(md, true), md);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > merges a soft break before a hard-breaking line
#[test]
fn merges_a_soft_break_before_a_hard_breaking_line() {
    assert_eq!(
        normalize_markdown("foo\nbar  \nbaz", true),
        "foo bar  \nbaz"
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > treats whitespace-only lines as empty separators (no merge across them)
#[test]
fn treats_whitespace_only_lines_as_empty_separators() {
    assert_eq!(normalize_markdown("A1\n     \nA2", true), "A1\n\nA2");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = true > mdx start tag followed by content, than closing tag preceded by content
#[test]
fn mdx_start_tag_followed_by_content_than_closing_tag_preceded_by_content() {
    assert_eq!(
        normalize_markdown("<MyComponent>Line 1\nSome Text</MyComponent>", true),
        "<MyComponent>Line 1 Some Text</MyComponent>"
    );
}

// upstream: "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = false > should not combine lines separated by a single \n"
#[test]
fn should_not_combine_lines_separated_by_a_single_newline() {
    assert_eq!(normalize_markdown(MERGE_SOURCE, false), MERGE_SOURCE);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = false > tables
#[test]
fn normalize_tables() {
    assert_eq!(normalize_markdown(TABLE, false), TABLE);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = false > preserves trailing whitespace on content lines
#[test]
fn preserves_trailing_whitespace_on_content_lines() {
    assert_eq!(normalize_markdown("foo   \n\nbar", false), "foo   \n\nbar");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = false > collapses whitespace-only lines to empty (paragraph separator)
#[test]
fn collapses_whitespace_only_lines_to_empty() {
    assert_eq!(normalize_markdown("A\n   \nB", false), "A\n\nB");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = false > preserves leading whitespace on content lines
#[test]
fn preserves_leading_whitespace_on_content_lines() {
    assert_eq!(normalize_markdown("   foo\n\nbar", false), "   foo\n\nbar");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::normalizeMarkdown - shouldMergeAdjacentLines = false > preserves indented fenced code blocks nested inside tags
#[test]
fn preserves_indented_fenced_code_blocks_nested_inside_tags() {
    let markdown = "\n<Banner>\n```ts\n  indent 1;\n```\n</Banner>\n";
    assert_eq!(normalize_markdown(markdown, false), markdown);
}

/// Imports and exports `md` with `[...TRANSFORMERS,
/// HIGHLIGHT_TEXT_MATCH_IMPORT]`, preserving new lines.
fn preserved_round_trip(md: &str) -> String {
    let mut all = transformers();
    all.push(highlight_text_match_import());
    let editor = editor();
    import_full(&editor, md, &all, true, false);
    export_full(&editor, &all, true)
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown hard line break import > preserves hard line break when shouldPreserveNewLines is true
#[test]
fn preserves_hard_line_break_when_should_preserve_new_lines_is_true() {
    let md = "foo  \nbar";
    assert_eq!(preserved_round_trip(md), md);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown hard line break import > preserves backslash hard line break when shouldPreserveNewLines is true
#[test]
fn preserves_backslash_hard_line_break_when_should_preserve_new_lines_is_true() {
    let md = "foo\\\nbar";
    assert_eq!(preserved_round_trip(md), md);
}

/// `[...TRANSFORMERS, HIGHLIGHT_TEXT_MATCH_IMPORT]`.
fn with_highlight() -> Vec<Transformer> {
    let mut all = transformers();
    all.push(highlight_text_match_import());
    all
}

/// The whitespace tests' `expectRoundTrip(md, shouldMergeAdjacentLines)`.
fn expect_whitespace_round_trip(md: &str, should_merge_adjacent_lines: bool) {
    let editor = editor();
    import_full(
        &editor,
        md,
        &with_highlight(),
        false,
        should_merge_adjacent_lines,
    );
    assert_eq!(export_full(&editor, &with_highlight(), false), md);
}

/// `getHardLineBreakMarker(line)`.
fn get_hard_line_break_marker(line: &str) -> String {
    parse_markdown_hard_line_break(line)
        .expect("Expected a hard line break marker")
        .1
}

/// The first block's line break and the block, after importing `md`.
fn imported_line_break(editor: &Editor, check: impl FnOnce(&NodeRef, &NodeRef) + 'static) {
    read(editor, move || {
        let block = get_root().get_first_child_or_throw().unwrap();
        assert!(block.is_element(), "Expected an element block");
        let line_break = block
            .get_children()
            .into_iter()
            .find(|c| c.get_type() == "linebreak")
            .expect("Expected a line break node");
        check(&block, &line_break);
    });
}

/// `expectImportedHardLineBreakMarker(md, marker)`.
fn expect_imported_hard_line_break_marker(md: &str, marker: &'static str) {
    let editor = editor();
    import_full(&editor, md, &with_highlight(), false, false);
    imported_line_break(&editor, move |block, line_break| {
        assert!(
            !block
                .get_children()
                .iter()
                .filter(|c| c.is_text())
                .any(|c| c.get_text_content() == marker)
        );
        assert_eq!(block.get_text_content(), "foo\nbar");
        assert_eq!(
            get_state(line_break, &hard_line_break_state()).unwrap(),
            marker
        );
    });
    assert_eq!(export_full(&editor, &with_highlight(), false), md);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves trailing whitespace on a standalone paragraph line (default mode)
#[test]
fn preserves_trailing_whitespace_on_a_standalone_paragraph_line() {
    expect_whitespace_round_trip("hello world   \n\nnext paragraph", false);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves leading whitespace on a standalone paragraph line (default mode)
#[test]
fn preserves_leading_whitespace_on_a_standalone_paragraph_line() {
    expect_whitespace_round_trip("   hello world\n\nnext paragraph", false);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves two-space hard line break in default mode
#[test]
fn preserves_two_space_hard_line_break_in_default_mode() {
    expect_whitespace_round_trip("foo  \nbar", false);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves exact hard line break spaces in default mode
#[test]
fn preserves_exact_hard_line_break_spaces_in_default_mode() {
    expect_whitespace_round_trip("foo   \nbar", false);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves backslash hard line break in default mode
#[test]
fn preserves_backslash_hard_line_break_in_default_mode() {
    expect_whitespace_round_trip("foo\\\nbar", false);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > stores a two-space hard line break marker after formatted text
#[test]
fn stores_a_two_space_hard_line_break_marker_after_formatted_text() {
    expect_imported_hard_line_break_marker("**foo**  \nbar", "  ");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > stores exact hard line break spaces after formatted text
#[test]
fn stores_exact_hard_line_break_spaces_after_formatted_text() {
    expect_imported_hard_line_break_marker("**foo**   \nbar", "   ");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > stores a two-space hard line break marker after a link
#[test]
fn stores_a_two_space_hard_line_break_marker_after_a_link() {
    expect_imported_hard_line_break_marker("[foo](https://lexical.dev)  \nbar", "  ");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > does not infer a hard line break marker from spaces inside a link
#[test]
fn does_not_infer_a_hard_line_break_marker_from_spaces_inside_a_link() {
    let md = "[foo  ](https://lexical.dev)\nbar";
    let editor = editor();
    import_full(&editor, md, &with_highlight(), false, false);
    imported_line_break(&editor, |block, line_break| {
        assert_eq!(block.get_text_content(), "foo  \nbar");
        assert_eq!(get_state(line_break, &hard_line_break_state()).unwrap(), "");
    });
    assert_eq!(export_full(&editor, &with_highlight(), false), md);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > stores a two-space hard line break marker after formatted blockquote text
#[test]
fn stores_a_two_space_hard_line_break_marker_after_formatted_blockquote_text() {
    expect_imported_hard_line_break_marker("> **foo**  \n> bar", "  ");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves two-space hard line break in merge-adjacent mode
#[test]
fn preserves_two_space_hard_line_break_in_merge_adjacent_mode() {
    expect_whitespace_round_trip("foo  \nbar", true);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves exact hard line break spaces in merge-adjacent mode
#[test]
fn preserves_exact_hard_line_break_spaces_in_merge_adjacent_mode() {
    expect_whitespace_round_trip("foo   \nbar", true);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves backslash hard line break in merge-adjacent mode
#[test]
fn preserves_backslash_hard_line_break_in_merge_adjacent_mode() {
    expect_whitespace_round_trip("foo\\\nbar", true);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves two-space hard line break between blockquote lines
#[test]
fn preserves_two_space_hard_line_break_between_blockquote_lines() {
    expect_whitespace_round_trip("> foo  \n> bar", false);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves exact hard line break spaces between blockquote lines
#[test]
fn preserves_exact_hard_line_break_spaces_between_blockquote_lines() {
    expect_whitespace_round_trip("> foo   \n> bar", false);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > preserves backslash hard line break between blockquote lines
#[test]
fn preserves_backslash_hard_line_break_between_blockquote_lines() {
    expect_whitespace_round_trip("> foo\\\n> bar", false);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > exports exact hard line break markers from line break state
#[test]
fn exports_exact_hard_line_break_markers_from_line_break_state() {
    let editor = editor();
    let marker = get_hard_line_break_marker("foo   ");
    update(&editor, move || {
        let line_break = create_line_break_node();
        set_state(&line_break, &hard_line_break_state(), marker)?;
        set_root(vec![paragraph(vec![text("foo"), line_break, text("bar")])])
    });
    assert_eq!(
        export_full(&editor, &with_highlight(), false),
        "foo   \nbar"
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown whitespace import (default mode) > does not copy markdown hard line break markers when line breaks are copied
#[test]
fn does_not_copy_markdown_hard_line_break_markers_when_line_breaks_are_copied() {
    let editor = editor();
    let marker = get_hard_line_break_marker("foo   ");
    update(&editor, move || {
        let line_break = create_line_break_node();
        set_state(&line_break, &hard_line_break_state(), marker)?;
        assert_eq!(get_state(&line_break, &hard_line_break_state())?, "   ");
        let copy = copy_node(&line_break);
        assert_eq!(get_state(&copy, &hard_line_break_state())?, "");
        assert_eq!(get_state(&line_break, &hard_line_break_state())?, "   ");
        Ok(())
    });
}

/// `roundtrip(md)`: imports and exports with `TRANSFORMERS`.
fn roundtrip(md: &str) -> String {
    let editor = editor();
    import(&editor, md);
    export(&editor)
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown Safari compatibility (issue #8012) > does not throw when constructing markdown regex patterns
#[test]
fn does_not_throw_when_constructing_markdown_regex_patterns() {
    let editor = editor();
    editor
        .update(
            || convert_from_markdown_string("`hello`", &transformers(), None, false, false),
            crate::UpdateOptions {
                discrete: true,
                ..crate::UpdateOptions::default()
            },
        )
        .unwrap();
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown Safari compatibility (issue #8012) > parses a code span at the start of a string
#[test]
fn parses_a_code_span_at_the_start_of_a_string() {
    assert_eq!(roundtrip("`hello` world"), "`hello` world");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown Safari compatibility (issue #8012) > parses a code span in the middle of text
#[test]
fn parses_a_code_span_in_the_middle_of_text() {
    assert_eq!(roundtrip("foo `code` bar"), "foo `code` bar");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown Safari compatibility (issue #8012) > parses multiple code spans on the same line
#[test]
fn parses_multiple_code_spans_on_the_same_line() {
    assert_eq!(roundtrip("`a` and `b`"), "`a` and `b`");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown Safari compatibility (issue #8012) > does not parse a backtick preceded by a backslash as a code span
#[test]
fn does_not_parse_a_backtick_preceded_by_a_backslash_as_a_code_span() {
    let editor = editor();
    import(&editor, "\\`not code\\`");
    assert_eq!(root_text(&editor), "`not code`");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown Safari compatibility (issue #8012) > correctly captures code span content
#[test]
fn correctly_captures_code_span_content() {
    assert_eq!(roundtrip("`hello world`"), "`hello world`");
    assert_eq!(roundtrip("foo `bar baz` qux"), "foo `bar baz` qux");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::markdown Safari compatibility (issue #8012) > does not apply emphasis formatting inside a code span
#[test]
fn does_not_apply_emphasis_formatting_inside_a_code_span() {
    assert_eq!(roundtrip("`**not bold**`"), "`**not bold**`");
}

/// `exportCodeSpan(content)`.
fn export_code_span(content: &str) -> String {
    let editor = editor();
    let content = content.to_owned();
    update(&editor, move || {
        let t = text(&content);
        t.toggle_format("code");
        get_root().append(&[paragraph(vec![t])])
    });
    export(&editor)
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::inline code with backticks (CommonMark code spans) > round-trips code spans whose content contains backticks
#[test]
fn round_trips_code_spans_whose_content_contains_backticks() {
    assert_eq!(roundtrip("Here: `` a`b ``"), "Here: `` a`b ``");
    assert_eq!(roundtrip("Code: ``` a``b ```"), "Code: ``` a``b ```");
    assert_eq!(roundtrip("`` `x ``"), "`` `x ``");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::inline code with backticks (CommonMark code spans) > exports a content-derived fence longer than any backtick run
#[test]
fn exports_a_content_derived_fence_longer_than_any_backtick_run() {
    assert_eq!(export_code_span("block code"), "`block code`");
    assert_eq!(export_code_span("a`b"), "`` a`b ``");
    assert_eq!(export_code_span("a``b"), "``` a``b ```");
    assert_eq!(export_code_span("`x"), "`` `x ``");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::inline code with backticks (CommonMark code spans) > does not pad a code span made only of spaces
#[test]
fn does_not_pad_a_code_span_made_only_of_spaces() {
    assert_eq!(export_code_span(" "), "` `");
    assert_eq!(export_code_span("   "), "`   `");
    assert_eq!(roundtrip("a ` ` b"), "a ` ` b");
    assert_eq!(roundtrip("a `   ` b"), "a `   ` b");
    assert_eq!(export_code_span(" x "), "`  x  `");
    assert_eq!(export_code_span("\tx\t"), "` \tx\t `");
    assert_eq!(export_code_span(" \t "), "`  \t  `");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::inline code with backticks (CommonMark code spans) > normalizes a redundant inline fence to the minimal valid fence
#[test]
fn normalizes_a_redundant_inline_fence_to_the_minimal_valid_fence() {
    assert_eq!(roundtrip("a `block code` b"), "a `block code` b");
    assert_eq!(roundtrip("a ```block code``` b"), "a `block code` b");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::inline code with backticks (CommonMark code spans) > keeps the code fence innermost when combined with bold
#[test]
fn keeps_the_code_fence_innermost_when_combined_with_bold() {
    assert_eq!(roundtrip("**`` a`b ``**"), "**`` a`b ``**");
}

/// `$setSelectionFromCaretRange($getCaretRange($getTextPointCaret(anchor,
/// 'next', anchorOffset), $getTextPointCaret(focus, 'next',
/// focusOffset)))`.
fn select_carets(
    anchor: &NodeRef,
    anchor_offset: impl Into<TextOffset>,
    focus: &NodeRef,
    focus_offset: impl Into<TextOffset>,
) -> Result<(), LexicalError> {
    set_selection_from_caret_range(&get_caret_range(
        get_text_point_caret(anchor, CaretDirection::Next, anchor_offset)?,
        get_text_point_caret(focus, CaretDirection::Next, focus_offset)?,
    )?)?;
    Ok(())
}

/// A paragraph of `Hello World` selected from `anchor` to `focus`
/// (`None` is the end), exported as markdown.
fn export_hello_world_selection(anchor: usize, focus: Option<usize>) -> String {
    let editor = editor();
    update(&editor, move || {
        let t = text("Hello World");
        get_root().append(&[paragraph(vec![t.clone()])])?;
        t.select(Some(anchor), focus)?;
        Ok(())
    });
    export_selection(&editor)
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > converts full selection to markdown
#[test]
fn converts_full_selection_to_markdown() {
    assert_eq!(export_hello_world_selection(0, None), "Hello World");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > converts partial text selection to markdown
#[test]
fn converts_partial_text_selection_to_markdown() {
    assert_eq!(export_hello_world_selection(6, Some(11)), "World");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > converts selection with bold text to markdown
#[test]
fn converts_selection_with_bold_text_to_markdown() {
    let editor = editor();
    update(&editor, || {
        let normal = text("Hello ");
        let bold = text("Bold");
        bold.toggle_format("bold");
        let after = text(" World");
        get_root().append(&[paragraph(vec![normal.clone(), bold.clone(), after])])?;
        select_carets(&normal, 0, &bold, 4)
    });
    assert_eq!(export_selection(&editor), "Hello **Bold**");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > does not prefix a newline when the selection starts after the first block
#[test]
fn does_not_prefix_a_newline_when_the_selection_starts_after_the_first_block() {
    let editor = editor();
    update(&editor, || {
        let first = text("First");
        let second = text("Second");
        let third = text("Third");
        get_root().append(&[
            paragraph(vec![first]),
            paragraph(vec![second.clone()]),
            paragraph(vec![third.clone()]),
        ])?;
        select_carets(&second, 0, &third, 5)
    });
    assert_eq!(export_selection(&editor), "Second\n\nThird");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > returns empty string for null selection
#[test]
fn returns_empty_string_for_null_selection() {
    assert_eq!(
        convert_selection_to_markdown_string(&transformers(), None, false).unwrap(),
        ""
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > returns empty string for collapsed selection
#[test]
fn returns_empty_string_for_collapsed_selection() {
    assert_eq!(export_hello_world_selection(5, Some(5)), "");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > converts backward selection to markdown
#[test]
fn converts_backward_selection_to_markdown() {
    assert_eq!(export_hello_world_selection(5, Some(0)), "Hello");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > converts multi-paragraph selection to markdown
#[test]
fn converts_multi_paragraph_selection_to_markdown() {
    let editor = editor();
    update(&editor, || {
        let t1 = text("First paragraph");
        let t2 = text("Second paragraph");
        get_root().append(&[paragraph(vec![t1.clone()]), paragraph(vec![t2.clone()])])?;
        select_carets(&t1, 0, &t2, CaretDirection::Next)
    });
    assert_eq!(
        export_selection(&editor),
        "First paragraph\n\nSecond paragraph"
    );
}

fn item(children: Vec<NodeRef>) -> NodeRef {
    with(create_list_item_node(None), children)
}

fn bullet_list(items: Vec<NodeRef>) -> NodeRef {
    with(create_list_node(ListType::Bullet, 1.0), items)
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > converts selection within a list to markdown
#[test]
fn converts_selection_within_a_list_to_markdown() {
    let editor = editor();
    update(&editor, || {
        let t1 = text("Item 1");
        let t2 = text("Item 2");
        get_root().append(&[bullet_list(vec![
            item(vec![t1.clone()]),
            item(vec![t2.clone()]),
        ])])?;
        select_carets(&t1, 0, &t2, CaretDirection::Next)
    });
    assert_eq!(export_selection(&editor), "- Item 1\n- Item 2");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > preserves link when partially selecting link text
#[test]
fn preserves_link_when_partially_selecting_link_text() {
    let editor = editor();
    update(&editor, || {
        let link_text = text("link text");
        get_root().append(&[paragraph(vec![
            text("before "),
            link_node("https://example.com", vec![link_text.clone()]),
            text(" after"),
        ])])?;
        link_text.select(Some(0), Some(4))?;
        Ok(())
    });
    assert_eq!(export_selection(&editor), "[link](https://example.com)");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > list partial selection only includes selected items
#[test]
fn list_partial_selection_only_includes_selected_items() {
    let editor = editor();
    update(&editor, || {
        let t2 = text("Item 2");
        let t3 = text("Item 3");
        get_root().append(&[bullet_list(vec![
            item(vec![text("Item 1")]),
            item(vec![t2.clone()]),
            item(vec![t3.clone()]),
        ])])?;
        select_carets(&t2, 0, &t3, 2)
    });
    assert_eq!(export_selection(&editor), "- Item 2\n- It");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > quote partial selection only includes selected text
#[test]
fn quote_partial_selection_only_includes_selected_text() {
    let editor = editor();
    update(&editor, || {
        let t2 = text("Line 2");
        get_root().append(&[with(
            create_quote_node(false),
            vec![text("Line 1"), create_line_break_node(), t2.clone()],
        )])?;
        t2.select(Some(0), None)?;
        Ok(())
    });
    assert_eq!(export_selection(&editor), "> Line 2");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString > nested list partial selection only includes selected nested items
#[test]
fn nested_list_partial_selection_only_includes_selected_nested_items() {
    let editor = editor();
    update(&editor, || {
        let nt1 = text("Nested A");
        let nested_list = bullet_list(vec![item(vec![nt1.clone()]), item(vec![text("Nested B")])]);
        get_root().append(&[bullet_list(vec![
            item(vec![text("Item 1")]),
            item(vec![nested_list]),
            item(vec![text("Item 3")]),
        ])])?;
        nt1.select(Some(0), None)?;
        Ok(())
    });
    assert_eq!(export_selection(&editor), "    - Nested A");
}

/// `$convertFromMarkdownString(markdown, transformers)` into a fresh
/// editor, rendered with `$generateHtmlFromNodes`.
#[cfg(feature = "html")]
fn import_html(markdown: &str, transformers: &[Transformer]) -> String {
    let editor = editor();
    import_full(&editor, markdown, transformers, false, false);
    html(&editor)
}

/// `[CHECK_LIST, ...TRANSFORMERS]`.
fn with_check_list() -> Vec<Transformer> {
    let mut all = vec![Transformer::Element(check_list())];
    all.extend(transformers());
    all
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::List marker details > reads an uppercase [X] as checked, as the /i on its regex intends
#[cfg(feature = "html")]
#[test]
fn reads_an_uppercase_x_as_checked() {
    assert_eq!(
        import_html("- [X] up\n- [ ] no", &with_check_list()),
        "<ul __lexicallisttype=\"check\"><li role=\"checkbox\" tabindex=\"-1\" aria-checked=\"true\" value=\"1\"><span style=\"white-space: pre-wrap;\">up</span></li><li role=\"checkbox\" tabindex=\"-1\" aria-checked=\"false\" value=\"2\"><span style=\"white-space: pre-wrap;\">no</span></li></ul>"
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::List marker details > expands a tab after the marker when measuring the content column
#[cfg(feature = "html")]
#[test]
fn expands_a_tab_after_the_marker_when_measuring_the_content_column() {
    // The tab opens the content of `a` at column four, so a two-space `- b`
    // does not reach it and stays a sibling.
    assert_eq!(
        import_html("-\ta\n  - b", &transformers()),
        "<ul><li value=\"1\"><span style=\"white-space: pre-wrap;\">a</span></li><li value=\"2\"><span style=\"white-space: pre-wrap;\">b</span></li></ul>"
    );
}

/// A root of a paragraph, selected at its end, before `list`, with the
/// shortcuts registered; then `typed` is typed a character at a time and an
/// empty discrete update runs.
fn type_above_list(editor: &Editor, list: fn() -> NodeRef, typed: &str) {
    update(editor, move || {
        let p = create_paragraph_node();
        set_root(vec![p.clone(), list()])?;
        p.select_end()?;
        Ok(())
    });
    type_chars(editor, typed);
    update(editor, || Ok(()));
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::List marker details > keeps a typed start number out of a list the item only passes through
#[test]
fn keeps_a_typed_start_number_out_of_a_list_the_item_only_passes_through() {
    let editor = editor();
    let _unregister = register_markdown_shortcuts(&editor, &transformers()).unwrap();
    // An indented ordered marker typed above an ordered list sends the item
    // into a sublist, so the outer list's start stays what it was.
    type_above_list(
        &editor,
        || {
            with(
                create_list_node(ListType::Number, 5.0),
                vec![item(vec![text("y")])],
            )
        },
        "    1. z",
    );
    read(&editor, || {
        let list = get_root().get_first_child().unwrap();
        assert!(is_list_node(&list), "expected a list");
        assert_eq!(list.get_list_start(), Some(5.0));
        assert_eq!(list.get_last_child().unwrap().get_text_content(), "y");
    });
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Loose sublists > nests a loose type-changing sublist into the item above it
#[cfg(feature = "html")]
#[test]
fn nests_a_loose_type_changing_sublist_into_the_item_above_it() {
    assert_eq!(
        import_html("- a\n\n  1. b", &transformers()),
        "<ul><li value=\"1\"><span style=\"white-space: pre-wrap;\">a</span><ol><li value=\"1\"><span style=\"white-space: pre-wrap;\">b</span></li></ol></li></ul>"
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Loose sublists > returns one joined list from $generateNodesFromMarkdownString
#[test]
fn returns_one_joined_list_from_generate_nodes_from_markdown_string() {
    let editor = editor();
    let shape = update_with(&editor, || {
        Ok(
            generate_nodes_from_markdown_string("- a\n\n  - b", &transformers(), false, false)?
                .iter()
                .map(|n| {
                    if is_list_node(n) {
                        "list".to_owned()
                    } else {
                        n.get_type().to_owned()
                    }
                })
                .collect::<Vec<_>>()
                .join(","),
        )
    });
    assert_eq!(shape, "list");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Sublist indent boundaries > does not nest an indent short of a wide marker's content column
#[cfg(feature = "html")]
#[test]
fn does_not_nest_an_indent_short_of_a_wide_markers_content_column() {
    assert_eq!(
        import_html("10. a\n   - b", &transformers()),
        "<ol start=\"10\"><li value=\"10\"><span style=\"white-space: pre-wrap;\">a</span></li></ol><ul><li value=\"1\"><span style=\"white-space: pre-wrap;\">b</span></li></ul>"
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Sublist indent boundaries > keeps a retyped sublist in front of the one it was placed before
#[cfg(feature = "html")]
#[test]
fn keeps_a_retyped_sublist_in_front_of_the_one_it_was_placed_before() {
    let editor = editor();
    let _unregister = register_markdown_shortcuts(&editor, &transformers()).unwrap();
    type_above_list(
        &editor,
        || {
            with(
                create_list_node(ListType::Number, 1.0),
                vec![
                    item(vec![bullet_list(vec![item(vec![text("x")])])]),
                    item(vec![text("y")]),
                ],
            )
        },
        "    1. z",
    );
    assert_eq!(
        html(&editor),
        "<ol><li value=\"1\"><ol><li value=\"1\"><span style=\"white-space: pre-wrap;\">z</span></li></ol><ul><li value=\"1\"><span style=\"white-space: pre-wrap;\">x</span></li></ul></li><li value=\"1\"><span style=\"white-space: pre-wrap;\">y</span></li></ol>"
    );
}

// upstream: "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Loose list indents round trip > re-imports its own export of \"- a\n\n  - b\" unchanged"
// upstream: "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Loose list indents round trip > re-imports its own export of \"- a\n\n        - b\" unchanged"
// upstream: "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Loose list indents round trip > re-imports its own export of \"1. First\n\n   - detail\" unchanged"
// upstream: "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Loose list indents round trip > re-imports its own export of \"- a\n\ntext\n\n  - b\" unchanged"
#[test]
fn re_imports_its_own_export_unchanged() {
    let cases = [
        "- a\n\n  - b",
        "- a\n\n        - b",
        "1. First\n\n   - detail",
        "- a\n\ntext\n\n  - b",
    ]
    .into_iter()
    .map(|md| {
        case(
            format!(
                "Loose list indents round trip > re-imports its own export of \"{md}\" unchanged"
            ),
            move || {
                let once = roundtrip(md);
                assert_eq!(roundtrip(&once), once);
            },
        )
    })
    .collect();
    run_cases(SOURCE, FILE, "re_imports_its_own_export_unchanged", cases);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Sublist markers > records a marker on the list the item lands in, not the one above it
#[test]
fn records_a_marker_on_the_list_the_item_lands_in() {
    let md = "- a\n    * b\n    * c";
    assert_eq!(roundtrip(md), md);
}

/// `typeLines(editor, lines)`: each line typed a character at a time into a
/// new paragraph.
fn type_lines(editor: &Editor, lines: &[&str]) {
    for line in lines {
        update(editor, || {
            let p = create_paragraph_node();
            get_root().append(std::slice::from_ref(&p))?;
            p.select_end()?;
            Ok(())
        });
        type_chars(editor, line);
    }
}

/// `caretText(editor)`: the anchor node's text and offset.
fn caret_text(editor: &Editor) -> String {
    read(editor, || {
        let selection = get_selection()
            .and_then(|s| s.range())
            .expect("expected a range selection");
        format!(
            "{}@{}",
            selection.anchor.get_node().unwrap().get_text_content(),
            selection.anchor.offset
        )
    })
}

fn shortcut_editor() -> Editor {
    let editor = editor();
    let _unregister = register_markdown_shortcuts(&editor, &transformers()).unwrap();
    editor
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > indents a typed "1. " with no list above it
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > indents a typed "    1. " with no list above it
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > indents a typed "- " with no list above it
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > indents a typed "    - " with no list above it
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > indents a typed "      * " with no list above it
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > indents a typed "        * " with no list above it
// upstream: "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > indents a typed \"\t- \" with no list above it"
#[cfg(feature = "html")]
#[test]
fn indents_a_typed_marker_with_no_list_above_it() {
    let table = [
        ("1. ", "<ol><li value=\"1\"></li></ol>"),
        (
            "    1. ",
            "<ol><li value=\"1\"><ol><li value=\"1\"></li></ol></li></ol>",
        ),
        ("- ", "<ul><li value=\"1\"></li></ul>"),
        (
            "    - ",
            "<ul><li value=\"1\"><ul><li value=\"1\"></li></ul></li></ul>",
        ),
        (
            "      * ",
            "<ul><li value=\"1\"><ul><li value=\"1\"></li></ul></li></ul>",
        ),
        (
            "        * ",
            "<ul><li value=\"1\"><ul><li value=\"1\"><ul><li value=\"1\"></li></ul></li></ul></li></ul>",
        ),
        (
            "\t- ",
            "<ul><li value=\"1\"><ul><li value=\"1\"></li></ul></li></ul>",
        ),
    ];
    let cases = table
        .into_iter()
        .map(|(typed, expected)| {
            case(
                format!(
                    "Typed sublist shortcuts > indents a typed \"{typed}\" with no list above it"
                ),
                move || {
                    let editor = shortcut_editor();
                    type_lines(&editor, &[typed]);
                    // The shortcut runs from an update listener, so it lands
                    // on the next update.
                    update(&editor, || Ok(()));
                    assert_eq!(html(&editor), expected);
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "indents_a_typed_marker_with_no_list_above_it",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > nests a typed sublist of another type and keeps the caret in it
#[cfg(feature = "html")]
#[test]
fn nests_a_typed_sublist_of_another_type_and_keeps_the_caret_in_it() {
    let editor = shortcut_editor();
    type_lines(&editor, &["- a", "    1. b"]);
    assert_eq!(
        html(&editor),
        "<ul><li value=\"1\"><span style=\"white-space: pre-wrap;\">a</span><ol><li value=\"1\"><span style=\"white-space: pre-wrap;\">b</span></li></ol></li></ul>"
    );
    assert_eq!(caret_text(&editor), "b@1");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > nests a typed sublist of the same type and keeps the caret in it
#[cfg(feature = "html")]
#[test]
fn nests_a_typed_sublist_of_the_same_type_and_keeps_the_caret_in_it() {
    let editor = shortcut_editor();
    type_lines(&editor, &["- a", "    - b"]);
    assert_eq!(
        html(&editor),
        "<ul><li value=\"1\"><span style=\"white-space: pre-wrap;\">a</span><ul><li value=\"1\"><span style=\"white-space: pre-wrap;\">b</span></li></ul></li></ul>"
    );
    assert_eq!(caret_text(&editor), "b@1");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > reads a typed indent in fixed steps, not by content column
#[cfg(feature = "html")]
#[test]
fn reads_a_typed_indent_in_fixed_steps_not_by_content_column() {
    let editor = shortcut_editor();
    type_lines(&editor, &["1. a", "   - b"]);
    assert_eq!(
        html(&editor),
        "<ol><li value=\"1\"><span style=\"white-space: pre-wrap;\">a</span></li></ol><ul><li value=\"1\"><span style=\"white-space: pre-wrap;\">b</span></li></ul>"
    );
    assert_eq!(caret_text(&editor), "b@1");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Typed sublist shortcuts > nests a typed sublist under a list a shortcut did not create
#[cfg(feature = "html")]
#[test]
fn nests_a_typed_sublist_under_a_list_a_shortcut_did_not_create() {
    let editor = shortcut_editor();
    update(&editor, || {
        set_root(vec![bullet_list(vec![item(vec![text("a")])])])
    });
    type_lines(&editor, &["    1. b"]);
    assert_eq!(
        html(&editor),
        "<ul><li value=\"1\"><span style=\"white-space: pre-wrap;\">a</span><ol><li value=\"1\"><span style=\"white-space: pre-wrap;\">b</span></li></ol></li></ul>"
    );
    assert_eq!(caret_text(&editor), "b@1");
}

/// The root holds `blocks()`, whose paragraph (found by `paragraph_at`)
/// gets `typed` inserted at its end; then a space is typed.
#[cfg(feature = "html")]
fn type_marker_beside_list(blocks: fn() -> (NodeRef, Vec<NodeRef>), typed: &'static str) -> String {
    let editor = shortcut_editor();
    update(&editor, move || {
        let (p, all) = blocks();
        set_root(all)?;
        p.select_end()?.insert_text(typed)
    });
    type_chars(&editor, " ");
    html(&editor)
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Ordered list start adjustment (#8677) > updates list start when typed marker precedes an existing ordered list
#[cfg(feature = "html")]
#[test]
fn updates_list_start_when_typed_marker_precedes_an_existing_ordered_list() {
    let out = type_marker_beside_list(
        || {
            let p = create_paragraph_node();
            let list = with(
                create_list_node(ListType::Number, 2.0),
                vec![item(vec![text("A")]), item(vec![text("B")])],
            );
            (p.clone(), vec![p, list])
        },
        "1.",
    );
    assert_eq!(
        out,
        "<ol><li value=\"1\"></li><li value=\"2\"><span style=\"white-space: pre-wrap;\">A</span></li><li value=\"3\"><span style=\"white-space: pre-wrap;\">B</span></li></ol>"
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Ordered list start adjustment (#8677) > respects an arbitrary typed start number
#[cfg(feature = "html")]
#[test]
fn respects_an_arbitrary_typed_start_number() {
    let out = type_marker_beside_list(
        || {
            let p = create_paragraph_node();
            let list = with(
                create_list_node(ListType::Number, 2.0),
                vec![item(vec![text("A")])],
            );
            (p.clone(), vec![p, list])
        },
        "7.",
    );
    assert_eq!(
        out,
        "<ol start=\"7\"><li value=\"7\"></li><li value=\"8\"><span style=\"white-space: pre-wrap;\">A</span></li></ol>"
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Ordered list start adjustment (#8677) > does not change start when typed marker follows an existing ordered list
#[cfg(feature = "html")]
#[test]
fn does_not_change_start_when_typed_marker_follows_an_existing_ordered_list() {
    let out = type_marker_beside_list(
        || {
            let list = with(
                create_list_node(ListType::Number, 2.0),
                vec![item(vec![text("A")])],
            );
            let p = create_paragraph_node();
            (p.clone(), vec![list, p])
        },
        "9.",
    );
    assert_eq!(
        out,
        "<ol start=\"2\"><li value=\"2\"><span style=\"white-space: pre-wrap;\">A</span></li><li value=\"3\"></li></ol>"
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::Ordered list start adjustment (#8677) > creates a fresh ordered list when the next sibling is a different list type
#[cfg(feature = "html")]
#[test]
fn creates_a_fresh_ordered_list_when_the_next_sibling_is_a_different_list_type() {
    let out = type_marker_beside_list(
        || {
            let p = create_paragraph_node();
            let bullets = bullet_list(vec![item(vec![text("A")])]);
            (p.clone(), vec![p, bullets])
        },
        "5.",
    );
    assert_eq!(
        out,
        "<ol start=\"5\"><li value=\"5\"></li></ol><ul><li value=\"1\"><span style=\"white-space: pre-wrap;\">A</span></li></ul>"
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$generateNodesFromMarkdownString > returns nodes without modifying the root
#[test]
fn returns_nodes_without_modifying_the_root() {
    let editor = editor();
    update(&editor, || {
        set_root(vec![paragraph(vec![text("existing")])])
    });
    let types = update_with(&editor, || {
        Ok(generate_nodes_from_markdown_string(
            "# Heading\n\nParagraph",
            &transformers(),
            false,
            false,
        )?
        .iter()
        .map(|n| n.get_type().to_owned())
        .collect::<Vec<_>>())
    });
    assert_eq!(types, ["heading", "paragraph"]);
    assert_eq!(root_text(&editor), "existing");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$generateNodesFromMarkdownString > produces the same nodes as $convertFromMarkdownString
#[cfg(feature = "html")]
#[test]
fn produces_the_same_nodes_as_convert_from_markdown_string() {
    let md = "# Title\n\n- item 1\n- item 2\n\n> quote\n\n```\ncode\n```";
    let convert_html = import_html(md, &transformers());
    let generate_editor = editor();
    update(&generate_editor, move || {
        let nodes = generate_nodes_from_markdown_string(md, &transformers(), false, false)?;
        set_root(nodes)
    });
    assert_eq!(html(&generate_editor), convert_html);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$generateNodesFromMarkdownString > returned nodes can be inserted at selection
#[cfg(feature = "html")]
#[test]
fn returned_nodes_can_be_inserted_at_selection() {
    let editor = editor();
    update(&editor, || {
        set_root(vec![
            paragraph(vec![text("before")]),
            paragraph(vec![text("after")]),
        ])
    });
    update(&editor, || {
        get_root().select(Some(1), Some(1))?;
        let nodes = generate_nodes_from_markdown_string("**bold**", &transformers(), false, false)?;
        if let Some(selection) = get_selection().filter(|s| s.is_range()) {
            selection.insert_nodes(nodes)?;
        }
        Ok(())
    });
    let out = html(&editor);
    assert!(out.contains("before"));
    assert!(out.contains("<strong"));
    assert!(out.contains("after"));
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$generateNodesFromMarkdownString > handles adjacent line merging (commonmark)
#[test]
fn handles_adjacent_line_merging() {
    let editor = editor();
    let types = update_with(&editor, || {
        Ok(
            generate_nodes_from_markdown_string("line 1\nline 2", &transformers(), false, true)?
                .iter()
                .map(|n| n.get_type().to_owned())
                .collect::<Vec<_>>(),
        )
    });
    assert_eq!(types, ["paragraph"]);
}

/// `createTestEditor({nodes: [LinkNode]})`.
fn link_editor() -> Editor {
    create_test_editor(Some(vec![NodeClass::of::<crate::link::LinkNode>()]))
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString whitespace slices > does not emit a dangling closing tag when the selection slices a format down to whitespace
#[test]
fn does_not_emit_a_dangling_closing_tag_when_the_selection_slices_a_format_down_to_whitespace() {
    let editor = link_editor();
    update(&editor, || {
        let first = text("a  ");
        first.toggle_format("bold");
        first.set_style("color: red");
        let second = text("b");
        second.toggle_format("bold");
        get_root().append(&[paragraph(vec![first.clone(), second.clone()])])?;
        select_carets(&first, 1, &second, 1)
    });
    assert_eq!(export_selection(&editor), "  **b**");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString whitespace slices > closes a tag before a formatted space at the end of a selected link
#[test]
fn closes_a_tag_before_a_formatted_space_at_the_end_of_a_selected_link() {
    let editor = link_editor();
    update(&editor, || {
        let bold = format("bold");
        let p = paragraph(vec![
            link_node(
                "u",
                vec![
                    formatted("a", or(bold, format("italic"))),
                    formatted(" ", bold),
                ],
            ),
            text("c"),
        ]);
        get_root().append(std::slice::from_ref(&p))?;
        let size = p.get_children_size();
        p.select(Some(0), Some(size))?;
        Ok(())
    });
    assert_eq!(export_selection(&editor), "[***a*** ](u)c");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString whitespace slices > closes the tags of the previous node before a slice cut down to whitespace
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString whitespace slices > keeps a tag open across consecutive formatted spaces
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString whitespace slices > closes before a slice cut down to a formatted space
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString whitespace slices > closes when the format continues only after the selection
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString whitespace slices > closes every tag at the end of a sliced node
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString whitespace slices > leaves out inline code the selection only touches
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::$convertSelectionToMarkdownString whitespace slices > leaves out inline code the selection starts at the end of
#[test]
fn whitespace_slices() {
    let b = format("bold");
    let i = format("italic");
    let s = format("strikethrough");
    let c = format("code");
    type Formats = Vec<(&'static str, f64)>;
    let table: Vec<(&str, Formats, [usize; 4], &str)> = vec![
        (
            "closes the tags of the previous node before a slice cut down to whitespace",
            vec![("a", or(b, i)), (" b", b)],
            [0, 0, 1, 1],
            "***a*** ",
        ),
        (
            "keeps a tag open across consecutive formatted spaces",
            vec![("a", b), (" ", or(b, i)), (" ", or(b, s)), ("cd", b)],
            [0, 0, 3, 1],
            "**a  c**",
        ),
        (
            "closes before a slice cut down to a formatted space",
            vec![("a ", b), (" b", or(b, i))],
            [0, 0, 1, 1],
            "**a**  ",
        ),
        (
            "closes when the format continues only after the selection",
            vec![("a", or(b, i)), (" x", b), ("c", or(b, i))],
            [0, 0, 1, 1],
            "***a*** ",
        ),
        (
            "closes every tag at the end of a sliced node",
            vec![("a", b), ("bc", or(b, i)), ("d", b)],
            [0, 0, 1, 1],
            "**a*b***",
        ),
        (
            "leaves out inline code the selection only touches",
            vec![("a", b), ("xy", or(b, c))],
            [0, 0, 1, 0],
            "**a**",
        ),
        (
            "leaves out inline code the selection starts at the end of",
            vec![("xy", or(b, c)), ("a", b)],
            [0, 2, 1, 1],
            "**a**",
        ),
    ];
    let cases = table
        .into_iter()
        .map(
            |(name, formats, [start_node, start, end_node, end], expected)| {
                case(
                    format!("$convertSelectionToMarkdownString whitespace slices > {name}"),
                    move || {
                        let editor = link_editor();
                        update(&editor, move || {
                            let nodes: Vec<NodeRef> =
                                formats.iter().map(|(t, f)| formatted(t, *f)).collect();
                            get_root().append(&[paragraph(nodes.clone())])?;
                            select_carets(&nodes[start_node], start, &nodes[end_node], end)
                        });
                        assert_eq!(export_selection(&editor), expected);
                    },
                )
            },
        )
        .collect();
    run_cases(SOURCE, FILE, "whitespace_slices", cases);
}

/// `ImportedLink`: a link's title and URL.
type ImportedLink = (Option<String>, String);

/// `importLinks(md)`: the title and URL of each text node's link parent.
fn import_links(md: &str) -> Vec<ImportedLink> {
    let editor = link_editor();
    import(&editor, md);
    read(&editor, || {
        get_root()
            .get_all_text_nodes()
            .iter()
            .filter_map(NodeRef::get_parent)
            .filter(is_link_node)
            .map(|l| (l.get_title(), l.get_url().unwrap_or_default()))
            .collect()
    })
}

/// `{title, url}`.
fn il(title: Option<&str>, url: &str) -> ImportedLink {
    (title.map(str::to_owned), url.to_owned())
}

/// Each `[md, links]` case as an upstream `reads ${JSON.stringify(md)}`
/// test of `describe`, checking `importLinks(md)`.
fn import_links_cases(describe: &str, table: Vec<(&'static str, Vec<ImportedLink>)>) -> Cases {
    table
        .into_iter()
        .map(|(md, links)| {
            case(format!("{describe} > reads {}", j(md)), move || {
                assert_eq!(import_links(md), links);
            })
        })
        .collect()
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination ends at whitespace > reads "[x](b\\ \"t\")"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination ends at whitespace > reads "[x](foo\\ bar)"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination ends at whitespace > reads "[x](foo\\ bar \"t\")"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination ends at whitespace > reads "[x](a\\ b)"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination ends at whitespace > reads "[x](a\\ b \"t\")"
#[test]
fn link_destination_ends_at_whitespace() {
    let cases = import_links_cases(
        "link destination ends at whitespace",
        vec![
            (r#"[x](b\ "t")"#, vec![il(Some("t"), r"b\")]),
            (r"[x](foo\ bar)", vec![]),
            (r#"[x](foo\ bar "t")"#, vec![]),
            (r"[x](a\ b)", vec![]),
            (r#"[x](a\ b "t")"#, vec![]),
        ],
    );
    run_cases(SOURCE, FILE, "link_destination_ends_at_whitespace", cases);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::a raw link destination cannot begin with an angle bracket > reads "[x](<a<b>)"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::a raw link destination cannot begin with an angle bracket > reads "[x](<>x)"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::a raw link destination cannot begin with an angle bracket > reads "[x](<a>b)"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::a raw link destination cannot begin with an angle bracket > reads "[x](<foo)"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::a raw link destination cannot begin with an angle bracket > reads "[x](<\\>)"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::a raw link destination cannot begin with an angle bracket > reads "[x](a<b>c)"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::a raw link destination cannot begin with an angle bracket > reads "[x](a<b)"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::a raw link destination cannot begin with an angle bracket > reads "[x](a>b)"
#[test]
fn a_raw_link_destination_cannot_begin_with_an_angle_bracket() {
    let cases = import_links_cases(
        "a raw link destination cannot begin with an angle bracket",
        vec![
            ("[x](<a<b>)", vec![]),
            ("[x](<>x)", vec![]),
            ("[x](<a>b)", vec![]),
            ("[x](<foo)", vec![]),
            (r"[x](<\>)", vec![]),
            // Anywhere but in first place an angle bracket is ordinary.
            ("[x](a<b>c)", vec![il(None, "a<b>c")]),
            ("[x](a<b)", vec![il(None, "a<b")]),
            ("[x](a>b)", vec![il(None, "a>b")]),
        ],
    );
    run_cases(
        SOURCE,
        FILE,
        "a_raw_link_destination_cannot_begin_with_an_angle_bracket",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads "[x](/uri \"t\")"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads "[x](/uri 't')"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads "[x](/uri (t))"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads "[x](/uri  \"t\")"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads "[x](/uri\t\"t\")"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads "[x](/uri\"t\")"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads "[x]( \"t\")"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads "[x](/uri (a(b)c))"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads "[x]( /uri)"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads "[x](/uri )"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads "[x]( /uri \"t\" )"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads "[x]()"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads "[x]( )"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads "[x](foo(and(bar)))"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads "[x](a(b(c)d)e)"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination and title shapes > reads "[x](((a)))"
#[test]
fn link_destination_and_title_shapes() {
    let cases = import_links_cases(
        "link destination and title shapes",
        vec![
            // A title takes any of the three spellings, after whitespace.
            (r#"[x](/uri "t")"#, vec![il(Some("t"), "/uri")]),
            ("[x](/uri 't')", vec![il(Some("t"), "/uri")]),
            ("[x](/uri (t))", vec![il(Some("t"), "/uri")]),
            (r#"[x](/uri  "t")"#, vec![il(Some("t"), "/uri")]),
            ("[x](/uri\t\"t\")", vec![il(Some("t"), "/uri")]),
            // Without the whitespace it is all destination.
            (r#"[x](/uri"t")"#, vec![il(None, r#"/uri"t""#)]),
            (r#"[x]( "t")"#, vec![il(None, r#""t""#)]),
            // A parenthesized title may not hold an unescaped parenthesis.
            ("[x](/uri (a(b)c))", vec![]),
            // Whitespace may sit on either side of the destination.
            ("[x]( /uri)", vec![il(None, "/uri")]),
            ("[x](/uri )", vec![il(None, "/uri")]),
            (r#"[x]( /uri "t" )"#, vec![il(Some("t"), "/uri")]),
            // The destination itself is optional.
            ("[x]()", vec![il(None, "")]),
            ("[x]( )", vec![il(None, "")]),
            // Parentheses nest, up to the depth the pattern is written for.
            ("[x](foo(and(bar)))", vec![il(None, "foo(and(bar))")]),
            ("[x](a(b(c)d)e)", vec![il(None, "a(b(c)d)e")]),
            ("[x](((a)))", vec![il(None, "((a))")]),
        ],
    );
    run_cases(SOURCE, FILE, "link_destination_and_title_shapes", cases);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination whitespace does not backtrack > reads a run that never closes
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination whitespace does not backtrack > reads a run in front of a quote
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination whitespace does not backtrack > reads a run of tabs
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination whitespace does not backtrack > reads a run on either side of a destination
#[test]
fn link_destination_whitespace_does_not_backtrack() {
    const RUN: usize = 200_000;
    let table = vec![
        (
            "a run that never closes",
            format!("[x]({}", " ".repeat(RUN)),
        ),
        (
            "a run in front of a quote",
            format!("[x]({}\"", " ".repeat(RUN)),
        ),
        ("a run of tabs", format!("[x]({}", "\t".repeat(RUN))),
        (
            "a run on either side of a destination",
            format!("[x]({}/uri{}", " ".repeat(RUN), " ".repeat(RUN)),
        ),
    ];
    let cases = table
        .into_iter()
        .map(|(name, md)| {
            case(
                format!("link destination whitespace does not backtrack > reads {name}"),
                move || {
                    // Upstream's limit is `{timeout: 5000}`. A debug build
                    // takes about 2 s for the run of tabs alone (it makes
                    // 200,000 TabNodes, as JS does), and more on a busy CI
                    // runner, while walking every split of a run would take
                    // minutes. So the limit is 30 s, as in the Swift port.
                    let start = std::time::Instant::now();
                    let editor = link_editor();
                    import(&editor, &md);
                    // None of them is a link.
                    assert_eq!(
                        read(&editor, || {
                            get_root()
                                .get_all_text_nodes()
                                .iter()
                                .filter_map(NodeRef::get_parent)
                                .filter(is_link_node)
                                .count()
                        }),
                        0
                    );
                    assert!(start.elapsed() < std::time::Duration::from_secs(30));
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "link_destination_whitespace_does_not_backtrack",
        cases,
    );
}

/// `expectLinkRoundTrip(url, title, md, after, afterFormat)`.
fn expect_link_round_trip(
    url: &str,
    title: Option<&str>,
    md: &str,
    after: &str,
    after_format: Option<&'static str>,
) {
    let editor = link_editor();
    let (u, t, a) = (url.to_owned(), title.map(str::to_owned), after.to_owned());
    update(&editor, move || {
        let l = create_link_node(
            &u,
            &LinkAttributes {
                title: Some(t),
                ..LinkAttributes::default()
            },
        );
        l.append(&[text("x")])?;
        let p = paragraph(vec![l]);
        if !a.is_empty() {
            let after_node = text(&a);
            if let Some(f) = after_format {
                after_node.toggle_format(f);
            }
            p.append(&[after_node])?;
        }
        set_root(vec![p])
    });
    let exported = export(&editor);
    assert_eq!(exported, md);
    let reimported = link_editor();
    import(&reimported, &exported);
    let (links, all) = read(&reimported, || {
        (
            get_root()
                .get_all_text_nodes()
                .iter()
                .filter_map(NodeRef::get_parent)
                .filter(is_link_node)
                .map(|l| (l.get_url().unwrap_or_default(), l.get_title()))
                .collect::<Vec<_>>(),
            get_root().get_text_content(),
        )
    });
    assert_eq!(links, [(url.to_owned(), title.map(str::to_owned))]);
    assert_eq!(all, format!("x{after}"));
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://en.wikipedia.org/wiki/Ruby_(programming_language)"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/a)b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/a(b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/a(b)c(d)e"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/((a))"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/a\\"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/a\\(b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/a b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/a b(c)"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/a b>c"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/a b<c"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/a b\\"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/a\tb"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "/a\u0001b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "/ab"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "/a\u0000b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/a\nb"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/a\r\nb"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/a\rb"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/a b\nc"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/a<b>c"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/a&#10;b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/a&#x0A;b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/a&amp;b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "&#38;"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/?a=1&b=2"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://example.com/?a=1&b=2;c=3"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "<foo>"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves ""
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link destination round trip > preserves "https://lexical.dev"
#[test]
fn link_destination_round_trip() {
    let table = [
        (
            "https://en.wikipedia.org/wiki/Ruby_(programming_language)",
            r"[x](https://en.wikipedia.org/wiki/Ruby_\(programming_language\))",
        ),
        ("https://example.com/a)b", r"[x](https://example.com/a\)b)"),
        ("https://example.com/a(b", r"[x](https://example.com/a\(b)"),
        (
            "https://example.com/a(b)c(d)e",
            r"[x](https://example.com/a\(b\)c\(d\)e)",
        ),
        (
            "https://example.com/((a))",
            r"[x](https://example.com/\(\(a\)\))",
        ),
        (r"https://example.com/a\", r"[x](https://example.com/a\\)"),
        (
            r"https://example.com/a\(b",
            r"[x](https://example.com/a\\\(b)",
        ),
        ("https://example.com/a b", "[x](<https://example.com/a b>)"),
        (
            "https://example.com/a b(c)",
            "[x](<https://example.com/a b(c)>)",
        ),
        (
            "https://example.com/a b>c",
            r"[x](<https://example.com/a b\>c>)",
        ),
        (
            "https://example.com/a b<c",
            r"[x](<https://example.com/a b\<c>)",
        ),
        (
            r"https://example.com/a b\",
            r"[x](<https://example.com/a b\\>)",
        ),
        (
            "https://example.com/a\tb",
            "[x](<https://example.com/a\tb>)",
        ),
        // Nor a control character, which the pointy form can hold.
        ("/a\u{0001}b", "[x](</a\u{0001}b>)"),
        ("/a\u{007f}b", "[x](</a\u{007f}b>)"),
        ("/a\u{0000}b", "[x](</a\u{0000}b>)"),
        // No destination may hold a line ending.
        (
            "https://example.com/a\nb",
            "[x](<https://example.com/a&#10;b>)",
        ),
        (
            "https://example.com/a\r\nb",
            "[x](<https://example.com/a&#13;&#10;b>)",
        ),
        (
            "https://example.com/a\rb",
            "[x](<https://example.com/a&#13;b>)",
        ),
        (
            "https://example.com/a b\nc",
            "[x](<https://example.com/a b&#10;c>)",
        ),
        // Only a `<` in first place would turn the raw form into the pointy
        // one.
        (
            "https://example.com/a<b>c",
            "[x](https://example.com/a<b>c)",
        ),
        // A URL that already holds a character reference keeps it.
        (
            "https://example.com/a&#10;b",
            "[x](https://example.com/a&#38;#10;b)",
        ),
        (
            "https://example.com/a&#x0A;b",
            "[x](https://example.com/a&#38;#x0A;b)",
        ),
        (
            "https://example.com/a&amp;b",
            "[x](https://example.com/a&#38;amp;b)",
        ),
        ("&#38;", "[x](&#38;#38;)"),
        // An `&` that begins no reference is ordinary.
        (
            "https://example.com/?a=1&b=2",
            "[x](https://example.com/?a=1&b=2)",
        ),
        (
            "https://example.com/?a=1&b=2;c=3",
            "[x](https://example.com/?a=1&b=2;c=3)",
        ),
        ("<foo>", r"[x](\<foo>)"),
        ("", "[x](<>)"),
        ("https://lexical.dev", "[x](https://lexical.dev)"),
    ];
    let cases = table
        .into_iter()
        .map(|(url, md)| {
            case(
                format!("link destination round trip > preserves {}", j(url)),
                move || expect_link_round_trip(url, None, md, "", None),
            )
        })
        .collect();
    run_cases(SOURCE, FILE, "link_destination_round_trip", cases);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves "a\nb"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves "a\rb"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves "a\r\nb"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves "a\n\nb"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves "a&#10;\nb"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves "a&#10;b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves "a&amp;b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves "a&b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves "a\"b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves "a\\b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves "a\\\"b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link title round trip > preserves "plain"
#[test]
fn link_title_round_trip() {
    let table = [
        ("a\nb", r#"[x](/uri "a&#10;b")"#),
        ("a\rb", r#"[x](/uri "a&#13;b")"#),
        ("a\r\nb", r#"[x](/uri "a&#13;&#10;b")"#),
        ("a\n\nb", r#"[x](/uri "a&#10;&#10;b")"#),
        ("a&#10;\nb", r#"[x](/uri "a&#38;#10;&#10;b")"#),
        ("a&#10;b", r#"[x](/uri "a&#38;#10;b")"#),
        ("a&amp;b", r#"[x](/uri "a&#38;amp;b")"#),
        ("a&b", r#"[x](/uri "a&b")"#),
        ("a\"b", r#"[x](/uri "a\"b")"#),
        (r"a\b", r#"[x](/uri "a\\b")"#),
        (r#"a\"b"#, r#"[x](/uri "a\\\"b")"#),
        ("plain", r#"[x](/uri "plain")"#),
    ];
    let cases = table
        .into_iter()
        .map(|(title, md)| {
            case(
                format!("link title round trip > preserves {}", j(title)),
                move || expect_link_round_trip("/uri", Some(title), md, "", None),
            )
        })
        .collect();
    run_cases(SOURCE, FILE, "link_title_round_trip", cases);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::text after a link > keeps ["/uri","a\\"] apart from it
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::text after a link > keeps ["/uri","a`"] apart from it
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::text after a link > keeps ["/a`b",null] apart from it
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::text after a link > keeps ["/a `b",null] apart from it
#[test]
fn text_after_a_link_keeps_apart() {
    const AFTER: &str = " \") `b";
    let table = [
        ("/uri", Some(r"a\"), r#"[x](/uri "a\\") ") \`b"#),
        ("/uri", Some("a`"), r#"[x](/uri "a\`") ") \`b"#),
        ("/a`b", None, r#"[x](/a\`b) ") \`b"#),
        ("/a `b", None, r#"[x](</a \`b>) ") \`b"#),
    ];
    let cases = table
        .into_iter()
        .map(|(url, title, md)| {
            case(
                format!(
                    "text after a link > keeps [{},{}] apart from it",
                    j(url),
                    title.map_or("null".to_owned(), j)
                ),
                move || expect_link_round_trip(url, title, md, AFTER, None),
            )
        })
        .collect();
    run_cases(SOURCE, FILE, "text_after_a_link_keeps_apart", cases);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::text after a link > keeps a backtick in the link apart from inline code after it
#[test]
fn keeps_a_backtick_in_the_link_apart_from_inline_code_after_it() {
    expect_link_round_trip("/a`b", None, r"[x](/a\`b)`c`", "c", Some("code"));
}

/// `shortcutGroups(md)` (`LINK.regExp.exec(md)?.slice(1)`) and the same of
/// `LINK.importRegExp`.
type Groups = Option<Vec<Option<String>>>;

fn link_groups(md: &str) -> (Groups, Groups) {
    let builtin = link();
    let groups = |m: RegExpMatch| m.groups[1..].to_vec();
    (
        builtin.reg_exp.exec(md).map(groups),
        builtin
            .import_reg_exp
            .as_ref()
            .unwrap()
            .exec(md)
            .map(groups),
    )
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link import > reads "[x](/uri 'a\\\\')"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link import > reads "[x](/uri 'it\\'s')"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link import > reads "[x](/uri (a\\)))"
// upstream: "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link import > reads \"[x](/uri (a\\\\\u2028b))\""
// upstream: "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link import > reads \"[x](/uri \\\"a\\\\\u2028b\\\")\""
// upstream: "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link import > reads \"[x](</a\\\\\u2028b> \\\"t\\\")\""
#[test]
fn link_import_reads() {
    let table = [
        (r"[x](/uri 'a\\')", "/uri", r"a\"),
        (r"[x](/uri 'it\'s')", "/uri", "it's"),
        (r"[x](/uri (a\)))", "/uri", "a)"),
        ("[x](/uri (a\\\u{2028}b))", "/uri", "a\\\u{2028}b"),
        ("[x](/uri \"a\\\u{2028}b\")", "/uri", "a\\\u{2028}b"),
        ("[x](</a\\\u{2028}b> \"t\")", "/a\\\u{2028}b", "t"),
    ];
    let cases = table
        .into_iter()
        .map(|(md, url, title)| {
            case(format!("link import > reads {}", j(md)), move || {
                assert_eq!(import_links(md), [il(Some(title), url)]);
                let (shortcut, import) = link_groups(md);
                assert_eq!(shortcut, import);
            })
        })
        .collect();
    run_cases(SOURCE, FILE, "link_import_reads", cases);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link import > reads no link from "[x](</a\rb>)"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::link import > reads no link from "[x](</a\\\rb>)"
#[test]
fn link_import_reads_no_link_from() {
    let cases = ["[x](</a\rb>)", "[x](</a\\\rb>)"]
        .into_iter()
        .map(|md| {
            case(
                format!("link import > reads no link from {}", j(md)),
                move || {
                    assert_eq!(import_links(md), []);
                    assert_eq!(link_groups(md).0, None);
                },
            )
        })
        .collect();
    run_cases(SOURCE, FILE, "link_import_reads_no_link_from", cases);
}

const EN_DASH: &str = "\u{2013}";
const LEFT_DOUBLE_QUOTE: &str = "\u{201C}";
const RIGHT_DOUBLE_QUOTE: &str = "\u{201D}";
const LOW_DOUBLE_QUOTE: &str = "\u{201E}";
const EURO_SIGN: &str = "\u{20AC}";
const LATIN_E_ACUTE: &str = "\u{00E9}";
const CYRILLIC_BE: &str = "\u{0431}";
const HERE_LINK: &str = "[here](https://example.com)";
const GRINNING_FACE: &str = "\u{1F600}";
const EMPHASIS: &str = "emphasis next to Unicode punctuation and symbols";

type NamedRun = (String, String);

/// `importRuns(md)`: each text node with its bold and italic formats
/// (`plain` for neither).
fn import_runs(md: &str) -> Vec<NamedRun> {
    let editor = editor();
    import(&editor, md);
    read(&editor, || {
        get_root()
            .get_all_text_nodes()
            .iter()
            .map(|node| {
                let formats: Vec<&str> = ["bold", "italic"]
                    .into_iter()
                    .filter(|f| node.has_format(f))
                    .collect();
                let name = if formats.is_empty() {
                    "plain".to_owned()
                } else {
                    formats.join(" ")
                };
                (node.get_text_content(), name)
            })
            .collect()
    })
}

fn named_runs(table: &[(&str, &str)]) -> Vec<NamedRun> {
    table
        .iter()
        .map(|(t, f)| ((*t).to_owned(), (*f).to_owned()))
        .collect()
}

/// `imports ${JSON.stringify(md)}` cases.
fn import_runs_cases(table: Vec<(String, Vec<NamedRun>)>) -> Cases {
    table
        .into_iter()
        .map(|(md, runs)| {
            case(format!("{EMPHASIS} > imports {}", j(&md)), move || {
                assert_eq!(import_runs(&md), runs);
            })
        })
        .collect()
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports "See –**[here](https://example.com)** now."
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports "See “**[here](https://example.com)**” now."
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports "See „**[here](https://example.com)**“ now."
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports "See €**[here](https://example.com)** now."
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports "See **[here](https://example.com)**–now."
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports "See –*[here](https://example.com)* now."
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports "See –_[here](https://example.com)_ now."
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports "See –__[here](https://example.com)__ now."
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports "See -**[here](https://example.com)** now."
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports "See **[here](https://example.com)** now."
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports "a**b**"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports "**a**b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports "snake_case_word"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports "é**[here](https://example.com)**"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports "б**[here](https://example.com)**"
#[test]
fn emphasis_imports() {
    let h = HERE_LINK;
    let cases = import_runs_cases(vec![
        (
            format!("See {EN_DASH}**{h}** now."),
            named_runs(&[
                (&format!("See {EN_DASH}"), "plain"),
                ("here", "bold"),
                (" now.", "plain"),
            ]),
        ),
        (
            format!("See {LEFT_DOUBLE_QUOTE}**{h}**{RIGHT_DOUBLE_QUOTE} now."),
            named_runs(&[
                (&format!("See {LEFT_DOUBLE_QUOTE}"), "plain"),
                ("here", "bold"),
                (&format!("{RIGHT_DOUBLE_QUOTE} now."), "plain"),
            ]),
        ),
        (
            format!("See {LOW_DOUBLE_QUOTE}**{h}**{LEFT_DOUBLE_QUOTE} now."),
            named_runs(&[
                (&format!("See {LOW_DOUBLE_QUOTE}"), "plain"),
                ("here", "bold"),
                (&format!("{LEFT_DOUBLE_QUOTE} now."), "plain"),
            ]),
        ),
        (
            format!("See {EURO_SIGN}**{h}** now."),
            named_runs(&[
                (&format!("See {EURO_SIGN}"), "plain"),
                ("here", "bold"),
                (" now.", "plain"),
            ]),
        ),
        // Only the closing run touches a Unicode punctuation character.
        (
            format!("See **{h}**{EN_DASH}now."),
            named_runs(&[
                ("See ", "plain"),
                ("here", "bold"),
                (&format!("{EN_DASH}now."), "plain"),
            ]),
        ),
        (
            format!("See {EN_DASH}*{h}* now."),
            named_runs(&[
                (&format!("See {EN_DASH}"), "plain"),
                ("here", "italic"),
                (" now.", "plain"),
            ]),
        ),
        (
            format!("See {EN_DASH}_{h}_ now."),
            named_runs(&[
                (&format!("See {EN_DASH}"), "plain"),
                ("here", "italic"),
                (" now.", "plain"),
            ]),
        ),
        (
            format!("See {EN_DASH}__{h}__ now."),
            named_runs(&[
                (&format!("See {EN_DASH}"), "plain"),
                ("here", "bold"),
                (" now.", "plain"),
            ]),
        ),
        // Unchanged: ASCII punctuation and whitespace still work.
        (
            format!("See -**{h}** now."),
            named_runs(&[("See -", "plain"), ("here", "bold"), (" now.", "plain")]),
        ),
        (
            format!("See **{h}** now."),
            named_runs(&[("See ", "plain"), ("here", "bold"), (" now.", "plain")]),
        ),
        // Unchanged: a delimiter run inside a word.
        (
            "a**b**".to_owned(),
            named_runs(&[("a", "plain"), ("b", "bold")]),
        ),
        (
            "**a**b".to_owned(),
            named_runs(&[("a", "bold"), ("b", "plain")]),
        ),
        (
            "snake_case_word".to_owned(),
            named_runs(&[("snake_case_word", "plain")]),
        ),
        // Unchanged: a letter is not punctuation, whatever the script.
        (
            format!("{LATIN_E_ACUTE}**{h}**"),
            named_runs(&[
                (&format!("{LATIN_E_ACUTE}**"), "plain"),
                ("here", "plain"),
                ("**", "plain"),
            ]),
        ),
        (
            format!("{CYRILLIC_BE}**{h}**"),
            named_runs(&[
                (&format!("{CYRILLIC_BE}**"), "plain"),
                ("here", "plain"),
                ("**", "plain"),
            ]),
        ),
    ]);
    run_cases(SOURCE, FILE, "emphasis_imports", cases);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports "See –**[here](https://example.com)** now." without escaped asterisks
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports "See “**[here](https://example.com)**” now." without escaped asterisks
#[test]
fn emphasis_exports_without_escaped_asterisks() {
    let h = HERE_LINK;
    let table = vec![
        (
            format!("See {EN_DASH}**{h}** now."),
            format!("See {EN_DASH}[**here**](https://example.com) now."),
        ),
        (
            format!("See {LEFT_DOUBLE_QUOTE}**{h}**{RIGHT_DOUBLE_QUOTE} now."),
            format!(
                "See {LEFT_DOUBLE_QUOTE}[**here**](https://example.com){RIGHT_DOUBLE_QUOTE} now."
            ),
        ),
    ];
    let cases = table
        .into_iter()
        .map(|(md, exported)| {
            case(
                format!("{EMPHASIS} > exports {} without escaped asterisks", j(&md)),
                move || assert_eq!(roundtrip(&md), exported),
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "emphasis_exports_without_escaped_asterisks",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports "a**😀**"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports "**😀**a"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports "a **😀**"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports "x﻿**“a”**"
#[test]
fn emphasis_imports_outside_the_bmp() {
    let g = GRINNING_FACE;
    let feff = format!("x\u{FEFF}**{LEFT_DOUBLE_QUOTE}a{RIGHT_DOUBLE_QUOTE}**");
    let cases = import_runs_cases(vec![
        (
            format!("a**{g}**"),
            named_runs(&[(&format!("a**{g}**"), "plain")]),
        ),
        (
            format!("**{g}**a"),
            named_runs(&[(&format!("**{g}**a"), "plain")]),
        ),
        (
            format!("a **{g}**"),
            named_runs(&[("a ", "plain"), (g, "bold")]),
        ),
        // U+FEFF is a format character (Cf), not CommonMark whitespace.
        (feff.clone(), named_runs(&[(&feff, "plain")])),
    ]);
    run_cases(SOURCE, FILE, "emphasis_imports_outside_the_bmp", cases);
}

/// JSON of runs of text and format names, as `%j` prints them.
fn runs_json(runs: &[(String, Vec<&str>)]) -> String {
    format!(
        "[{}]",
        runs.iter()
            .map(|(t, f)| format!(
                "[{},[{}]]",
                j(t),
                f.iter().map(|x| j(x)).collect::<Vec<_>>().join(",")
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [["foo",[]],["“bar”",["bold"]],["baz",[]]] as "fo&#111;**“bar”**&#98;az"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [["a",[]],["😀",["italic"]],["b",[]]] as "&#97;*😀*&#98;"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [["a",[]],["x",["bold","code"]],["b",[]]] as "&#97;**`x`**&#98;"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [["x.",["bold"]],["b",[]],["(y)",["bold"]]] as "**x.**&#98;**(y)**"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [["b",["bold","italic"]],["“y”",["italic"]]] as "***&#98;****“y”*"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [["a",["bold"]],["!",["strikethrough"]]] as "**a**~~!~~"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [["x",[]],["a",["bold"]],["“b”",["italic"]]] as "&#120;**&#97;***“b”*"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [["“a”",["bold","italic"]],["b",["italic"]],["c",[]]] as "***“a”****&#98;*&#99;"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [["x",[]],["a",["bold"]],["“b”",["bold","italic"]]] as "&#120;**&#97;*“b”***"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [["联系我@微博你好",[]],["“重要”",["bold"]]] as "联系我@微博你&#22909;**“重要”**"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [["(a@b.co)and",[]],["“x”",["bold"]]] as "(a@b.co)an&#100;**“x”**"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [["a",[]],["b",["bold"]],["c",[]]] as "a**b**c"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [["a=",[]],["“b”",["highlight"]],["=c",[]]] as "a&#61;==“b”==&#61;c"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [["a.",["bold"]],["x",[]],["c",["bold","code"]]] as "**a.**&#120;**`c`**"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [["“",["bold"]],[".b ",["italic"]],["c",["italic","code"]]] as "**“***.b `c`*"
#[test]
fn emphasis_exports_and_reimports() {
    let (l, r) = (LEFT_DOUBLE_QUOTE, RIGHT_DOUBLE_QUOTE);
    let g = GRINNING_FACE;
    type Runs = Vec<(String, Vec<&'static str>)>;
    let table: Vec<(Runs, String)> = vec![
        (
            vec![
                ("foo".into(), vec![]),
                (format!("{l}bar{r}"), vec!["bold"]),
                ("baz".into(), vec![]),
            ],
            format!("fo&#111;**{l}bar{r}**&#98;az"),
        ),
        (
            vec![
                ("a".into(), vec![]),
                (g.into(), vec!["italic"]),
                ("b".into(), vec![]),
            ],
            format!("&#97;*{g}*&#98;"),
        ),
        (
            vec![
                ("a".into(), vec![]),
                ("x".into(), vec!["bold", "code"]),
                ("b".into(), vec![]),
            ],
            "&#97;**`x`**&#98;".into(),
        ),
        (
            vec![
                ("x.".into(), vec!["bold"]),
                ("b".into(), vec![]),
                ("(y)".into(), vec!["bold"]),
            ],
            "**x.**&#98;**(y)**".into(),
        ),
        // Italic closes and reopens around the bold's end.
        (
            vec![
                ("b".into(), vec!["bold", "italic"]),
                (format!("{l}y{r}"), vec!["italic"]),
            ],
            "***&#98;****“y”*".into(),
        ),
        // Markers of different characters are separate delimiter runs.
        (
            vec![
                ("a".into(), vec!["bold"]),
                ("!".into(), vec!["strikethrough"]),
            ],
            "**a**~~!~~".into(),
        ),
        // A one-character reference reads as punctuation on its other side.
        (
            vec![
                ("x".into(), vec![]),
                ("a".into(), vec!["bold"]),
                (format!("{l}b{r}"), vec!["italic"]),
            ],
            "&#120;**&#97;***“b”*".into(),
        ),
        (
            vec![
                (format!("{l}a{r}"), vec!["bold", "italic"]),
                ("b".into(), vec!["italic"]),
                ("c".into(), vec![]),
            ],
            "***“a”****&#98;*&#99;".into(),
        ),
        (
            vec![
                ("x".into(), vec![]),
                ("a".into(), vec!["bold"]),
                (format!("{l}b{r}"), vec!["bold", "italic"]),
            ],
            "&#120;**&#97;*“b”***".into(),
        ),
        // An `@` that is not part of a link where the text ends does not stop
        // the reference.
        (
            vec![
                ("联系我@微博你好".into(), vec![]),
                (format!("{l}重要{r}"), vec!["bold"]),
            ],
            format!("联系我@微博你&#22909;**{l}重要{r}**"),
        ),
        (
            vec![
                ("(a@b.co)and".into(), vec![]),
                (format!("{l}x{r}"), vec!["bold"]),
            ],
            format!("(a@b.co)an&#100;**{l}x{r}**"),
        ),
        // A word character beside a word character needs no reference.
        (
            vec![
                ("a".into(), vec![]),
                ("b".into(), vec!["bold"]),
                ("c".into(), vec![]),
            ],
            "a**b**c".into(),
        ),
        // A tag character that text does not escape is kept out of the marker.
        (
            vec![
                ("a=".into(), vec![]),
                (format!("{l}b{r}"), vec!["highlight"]),
                ("=c".into(), vec![]),
            ],
            "a&#61;==“b”==&#61;c".into(),
        ),
        // Emphasis that ends before a code span is read before the span.
        (
            vec![
                ("a.".into(), vec!["bold"]),
                ("x".into(), vec![]),
                ("c".into(), vec!["bold", "code"]),
            ],
            "**a.**&#120;**`c`**".into(),
        ),
        (
            vec![
                (l.into(), vec!["bold"]),
                (".b ".into(), vec!["italic"]),
                ("c".into(), vec!["italic", "code"]),
            ],
            "**“***.b `c`*".into(),
        ),
    ];
    let cases = table
        .into_iter()
        .map(|(runs, exported)| {
            case(
                format!(
                    "{EMPHASIS} > exports and reimports {} as {}",
                    runs_json(&runs),
                    j(&exported)
                ),
                move || {
                    let editor = editor();
                    let nodes = runs.clone();
                    update(&editor, move || {
                        get_root().append(&[paragraph(
                            nodes
                                .iter()
                                .map(|(t, formats)| {
                                    let node = text(t);
                                    for f in formats {
                                        node.toggle_format(f);
                                    }
                                    node
                                })
                                .collect(),
                        )])
                    });
                    let markdown = export(&editor);
                    assert_eq!(markdown, exported);
                    import(&editor, &markdown);
                    let read_back = read(&editor, || {
                        get_root()
                            .get_all_text_nodes()
                            .iter()
                            .map(|node| {
                                (
                                    node.get_text_content(),
                                    ["bold", "italic", "strikethrough", "highlight", "code"]
                                        .into_iter()
                                        .filter(|f| node.has_format(f))
                                        .collect::<Vec<_>>(),
                                )
                            })
                            .collect::<Vec<_>>()
                    });
                    assert_eq!(read_back, runs);
                },
            )
        })
        .collect();
    run_cases(SOURCE, FILE, "emphasis_exports_and_reimports", cases);
}

/// Text nodes and whether each has `format`.
fn text_and_format(f: &'static str) -> impl Fn() -> Vec<(String, bool)> {
    move || {
        get_root()
            .get_all_text_nodes()
            .iter()
            .map(|n| (n.get_text_content(), n.has_format(f)))
            .collect()
    }
}

fn pairs(table: &[(&str, bool)]) -> Vec<(String, bool)> {
    table.iter().map(|(t, b)| ((*t).to_owned(), *b)).collect()
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > guards a word character after a raw backslash when preserving newlines
#[test]
fn guards_a_word_character_after_a_raw_backslash_when_preserving_newlines() {
    let editor = editor();
    let quoted = format!("{LEFT_DOUBLE_QUOTE}y{RIGHT_DOUBLE_QUOTE}");
    let q = quoted.clone();
    update(&editor, move || {
        get_root().append(&[paragraph(vec![text(r"x\b"), {
            let n = text(&q);
            n.toggle_format("bold");
            n
        }])])
    });
    let markdown = export_full(&editor, &transformers(), true);
    assert_eq!(markdown, format!("x&#92;&#98;**{quoted}**"));
    import_full(&editor, &markdown, &transformers(), true, false);
    assert_eq!(
        read(&editor, text_and_format("bold")),
        pairs(&[(r"x\b", false), (&quoted, true)])
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > keeps a raw backslash from escaping a bold marker when preserving newlines
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > keeps a raw backslash from escaping a italic marker when preserving newlines
#[test]
fn keeps_a_raw_backslash_from_escaping_a_marker_when_preserving_newlines() {
    let cases = [("bold", r"a&#92;**b**"), ("italic", r"a&#92;*b*")]
        .into_iter()
        .map(|(f, exported)| {
            case(
                format!("{EMPHASIS} > keeps a raw backslash from escaping a {f} marker when preserving newlines"),
                move || {
                    let editor = editor();
                    update(&editor, move || {
                        let b = text("b");
                        b.toggle_format(f);
                        get_root().append(&[paragraph(vec![text(r"a\"), b])])
                    });
                    let markdown = export_full(&editor, &transformers(), true);
                    assert_eq!(markdown, exported);
                    import_full(&editor, &markdown, &transformers(), true, false);
                    assert_eq!(read(&editor, text_and_format(f)), pairs(&[(r"a\", false), ("b", true)]));
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "keeps_a_raw_backslash_from_escaping_a_marker_when_preserving_newlines",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > keeps raw backslashes from escaping an escape when preserving newlines
#[test]
fn keeps_raw_backslashes_from_escaping_an_escape_when_preserving_newlines() {
    let editor = editor();
    update(&editor, || {
        let c = text("c");
        c.toggle_format("bold");
        get_root().append(&[paragraph(vec![text(r"a\\_b_"), c])])
    });
    let markdown = export_full(&editor, &transformers(), true);
    assert_eq!(markdown, r"a\\\\\_b\_**c**");
    import_full(&editor, &markdown, &transformers(), true, false);
    assert_eq!(
        read(&editor, text_and_format("bold")),
        pairs(&[(r"a\\_b_", false), ("c", true)])
    );
}

/// A text match transformer with an `export`, `regExp` and a `replace`
/// that does nothing; no `importRegExp`.
fn export_and_reg_exp(
    reg_exp: &str,
    export: impl Fn(&NodeRef) -> Option<String> + 'static,
) -> Transformer {
    Transformer::TextMatch(Rc::new(TextMatchTransformer {
        dependencies: vec![],
        export: Some(Rc::new(move |node, _, _| Ok(export(node)))),
        import_reg_exp: None,
        reg_exp: RegExp::of(reg_exp, ""),
        replace: Some(Rc::new(|_: &NodeRef, _: &RegExpMatch| Ok(None))),
        get_end_index: None,
        trigger: None,
    }))
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes no reference that would break a run beside raw output
#[test]
fn writes_no_reference_that_would_break_a_run_beside_raw_output() {
    let mention = export_and_reg_exp("@x$", |node| {
        (node.is_text() && node.get_text_content() == "@x").then(|| "@x".to_owned())
    });
    let editor = editor();
    update(&editor, || {
        let a = text("a");
        a.toggle_format("bold");
        let y = text(&format!("{LEFT_DOUBLE_QUOTE}y{RIGHT_DOUBLE_QUOTE}"));
        y.toggle_format("bold");
        y.toggle_format("italic");
        get_root().append(&[paragraph(vec![text("@x"), a, y])])
    });
    let mut all = vec![mention];
    all.extend(transformers());
    let markdown = export_full(&editor, &all, false);
    assert!(!markdown.contains("&#97;"));
    import(&editor, &markdown);
    assert!(read(&editor, || {
        get_root()
            .get_all_text_nodes()
            .iter()
            .any(|n| n.get_text_content().starts_with('a') && n.has_format("bold"))
    }));
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes a reference after "@john", which readers do not autolink
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes a reference after "foo@bar", which readers do not autolink
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes a reference after "a://b", which readers do not autolink
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes a reference after "xhttp://b", which readers do not autolink
#[test]
fn writes_a_reference_after_which_readers_do_not_autolink() {
    let table = [
        ("@john", "@joh&#110;"),
        ("foo@bar", "foo@ba&#114;"),
        ("a://b", "a://&#98;"),
        ("xhttp://b", "xhttp://&#98;"),
    ];
    let cases = table
        .into_iter()
        .map(|(t, markdown)| {
            case(
                format!(
                    "{EMPHASIS} > writes a reference after {}, which readers do not autolink",
                    j(t)
                ),
                move || {
                    let editor = editor();
                    update(&editor, move || {
                        let x = text(&format!("{LEFT_DOUBLE_QUOTE}x{RIGHT_DOUBLE_QUOTE}"));
                        x.toggle_format("bold");
                        get_root().append(&[paragraph(vec![text(&format!("see {t}")), x])])
                    });
                    assert_eq!(
                        export(&editor),
                        format!("see {markdown}**{LEFT_DOUBLE_QUOTE}x{RIGHT_DOUBLE_QUOTE}**")
                    );
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "writes_a_reference_after_which_readers_do_not_autolink",
        cases,
    );
}

/// Text nodes with their formats.
fn text_formats() -> Vec<(String, f64)> {
    get_root()
        .get_all_text_nodes()
        .iter()
        .map(|n| (n.get_text_content(), format_of(n)))
        .collect()
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads formatting after a text match whose syntax has a letter beside it
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads formatting before a text match whose syntax has a letter beside it
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads formatting in italic after a text match whose syntax has a letter beside it
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads formatting nested after a text match whose syntax has a letter beside it
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads formatting far after a text match whose syntax has a letter beside it
#[test]
fn reads_formatting_beside_a_text_match_whose_syntax_has_a_letter_beside_it() {
    let far_inner = format!("({}y)", "x ".repeat(40));
    let table: Vec<(&str, String, (String, f64))> = vec![
        (
            "after",
            "user:bob**(admin)**".into(),
            ("(admin)".into(), 1.0),
        ),
        (
            "before",
            "**(admin)**user:bob".into(),
            ("(admin)".into(), 1.0),
        ),
        (
            "in italic after",
            "user:bob*\u{201c}a\u{201d}*".into(),
            ("\u{201c}a\u{201d}".into(), 2.0),
        ),
        (
            "nested after",
            "user:bob**(text *more*)**".into(),
            ("more".into(), 3.0),
        ),
        (
            "far after",
            format!("user:bob**{far_inner}**"),
            (far_inner.clone(), 1.0),
        ),
    ];
    let cases = table
        .into_iter()
        .map(|(where_, markdown, formatted_run)| {
            case(
                format!("{EMPHASIS} > reads formatting {where_} a text match whose syntax has a letter beside it"),
                move || {
                    // A mention-style transformer whose syntax starts and
                    // ends in a letter.
                    let mention = Transformer::TextMatch(Rc::new(TextMatchTransformer {
                        dependencies: vec!["link"],
                        export: Some(Rc::new(|node, _, _| {
                            Ok(node
                                .get_url()
                                .filter(|u| is_link_node(node) && u.starts_with("mention:"))
                                .map(|u| format!("user:{}", &u["mention:".len()..])))
                        })),
                        import_reg_exp: Some(RegExp::of(r"user:(\w+)", "")),
                        reg_exp: RegExp::of(r"user:(\w+)$", ""),
                        replace: Some(Rc::new(|text_node: &NodeRef, m: &RegExpMatch| {
                            let l = link_node(&format!("mention:{}", m.str(1)), vec![text(m.str(1))]);
                            text_node.replace(&l, false)?;
                            Ok(None)
                        })),
                        get_end_index: None,
                        trigger: None,
                    }));
                    let mut all = vec![mention];
                    all.extend(transformers());
                    let editor = editor();
                    import_full(&editor, &markdown, &all, false, false);
                    assert!(read(&editor, text_formats).contains(&formatted_run));
                    assert_eq!(export_full(&editor, &all, false), markdown);
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "reads_formatting_beside_a_text_match_whose_syntax_has_a_letter_beside_it",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a run after a link the same whatever follows it on the line, with a short run
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a run after a link the same whatever follows it on the line, with a long run
#[test]
fn reads_a_run_after_a_link_the_same_whatever_follows_it_on_the_line() {
    let table = [
        ("a short run", "#".to_owned()),
        ("a long run", format!("#{}", "x".repeat(80))),
    ];
    let cases = table
        .into_iter()
        .map(|(name, inner)| {
            case(
                format!("{EMPHASIS} > reads a run after a link the same whatever follows it on the line, with {name}"),
                move || {
                    let editor = editor();
                    for tail in [String::new(), " [b](v)".to_owned(), format!(" {}", "y".repeat(80))] {
                        import(&editor, &format!("[a](u)**{inner}*{tail}"));
                        let first: Vec<(String, f64)> =
                            read(&editor, text_formats).into_iter().take(3).collect();
                        assert_eq!(
                            first,
                            [
                                ("a".to_owned(), 0.0),
                                ("*".to_owned(), 0.0),
                                (inner.clone(), format("italic")),
                            ]
                        );
                    }
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "reads_a_run_after_a_link_the_same_whatever_follows_it_on_the_line",
        cases,
    );
}

/// `input.slice(start, end)` in UTF-16 code units.
fn slice16(s: &str, (start, end): (usize, usize)) -> String {
    let units: Vec<u16> = s.encode_utf16().collect();
    String::from_utf16(&units[start..end]).unwrap()
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > gives a text match found earlier in the line its named groups where they are
#[test]
fn gives_a_text_match_found_earlier_in_the_line_its_named_groups_where_they_are() {
    let ranges = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let out = ranges.clone();
    let mention = Transformer::TextMatch(Rc::new(TextMatchTransformer {
        dependencies: vec![],
        export: None,
        import_reg_exp: Some(RegExp::of(r"@(?<name>\w+)", "d")),
        reg_exp: RegExp::of(r"@(?<name>\w+)$", "d"),
        replace: Some(Rc::new(move |text_node: &NodeRef, m: &RegExpMatch| {
            let (_, group) = m
                .names
                .iter()
                .find(|(n, _)| n == "name")
                .expect("a name group");
            let name = m
                .ranges
                .get(*group)
                .copied()
                .flatten()
                .expect("its indices");
            let first = m
                .ranges
                .get(1)
                .copied()
                .flatten()
                .expect("group 1's indices");
            out.borrow_mut().push(slice16(&m.input, name));
            out.borrow_mut().push(slice16(&m.input, first));
            text_node.set_text_content(&slice16(&m.input, name));
            Ok(None)
        })),
        get_end_index: None,
        trigger: None,
    }));
    let mut all = vec![mention];
    all.extend(transformers());
    let editor = editor();
    import_full(&editor, "**a** @bob", &all, false, false);
    assert_eq!(*ranges.borrow(), ["bob", "bob"]);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes no reference for the `.` of `www.` that is a format's marker
#[test]
fn writes_no_reference_for_the_dot_of_www_that_is_a_formats_marker() {
    let dot = Transformer::TextFormat(Rc::new(TextFormatTransformer {
        format: vec!["italic"],
        tag: ".".to_owned(),
        intraword: None,
    }));
    let editor = editor();
    update(&editor, || {
        let x = text("x.co");
        x.toggle_format("italic");
        get_root().append(&[paragraph(vec![text("www"), x, text(" or www.y.co")])])
    });
    assert_eq!(export_full(&editor, &[dot], false), "www.x.co. or www.y.co");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes no reference for an email's `@` after a raw backslash
#[test]
fn writes_no_reference_for_an_emails_at_after_a_raw_backslash() {
    let editor = editor();
    update(&editor, || {
        let y = text("\u{201c}y\u{201d}");
        y.toggle_format("bold");
        get_root().append(&[paragraph(vec![text(r"x\@b.co"), y])])
    });
    assert_eq!(
        export_full(&editor, &transformers(), true),
        "x\\@b.c&#111;**\u{201c}y\u{201d}**"
    );
}

/// Exports runs of (text, format) with new lines preserved, expects
/// `markdown`, and reads it back as the same runs.
fn expect_preserved_runs_round_trip(runs: Vec<(String, f64)>, markdown: &str) {
    let editor = editor();
    let nodes = runs.clone();
    update(&editor, move || {
        get_root().append(&[paragraph(
            nodes.iter().map(|(t, f)| formatted(t, *f)).collect(),
        )])
    });
    assert_eq!(export_full(&editor, &transformers(), true), markdown);
    import_full(&editor, markdown, &transformers(), true, false);
    assert_eq!(read(&editor, text_formats), runs);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > spoils a URL for the reference to a backslash before a marker
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > spoils a URL for the reference to a backslash before a code span
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > spoils a URL for the reference to a tag character before its marker
#[test]
fn spoils_a_url_for_the_reference_to() {
    let table = [
        (
            "a backslash before a marker",
            vec![(r"see http://x.com/a\", 0.0), ("b", format("bold"))],
            "see http&#58;//x.com/a&#92;**b**",
        ),
        (
            "a backslash before a code span",
            vec![(r"see http://x.com/a\", 0.0), ("b", format("code"))],
            "see http&#58;//x.com/a&#92;`b`",
        ),
        (
            "a tag character before its marker",
            vec![("see https://x.com/?a=", 0.0), ("b", format("highlight"))],
            "see https&#58;//x.com/?a&#61;==b==",
        ),
    ];
    let cases = table
        .into_iter()
        .map(|(name, runs, markdown)| {
            case(
                format!("{EMPHASIS} > spoils a URL for the reference to {name}"),
                move || {
                    expect_preserved_runs_round_trip(
                        runs.iter().map(|(t, f)| ((*t).to_owned(), *f)).collect(),
                        markdown,
                    );
                },
            )
        })
        .collect();
    run_cases(SOURCE, FILE, "spoils_a_url_for_the_reference_to", cases);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes a reference after CJK punctuation that ends bold before a CJK character
#[test]
fn writes_a_reference_after_cjk_punctuation_that_ends_bold_before_a_cjk_character() {
    let editor = editor();
    update(&editor, || {
        let bold = text("\u{4f60}\u{597d}\u{3002}");
        bold.toggle_format("bold");
        get_root().append(&[paragraph(vec![bold, text("\u{4e16}\u{754c}")])])
    });
    let markdown = export(&editor);
    assert_eq!(markdown, "**\u{4f60}\u{597d}\u{3002}**&#19990;\u{754c}");
    import(&editor, &markdown);
    assert_eq!(
        read(&editor, text_and_format("bold")),
        pairs(&[
            ("\u{4f60}\u{597d}\u{3002}", true),
            ("\u{4e16}\u{754c}", false)
        ])
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports bold italic text followed by italic text so that it re-imports (#9443)
#[test]
fn exports_bold_italic_text_followed_by_italic_text_so_that_it_re_imports() {
    let editor = editor();
    update(&editor, || {
        let a = text("a");
        a.toggle_format("bold");
        a.toggle_format("italic");
        let b = text(": b");
        b.toggle_format("italic");
        get_root().append(&[paragraph(vec![a, b])])
    });
    let markdown = export(&editor);
    assert_eq!(markdown, "***&#97;****: b*");
    import(&editor, &markdown);
    assert_eq!(
        read(&editor, text_formats),
        [
            ("a".to_owned(), or(format("bold"), format("italic"))),
            (": b".to_owned(), format("italic")),
        ]
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes references around an underscore italic between letters
#[test]
fn writes_references_around_an_underscore_italic_between_letters() {
    let star = italic_star();
    let all: Vec<Transformer> = transformers()
        .into_iter()
        .filter(|t| !matches!(t, Transformer::TextFormat(f) if Rc::ptr_eq(f, &star)))
        .collect();
    let editor = editor();
    update(&editor, || {
        let b = text("b");
        b.toggle_format("italic");
        get_root().append(&[paragraph(vec![text("a"), b, text("c")])])
    });
    let markdown = export_full(&editor, &all, false);
    assert_eq!(markdown, "&#97;_b_&#99;");
    import_full(&editor, &markdown, &all, false, false);
    assert_eq!(
        read(&editor, text_and_format("italic")),
        pairs(&[("a", false), ("b", true), ("c", false)])
    );
}

/// A paragraph of `runs` (text, format), exported with `TRANSFORMERS`.
fn export_runs(runs: Vec<(&'static str, f64)>) -> String {
    let editor = editor();
    update(&editor, move || {
        get_root().append(&[paragraph(
            runs.iter().map(|(t, f)| formatted(t, *f)).collect(),
        )])
    });
    export(&editor)
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes a reference outside a URL inside a code span
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes a reference outside a URL that starts later in the run
#[test]
fn writes_a_reference_outside() {
    let table = [
        (
            "a URL inside a code span",
            vec![
                ("a", 0.0),
                ("\u{201c}b\u{201d}", format("bold")),
                ("http://y", format("code")),
            ],
            "&#97;**\u{201c}b\u{201d}**`http://y`",
        ),
        (
            "a URL that starts later in the run",
            vec![
                ("foo", 0.0),
                ("\u{201c}bar\u{201d}", format("bold")),
                ("(www.x.com)", 0.0),
            ],
            "fo&#111;**\u{201c}bar\u{201d}**(www.x.com)",
        ),
    ];
    let cases = table
        .into_iter()
        .map(|(name, runs, markdown)| {
            case(
                format!("{EMPHASIS} > writes a reference outside {name}"),
                move || {
                    assert_eq!(export_runs(runs), markdown);
                },
            )
        })
        .collect();
    run_cases(SOURCE, FILE, "writes_a_reference_outside", cases);
}

/// `[markdown, runs]` cases of `name %j`, each importing `markdown` with
/// `transformers` and comparing the text nodes' (text, format).
fn import_format_cases(
    name: &str,
    table: Vec<(&'static str, Vec<(&'static str, f64)>)>,
    transformers: fn() -> Vec<Transformer>,
) -> Cases {
    table
        .into_iter()
        .map(|(markdown, runs)| {
            case(
                format!("{EMPHASIS} > {}", name.replace("%j", &j(markdown))),
                move || {
                    let editor = editor();
                    import_full(&editor, markdown, &transformers(), false, false);
                    let expected: Vec<(String, f64)> =
                        runs.iter().map(|(t, f)| ((*t).to_owned(), *f)).collect();
                    assert_eq!(read(&editor, text_formats), expected);
                },
            )
        })
        .collect()
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a run beside another marker as CommonMark does in "**bold***~~gone~~*"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a run beside another marker as CommonMark does in "**a***_b_*"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a run beside another marker as CommonMark does in "**~~**a"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a run beside another marker as CommonMark does in "~~a*~~b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a run beside another marker as CommonMark does in "[a](u)*`c`)**"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a run beside another marker as CommonMark does in "**)*(.`*b!****_"
#[test]
fn reads_a_run_beside_another_marker_as_commonmark_does() {
    let (b, i, s, c) = (
        format("bold"),
        format("italic"),
        format("strikethrough"),
        format("code"),
    );
    let cases = import_format_cases(
        "reads a run beside another marker as CommonMark does in %j",
        vec![
            (
                "**bold***~~gone~~*",
                vec![("bold", b), ("*", 0.0), ("gone", s), ("*", 0.0)],
            ),
            (
                "**a***_b_*",
                vec![("a", b), ("*", 0.0), ("b", i), ("*", 0.0)],
            ),
            ("**~~**a", vec![("**~~**a", 0.0)]),
            ("~~a*~~b", vec![("~~a*~~b", 0.0)]),
            (
                "[a](u)*`c`)**",
                vec![("a", 0.0), ("*", 0.0), ("c", c), (")**", 0.0)],
            ),
            (
                "**)*(.`*b!****_",
                vec![(")", b), ("(.`b!", or(b, i)), ("_", 0.0)],
            ),
        ],
        transformers,
    );
    run_cases(
        SOURCE,
        FILE,
        "reads_a_run_beside_another_marker_as_commonmark_does",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a highlight inside a format as a line of its own, and GFM's markers as GFM does, in "**==`====**"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a highlight inside a format as a line of its own, and GFM's markers as GFM does, in "**~~`~~~~**"
#[test]
fn reads_a_highlight_inside_a_format_as_a_line_of_its_own() {
    let (b, h) = (format("bold"), format("highlight"));
    let cases = import_format_cases(
        "reads a highlight inside a format as a line of its own, and GFM's markers as GFM does, in %j",
        vec![
            ("**==`====**", vec![("`", or(b, h)), ("==", b)]),
            ("**~~`~~~~**", vec![("~~`~~~~", b)]),
        ],
        transformers,
    );
    run_cases(
        SOURCE,
        FILE,
        "reads_a_highlight_inside_a_format_as_a_line_of_its_own",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > opens a code span after an escaped backtick in "x\\``bb`x"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > opens a code span after an escaped backtick in "\\``a` `b`"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > opens a code span after an escaped backtick in "`a\\`"
#[test]
fn opens_a_code_span_after_an_escaped_backtick() {
    let c = format("code");
    let cases = import_format_cases(
        "opens a code span after an escaped backtick in %j",
        vec![
            (r"x\``bb`x", vec![("x`", 0.0), ("bb", c), ("x", 0.0)]),
            (
                r"\``a` `b`",
                vec![("`", 0.0), ("a", c), (" ", 0.0), ("b", c)],
            ),
            (r"`a\`", vec![(r"a\", c)]),
        ],
        transformers,
    );
    run_cases(
        SOURCE,
        FILE,
        "opens_a_code_span_after_an_escaped_backtick",
        cases,
    );
}

/// A text match transformer of `import_reg_exp`, `reg_exp` and `replace`.
fn text_match(
    import_reg_exp: &str,
    reg_exp: &str,
    replace: impl Fn(&NodeRef) -> Result<(), LexicalError> + 'static,
) -> Transformer {
    Transformer::TextMatch(Rc::new(TextMatchTransformer {
        dependencies: vec![],
        export: None,
        import_reg_exp: Some(RegExp::of(import_reg_exp, "")),
        reg_exp: RegExp::of(reg_exp, ""),
        replace: Some(Rc::new(move |node: &NodeRef, _: &RegExpMatch| {
            replace(node)?;
            Ok(None)
        })),
        get_end_index: None,
        trigger: None,
    }))
}

/// `[TAG, ...TRANSFORMERS]`, `TAG` replacing `:[a-z]+\*` with `X`.
fn with_cutting_tag() -> Vec<Transformer> {
    let mut all = vec![text_match(r":[a-z]+\*", r":[a-z]+\*$", |node| {
        node.set_text_content("X");
        Ok(())
    })];
    all.extend(transformers());
    all
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads the rest of a run that a text match cuts in ":a**b*" on its own
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads the rest of a run that a text match cuts in ":a***b*" on its own
#[test]
fn reads_the_rest_of_a_run_that_a_text_match_cuts_on_its_own() {
    let cases = import_format_cases(
        "reads the rest of a run that a text match cuts in %j on its own",
        vec![
            (":a**b*", vec![("X", 0.0), ("b", format("italic"))]),
            (":a***b*", vec![("X**b*", 0.0)]),
        ],
        with_cutting_tag,
    );
    run_cases(
        SOURCE,
        FILE,
        "reads_the_rest_of_a_run_that_a_text_match_cuts_on_its_own",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes a reference before "bwww.x.co", which readers do not autolink
#[test]
fn writes_a_reference_before_which_readers_do_not_autolink() {
    let cases = ["bwww.x.co"]
        .into_iter()
        .map(|t| {
            case(
                format!(
                    "{EMPHASIS} > writes a reference before {}, which readers do not autolink",
                    j(t)
                ),
                move || {
                    assert_eq!(
                        export_runs(vec![(".", format("bold")), (t, 0.0)]),
                        format!("**.**&#98;{}", &t[1..])
                    );
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "writes_a_reference_before_which_readers_do_not_autolink",
        cases,
    );
}

/// `MENTION`: `@\w+` formatted as code, trimming the start of the text
/// after it.
fn trimming_mention() -> Vec<Transformer> {
    let mut all = vec![text_match(r"@(\w+)", r"@(\w+)$", |node| {
        if let Some(next) = node.get_next_sibling().filter(NodeRef::is_text) {
            next.set_text_content(next.get_text_content().trim_start());
        }
        node.toggle_format("code");
        Ok(())
    })];
    all.extend(transformers());
    all
}

fn expect_trimming_mention(markdown: &str) {
    let editor = editor();
    import_full(&editor, markdown, &trimming_mention(), false, false);
    let i = format("italic");
    assert_eq!(
        read(&editor, text_formats),
        [
            ("@bob".to_owned(), format("code")),
            ("a".to_owned(), i),
            (" ".to_owned(), 0.0),
            ("b".to_owned(), i),
        ]
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > scans the text after a match again when its transformer changes it
#[test]
fn scans_the_text_after_a_match_again_when_its_transformer_changes_it() {
    expect_trimming_mention("@bob *a* *b*");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads the text after a match on its own when its transformer changes it
#[test]
fn reads_the_text_after_a_match_on_its_own_when_its_transformer_changes_it() {
    expect_trimming_mention("@bob _a_ _b_");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports the text after a match before the text inside it
#[test]
fn imports_the_text_after_a_match_before_the_text_inside_it() {
    let seen = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let out = seen.clone();
    let mut all = vec![text_match("@x", "@x$", move |node| {
        out.borrow_mut()
            .push(node.get_parent_or_throw()?.get_text_content());
        Ok(())
    })];
    all.extend(transformers());
    let editor = editor();
    import_full(&editor, "**_a_** @x", &all, false, false);
    assert_eq!(*seen.borrow(), ["_a_ @x"]);
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > guards a letter right after an inline link's destination
#[test]
fn guards_a_letter_right_after_an_inline_links_destination() {
    let editor = editor();
    update(&editor, || {
        get_root().append(&[paragraph(vec![
            link_node("https://e.com", vec![text("API")]),
            text("s"),
            formatted("(beta)", format("bold")),
            text(" end"),
        ])])
    });
    assert_eq!(export(&editor), "[API](https://e.com)&#115;**(beta)** end");
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a delimiter run cut by an earlier match as it was in "*)**a*"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a delimiter run cut by an earlier match as it was in "“_)__a(“[)_"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a delimiter run cut by an earlier match as it was in "*a**`x`*"
#[test]
fn reads_a_delimiter_run_cut_by_an_earlier_match_as_it_was() {
    let table = [
        ("*)**a*", vec![("*)*", false), ("a", true)]),
        ("“_)__a(“[)_", vec![("“_)_", false), ("a(“[)", true)]),
        (
            "*a**`x`*",
            vec![("a", true), ("*", false), ("x", false), ("*", false)],
        ),
    ];
    let cases = table
        .into_iter()
        .map(|(markdown, runs)| {
            case(
                format!(
                    "{EMPHASIS} > reads a delimiter run cut by an earlier match as it was in {}",
                    j(markdown)
                ),
                move || {
                    let editor = editor();
                    import(&editor, markdown);
                    assert_eq!(read(&editor, text_and_format("italic")), pairs(&runs));
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "reads_a_delimiter_run_cut_by_an_earlier_match_as_it_was",
        cases,
    );
}

/// Each text node's text, whether it is code and whether its parent is a
/// link.
fn code_and_link() -> Vec<(String, bool, bool)> {
    get_root()
        .get_all_text_nodes()
        .iter()
        .map(|n| {
            (
                n.get_text_content(),
                n.has_format("code"),
                n.get_parent().is_some_and(|p| is_link_node(&p)),
            )
        })
        .collect()
}

fn triples(table: &[(&str, bool, bool)]) -> Vec<(String, bool, bool)> {
    table
        .iter()
        .map(|(t, a, b)| ((*t).to_owned(), *a, *b))
        .collect()
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > keeps a code span after a stray `[` before a link in "Use [this](u) or the [`--flag` from [docs](u)"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > keeps a code span after a stray `[` before a link in "[a](b) [`x`[c](d)"
#[test]
fn keeps_a_code_span_after_a_stray_bracket_before_a_link() {
    let table = [
        (
            "Use [this](u) or the [`--flag` from [docs](u)",
            vec![
                ("Use ", false, false),
                ("this", false, true),
                (" or the [", false, false),
                ("--flag", true, false),
                (" from ", false, false),
                ("docs", false, true),
            ],
        ),
        (
            "[a](b) [`x`[c](d)",
            vec![
                ("a", false, true),
                (" [", false, false),
                ("x", true, false),
                ("c", false, true),
            ],
        ),
    ];
    let cases = table
        .into_iter()
        .map(|(markdown, nodes)| {
            case(
                format!(
                    "{EMPHASIS} > keeps a code span after a stray `[` before a link in {}",
                    j(markdown)
                ),
                move || {
                    let editor = editor();
                    import(&editor, markdown);
                    assert_eq!(read(&editor, code_and_link), triples(&nodes));
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "keeps_a_code_span_after_a_stray_bracket_before_a_link",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > applies a link before a later link that holds a code span
#[test]
fn applies_a_link_before_a_later_link_that_holds_a_code_span() {
    let editor = editor();
    import(&editor, "[a](https://u) and [`b`](https://v)");
    assert_eq!(
        read(&editor, code_and_link),
        triples(&[
            ("a", false, true),
            (" and ", false, false),
            ("b", true, true)
        ])
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > keeps emphasis around a link after a code span in "`c` *[a](b)*"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > keeps emphasis around a link after a code span in "`c` x *[a](b) `d`*"
#[test]
fn keeps_emphasis_around_a_link_after_a_code_span() {
    let table = [
        (
            "`c` *[a](b)*",
            vec![("c", false), (" ", false), ("a", true)],
        ),
        (
            "`c` x *[a](b) `d`*",
            vec![
                ("c", false),
                (" x ", false),
                ("a", true),
                (" ", true),
                ("d", true),
            ],
        ),
    ];
    let cases = table
        .into_iter()
        .map(|(markdown, runs)| {
            case(
                format!(
                    "{EMPHASIS} > keeps emphasis around a link after a code span in {}",
                    j(markdown)
                ),
                move || {
                    let editor = editor();
                    import(&editor, markdown);
                    assert_eq!(read(&editor, text_and_format("italic")), pairs(&runs));
                },
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "keeps_emphasis_around_a_link_after_a_code_span",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > keeps a link from taking part of a later code span
#[test]
fn keeps_a_link_from_taking_part_of_a_later_code_span() {
    let editor = editor();
    import(&editor, "**`a` [b** `c](d)`");
    let nodes = read(&editor, || {
        get_root()
            .get_all_text_nodes()
            .iter()
            .map(|n| {
                (
                    n.get_text_content(),
                    n.has_format("bold"),
                    n.has_format("code"),
                )
            })
            .collect::<Vec<_>>()
    });
    assert_eq!(
        nodes,
        triples(&[
            ("a", true, true),
            (" [b", true, false),
            (" ", false, false),
            ("c](d)", false, true)
        ])
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > guards word characters in a selection export
#[test]
fn guards_word_characters_in_a_selection_export() {
    let editor = editor();
    update(&editor, || {
        let bold = formatted(
            &format!("{LEFT_DOUBLE_QUOTE}bar{RIGHT_DOUBLE_QUOTE}"),
            format("bold"),
        );
        let after = text("baz");
        get_root().append(&[paragraph(vec![text("foo"), bold.clone(), after.clone()])])?;
        bold.select(Some(0), Some(0))?.point_set(
            crate::Which::Focus,
            after.get_key(),
            2,
            crate::PointType::Text,
        )
    });
    assert_eq!(
        export_selection(&editor),
        format!("**{LEFT_DOUBLE_QUOTE}bar{RIGHT_DOUBLE_QUOTE}**&#98;a")
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > guards word characters across an inline element without a transformer
#[test]
fn guards_word_characters_across_an_inline_element_without_a_transformer() {
    let builtin = link();
    let all: Vec<Transformer> = transformers()
        .into_iter()
        .filter(|t| !matches!(t, Transformer::TextMatch(m) if Rc::ptr_eq(m, &builtin)))
        .collect();
    let editor = editor();
    update(&editor, || {
        get_root().append(&[paragraph(vec![
            text("foo"),
            link_node(
                "https://example.com",
                vec![formatted(
                    &format!("{LEFT_DOUBLE_QUOTE}bar{RIGHT_DOUBLE_QUOTE}"),
                    format("bold"),
                )],
            ),
            text("baz"),
        ])])
    });
    assert_eq!(
        export_full(&editor, &all, false),
        format!("fo&#111;**{LEFT_DOUBLE_QUOTE}bar{RIGHT_DOUBLE_QUOTE}**&#98;az")
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::decimal character references that name no character > reads "a&#1114112;b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::decimal character references that name no character > reads "a&#9999999;b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::decimal character references that name no character > reads "a&#0;b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::decimal character references that name no character > reads "a&#55296;b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::decimal character references that name no character > reads "a&#12345678;b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::decimal character references that name no character > reads "a&#1114111;b"
// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::decimal character references that name no character > reads "a&#35;b"
#[test]
fn decimal_character_references_that_name_no_character() {
    let table = [
        ("a&#1114112;b", "a\u{FFFD}b"),
        ("a&#9999999;b", "a\u{FFFD}b"),
        ("a&#0;b", "a\u{FFFD}b"),
        ("a&#55296;b", "a\u{FFFD}b"),
        ("a&#12345678;b", "a&#12345678;b"),
        ("a&#1114111;b", "a\u{10FFFF}b"),
        ("a&#35;b", "a#b"),
    ];
    let cases = table
        .into_iter()
        .map(|(md, t)| {
            case(
                format!(
                    "decimal character references that name no character > reads {}",
                    j(md)
                ),
                move || assert_eq!(root_text(&import_markdown(md)), t),
            )
        })
        .collect();
    run_cases(
        SOURCE,
        FILE,
        "decimal_character_references_that_name_no_character",
        cases,
    );
}

// upstream: lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::decimal character references that name no character > reads one in a link destination and title
#[test]
fn reads_one_in_a_link_destination_and_title() {
    assert_eq!(
        import_links("[x](https://example.com/&#9999999; \"t&#1114112;\")"),
        [il(Some("t\u{FFFD}"), "https://example.com/\u{FFFD}")]
    );
}
