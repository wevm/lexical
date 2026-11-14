use crate::case::{Case, Outcome, same};
use lexical::json::Value;
use lexical::link::{LinkAttributes, create_link_node};
use lexical::markdown::{
    MultilineElementTransformer, MultilineImportArgs, MultilineImportResult, RegExp, RegExpMatch,
    TextMatchTransformer, Transformer, check_list, code, convert_from_markdown_string,
    convert_to_markdown_string, link, transformers,
};
use lexical::{
    CreateEditorArgs, Editor, LexicalError, NodeRef, UpdateOptions, create_editor,
    create_text_node, get_selection, utf16,
};
use std::rc::Rc;

/// `markdown/import-export.json`, upstream LexicalMarkdown.test.ts's
/// `IMPORT_AND_EXPORT` table. Each `md` imports, in a discrete update, to
/// the editor state JS made from it, without selecting ("should not select
/// when importing"). Each `exportState`, the state JS built from the
/// table's `html`, exports to the markdown JS wrote from it.
///
/// JS built that state with `$generateNodesFromDOM`, and checks the
/// `importHtml` it rendered with `$generateHtmlFromNodes`; both wait for
/// `@lexical/html` (rust/PARITY.md), so this reads the state from its JSON
/// and leaves `importHtml` unchecked.
pub fn check(case: &Value, _file: &Value) -> Result<Outcome, String> {
    let custom = custom_transformers(case)?;
    let flag = |key: &str| case.get(key).and_then(Value::as_bool).unwrap_or(false);
    let should_preserve_new_lines = flag("shouldPreserveNewLines");
    let should_merge_adjacent_lines = flag("shouldMergeAdjacentLines");

    if let Some(import_state) = case.get("importState").and_then(Value::as_str) {
        let md = case.str("md")?.to_owned();
        let editor = create_test_editor();
        let mut all = custom.clone();
        all.extend(transformers());
        all.push(Transformer::TextMatch(highlight_text_match_import()));
        editor
            .update(
                move || {
                    convert_from_markdown_string(
                        &md,
                        &all,
                        None,
                        should_preserve_new_lines,
                        should_merge_adjacent_lines,
                    )
                },
                UpdateOptions {
                    discrete: true,
                    ..UpdateOptions::default()
                },
            )
            .map_err(|e| format!("importing: {e}"))?;
        same(
            "imported state",
            editor
                .get_editor_state()
                .to_json(false)
                .to_string()
                .as_str(),
            import_state,
        )?;
        let selected = editor
            .read(|| get_selection().is_some())
            .map_err(|e| format!("reading the selection: {e}"))?;
        same("a selection after importing", selected, false)?;
    }

    if let Some(export_state) = case.get("exportState").and_then(Value::as_str) {
        let editor = create_test_editor();
        let state = editor
            .parse_editor_state(export_state)
            .map_err(|e| format!("parsing exportState: {e}"))?;
        editor
            .set_editor_state(&state, None)
            .map_err(|e| format!("setting exportState: {e}"))?;
        let mut all = custom;
        all.extend(transformers());
        let md = editor
            .read(|| convert_to_markdown_string(&all, None, should_preserve_new_lines))
            .and_then(|r| r)
            .map_err(|e| format!("exporting: {e}"))?;
        same("exported markdown", md.as_str(), case.str("exportMd")?)?;
    }
    Ok(Outcome::Pass)
}

/// `createTestEditor()`: heading, list, list item, quote, code and link.
fn create_test_editor() -> Editor {
    create_editor(CreateEditorArgs {
        nodes: [
            lexical::rich_text::rich_text_nodes(),
            lexical::list::list_nodes(),
            lexical::code::code_nodes(),
            lexical::link::link_nodes(),
        ]
        .concat(),
    })
}

/// The custom transformers a case names, ported from upstream
/// LexicalMarkdown.test.ts (copied into packages/lexical-conformance's
/// markdown.test.ts).
fn custom_transformers(case: &Value) -> Result<Vec<Transformer>, String> {
    let Some(names) = case.get("customTransformers") else {
        return Ok(Vec::new());
    };
    names
        .as_array()
        .ok_or("customTransformers is not an array")?
        .iter()
        .map(|name| {
            Ok(match name.as_str() {
                Some("CHECK_LIST") => Transformer::Element(check_list()),
                Some("CODE_TAG_COUNTER_EXAMPLE") => {
                    Transformer::MultilineElement(code_tag_counter_example())
                }
                Some("HIGHLIGHT_TEXT_MATCH_IMPORT") => {
                    Transformer::TextMatch(highlight_text_match_import())
                }
                Some("MDX_HTML_TRANSFORMER") => {
                    Transformer::MultilineElement(mdx_html_transformer())
                }
                Some("SIMPLE_INLINE_JSX_MATCHER") => {
                    Transformer::TextMatch(simple_inline_jsx_matcher())
                }
                _ => return Err(format!("unknown custom transformer {name}")),
            })
        })
        .collect()
}

/// `HIGHLIGHT_TEXT_MATCH_IMPORT`: `LINK` importing `$text$` as highlighted
/// text.
fn highlight_text_match_import() -> Rc<TextMatchTransformer> {
    Rc::new(TextMatchTransformer {
        import_reg_exp: Some(RegExp::of(r"\$([^$]+?)\$", "")),
        replace: Some(Rc::new(|text_node: &NodeRef, _m: &RegExpMatch| {
            text_node.toggle_format("highlight");
            Ok(None)
        })),
        ..(*link()).clone()
    })
}

const MY_TAG_END: &str = "</MyTag>";

/// `SIMPLE_INLINE_JSX_MATCHER`: `<MyTag>text</MyTag>` imports as a link
/// to `simple-jsx`, with nested tags counted to find the closing one.
fn simple_inline_jsx_matcher() -> Rc<TextMatchTransformer> {
    Rc::new(TextMatchTransformer {
        dependencies: vec!["link"],
        export: None,
        get_end_index: Some(Rc::new(|node: &NodeRef, m: &RegExpMatch| {
            let mut opened_sub_start_matches = 0;
            let start = m.index + m.match_len();
            let mut end_index = start;
            let line: Vec<u16> = node.get_text_content().encode_utf16().collect();
            let lt = u16::from(b'<');
            let slash = u16::from(b'/');
            for i in start..line.len() {
                if line[i] == lt {
                    if line.get(i + 1) == Some(&slash) {
                        if opened_sub_start_matches == 0 {
                            end_index = i + utf16::len(MY_TAG_END);
                            break;
                        }
                        opened_sub_start_matches -= 1;
                    } else {
                        opened_sub_start_matches += 1;
                    }
                }
            }
            Ok(Some(end_index))
        })),
        import_reg_exp: Some(RegExp::of(r"<(MyTag)\s*>", "")),
        reg_exp: RegExp::of("__ignore__", ""),
        replace: Some(Rc::new(|text_node: &NodeRef, m: &RegExpMatch| {
            let link_node = create_link_node("simple-jsx", &LinkAttributes::default());
            let text_start = m.match_len() + m.index;
            let text_end = (m.index + utf16::len(&text_node.get_text_content()))
                .saturating_sub(utf16::len(MY_TAG_END));
            let text = if text_start < text_end {
                utf16::slice(&m.input, text_start, text_end)
            } else {
                String::new()
            };
            let link_text_node = create_text_node(&text);
            link_text_node.set_format(text_node.text_data().get_format());
            link_node.append(&[link_text_node])?;
            text_node.replace(&link_node, false)?;
            Ok(None)
        })),
        trigger: None,
    })
}

/// `MDX_HTML_TRANSFORMER`: `<MyComponent>` to its closing tag imports as a
/// `MyComponent` code block of `From HTML: ` and the lines between, and
/// exports back.
fn mdx_html_transformer() -> Rc<MultilineElementTransformer> {
    Rc::new(MultilineElementTransformer {
        handle_import_after_start_match: None,
        dependencies: vec!["code"],
        export: Some(Rc::new(|node, _export_children, _selection| {
            let text = node.get_text_content();
            Ok(text.starts_with("From HTML:").then(|| {
                format!(
                    "<MyComponent>{}</MyComponent>",
                    text.replacen("From HTML: ", "", 1)
                )
            }))
        })),
        reg_exp_end: Some(lexical::markdown::RegExpEnd {
            optional: false,
            reg_exp: RegExp::of(r"<\/(\w+)\s*>", ""),
        }),
        reg_exp_start: RegExp::of(r"<(\w+)[^>]*>", ""),
        replace: Rc::new(
            |root_node, _children, start_match, _end_match, lines_in_between, _is_import| {
                let Some(lines) = lines_in_between else {
                    return Ok(false);
                };
                if start_match.get(1) == Some("MyComponent") {
                    let code_block_node = lexical::code::create_code_node(start_match.get(1), None);
                    let text_node = create_text_node(&format!("From HTML: {}", lines.join("\n")));
                    code_block_node.append(&[text_node])?;
                    root_node.append(&[code_block_node])?;
                    return Ok(true);
                }
                Ok(false)
            },
        ),
    })
}

/// `CODE_TAG_COUNTER_EXAMPLE`: `CODE`, with nested fences counted to find
/// the closing one.
fn code_tag_counter_example() -> Rc<MultilineElementTransformer> {
    let code = code();
    Rc::new(MultilineElementTransformer {
        handle_import_after_start_match: Some(Rc::new(code_tag_counter_import)),
        dependencies: code.dependencies.clone(),
        export: code.export.clone(),
        reg_exp_start: code.reg_exp_start.clone(),
        reg_exp_end: None,
        replace: code.replace.clone(),
    })
}

/// `text.slice(start, -end)`.
fn slice_both(text: &str, start: usize, end: usize) -> String {
    let len = utf16::len(text);
    utf16::slice(text, start, len.saturating_sub(end))
}

/// `CODE_TAG_COUNTER_EXAMPLE.handleImportAfterStartMatch`.
fn code_tag_counter_import(
    args: &MultilineImportArgs<'_>,
) -> Result<MultilineImportResult, LexicalError> {
    let MultilineImportArgs {
        lines,
        root_node,
        start_line_index,
        start_match,
        ..
    } = *args;
    let regexp_end_regex = RegExp::of(r"[ \t]*```$", "");
    let sub_start_regex = RegExp::of(r"^[ \t]*```(\w+)?", "");
    let is_end_optional = false;
    let mut end_line_index = start_line_index;
    let lines_length = lines.len();
    let mut opened_sub_start_matches: i64 = 0;

    while end_line_index < lines_length {
        let potential_sub_start_match = sub_start_regex.match_first(&lines[end_line_index]);
        let end_match = regexp_end_regex.match_first(&lines[end_line_index]);
        if let Some(sub) = &potential_sub_start_match {
            match &end_match {
                Some(end) => {
                    if sub.index < end.index {
                        opened_sub_start_matches += 1;
                    }
                }
                None => opened_sub_start_matches += 1,
            }
        }
        if end_match.is_some() {
            opened_sub_start_matches -= 1;
        }
        if (end_match.is_none() || opened_sub_start_matches > 0)
            && (!is_end_optional || end_line_index < lines_length - 1)
        {
            end_line_index += 1;
            continue;
        }
        if let Some(end) = &end_match
            && start_line_index == end_line_index
            && end.index == start_match.index
        {
            end_line_index += 1;
            continue;
        }
        let mut lines_in_between = Vec::new();
        match &end_match {
            Some(end) if start_line_index == end_line_index => {
                lines_in_between.push(slice_both(
                    &lines[start_line_index],
                    start_match.match_len(),
                    end.match_len(),
                ));
            }
            _ => {
                for (i, line) in lines
                    .iter()
                    .enumerate()
                    .take(end_line_index + 1)
                    .skip(start_line_index)
                {
                    if i == start_line_index {
                        lines_in_between.push(utf16::slice_from(line, start_match.match_len()));
                    } else if let Some(end) = end_match.as_ref().filter(|_| i == end_line_index) {
                        lines_in_between.push(slice_both(line, 0, end.match_len()));
                    } else {
                        lines_in_between.push(line.clone());
                    }
                }
            }
        }
        if (code().replace)(
            root_node,
            None,
            start_match,
            end_match.as_ref(),
            Some(lines_in_between),
            true,
        )? {
            return Ok(MultilineImportResult::Handled(true, end_line_index));
        }
        break;
    }
    Ok(MultilineImportResult::Handled(false, start_line_index))
}
