//! Ports of `packages/lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts`.

use super::support::{Cases, j, run_cases};
use crate::constants::{TEXT_TYPE_TO_FORMAT, lookup};
use crate::link::{LinkAttributes, create_link_node, is_link_node, link_nodes};
use crate::markdown::{
    RegExp, RegExpMatch, TextMatchTransformer, Transformer, convert_from_markdown_string,
    generate_nodes_from_markdown_string, link, text_format_transformers,
};
use crate::upstream_tests::support::update_with;
use crate::{CreateEditorArgs, NodeRef, create_editor, create_text_node, get_root};
use std::rc::Rc;

const FILE: &str = "lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts";

/// `[...TEXT_FORMAT_TRANSFORMERS, LINK]`.
fn base_transformers() -> Vec<Transformer> {
    let mut all: Vec<Transformer> = text_format_transformers()
        .into_iter()
        .map(Transformer::TextFormat)
        .collect();
    all.push(Transformer::TextMatch(link()));
    all
}

/// `FORMAT_IN_PLACE`, with its `replace` (and `dependencies`) replaced.
fn format_in_place(
    replace: impl Fn(&NodeRef) -> Result<Option<NodeRef>, crate::LexicalError> + 'static,
) -> Rc<TextMatchTransformer> {
    Rc::new(TextMatchTransformer {
        dependencies: vec![],
        export: None,
        import_reg_exp: Some(RegExp::of("@[^ ]+", "")),
        reg_exp: RegExp::of("@[^ ]+", ""),
        replace: Some(Rc::new(move |node: &NodeRef, _: &RegExpMatch| {
            replace(node)
        })),
        get_end_index: None,
        trigger: None,
    })
}

/// `FORMAT_IN_PLACE`: toggles highlight on the matched node.
fn highlight_in_place() -> Rc<TextMatchTransformer> {
    format_in_place(|node| {
        node.toggle_format("highlight");
        Ok(None)
    })
}

#[derive(Debug, PartialEq)]
struct Read {
    format: f64,
    text: String,
    url: Option<String>,
}

/// `importText(markdown, transformers, detached)`.
fn import_text(markdown: &str, transformers: Vec<Transformer>, detached: bool) -> Vec<Read> {
    let editor = create_editor(CreateEditorArgs {
        nodes: link_nodes(),
        ..CreateEditorArgs::default()
    });
    let markdown = markdown.to_owned();
    update_with(&editor, move || {
        let container = if detached {
            let nodes =
                generate_nodes_from_markdown_string(&markdown, &transformers, false, false)?;
            let paragraph = nodes[0].clone();
            assert!(paragraph.is_element());
            assert!(!paragraph.is_attached());
            assert_eq!(get_root().get_children_size(), 0);
            paragraph
        } else {
            convert_from_markdown_string(&markdown, &transformers, None, false, false)?;
            get_root()
        };
        Ok(container
            .get_all_text_nodes()
            .iter()
            .map(|node| {
                let parent = node.get_parent();
                Read {
                    format: node.text_data().get_format(),
                    text: node.get_text_content(),
                    url: parent
                        .filter(is_link_node)
                        .and_then(|parent| parent.get_url()),
                }
            })
            .collect())
    })
}

fn joined(nodes: &[Read]) -> String {
    nodes.iter().map(|n| n.text.as_str()).collect()
}

fn format_of(name: &str) -> f64 {
    f64::from(lookup(TEXT_TYPE_TO_FORMAT, name).unwrap())
}

// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix "" > unescapes text formatted in place exactly once: @a\*b
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix "" > unescapes text formatted in place exactly once: @a\\\*b
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix "" > unescapes text formatted in place exactly once: @a&#65;b
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix "" > unescapes text formatted in place exactly once: @a&#38;#65;b
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix "" > unescapes text a link transformer declines: [a\*b]](url)
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix "" > unescapes text a link transformer declines: [a&#65;]](url)
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix "" > unescapes a matched node reused inside a link without returning it
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix "" > does not unescape a returned node twice
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix "" > keeps literal content when a transformer formats the node as code
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix "" > leaves newly generated replacement text literal
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix "prefix " > unescapes text formatted in place exactly once: @a\*b
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix "prefix " > unescapes text formatted in place exactly once: @a\\\*b
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix "prefix " > unescapes text formatted in place exactly once: @a&#65;b
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix "prefix " > unescapes text formatted in place exactly once: @a&#38;#65;b
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix "prefix " > unescapes text a link transformer declines: [a\*b]](url)
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix "prefix " > unescapes text a link transformer declines: [a&#65;]](url)
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix "prefix " > unescapes a matched node reused inside a link without returning it
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix "prefix " > does not unescape a returned node twice
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix "prefix " > keeps literal content when a transformer formats the node as code
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: false) > with prefix "prefix " > leaves newly generated replacement text literal
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix "" > unescapes text formatted in place exactly once: @a\*b
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix "" > unescapes text formatted in place exactly once: @a\\\*b
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix "" > unescapes text formatted in place exactly once: @a&#65;b
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix "" > unescapes text formatted in place exactly once: @a&#38;#65;b
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix "" > unescapes text a link transformer declines: [a\*b]](url)
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix "" > unescapes text a link transformer declines: [a&#65;]](url)
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix "" > unescapes a matched node reused inside a link without returning it
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix "" > does not unescape a returned node twice
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix "" > keeps literal content when a transformer formats the node as code
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix "" > leaves newly generated replacement text literal
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix "prefix " > unescapes text formatted in place exactly once: @a\*b
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix "prefix " > unescapes text formatted in place exactly once: @a\\\*b
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix "prefix " > unescapes text formatted in place exactly once: @a&#65;b
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix "prefix " > unescapes text formatted in place exactly once: @a&#38;#65;b
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix "prefix " > unescapes text a link transformer declines: [a\*b]](url)
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix "prefix " > unescapes text a link transformer declines: [a&#65;]](url)
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix "prefix " > unescapes a matched node reused inside a link without returning it
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix "prefix " > does not unescape a returned node twice
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix "prefix " > keeps literal content when a transformer formats the node as code
// upstream: lexical-markdown/src/__tests__/unit/RetainedTextMatchImport.test.ts::retained text matches (detached: true) > with prefix "prefix " > leaves newly generated replacement text literal
#[test]
fn retained_text_matches() {
    let mut cases: Cases = Vec::new();
    for detached in [false, true] {
        for prefix in ["", "prefix "] {
            let describe = format!(
                "retained text matches (detached: {detached}) > with prefix {}",
                j(prefix)
            );
            let read = move |markdown: &str, transformer: Option<Rc<TextMatchTransformer>>| {
                let mut all = Vec::new();
                if let Some(t) = transformer {
                    all.push(Transformer::TextMatch(t));
                }
                all.extend(base_transformers());
                import_text(&format!("{prefix}{markdown}"), all, detached)
            };
            for (md, text) in [
                (r"@a\*b", "@a*b"),
                (r"@a\\\*b", r"@a\*b"),
                ("@a&#65;b", "@aAb"),
                ("@a&#38;#65;b", "@a&#65;b"),
            ] {
                cases.push((
                    format!("{describe} > unescapes text formatted in place exactly once: {md}"),
                    Box::new(move || {
                        let nodes = read(md, Some(highlight_in_place()));
                        assert_eq!(joined(&nodes), format!("{prefix}{text}"));
                        assert_eq!(nodes.last().unwrap().format, format_of("highlight"));
                    }),
                ));
            }
            for (md, text) in [
                (r"[a\*b]](url)", "[a*b]](url)"),
                ("[a&#65;]](url)", "[aA]](url)"),
            ] {
                cases.push((
                    format!("{describe} > unescapes text a link transformer declines: {md}"),
                    Box::new(move || {
                        let nodes = read(md, None);
                        assert_eq!(joined(&nodes), format!("{prefix}{text}"));
                        assert!(nodes.iter().all(|n| n.url.is_none()));
                    }),
                ));
            }
            cases.push((
                format!(
                    "{describe} > unescapes a matched node reused inside a link without returning it"
                ),
                Box::new(move || {
                    let transformer = Rc::new(TextMatchTransformer {
                        dependencies: vec!["link"],
                        ..(*format_in_place(|node| {
                            let link = create_link_node(
                                "https://example.com",
                                &LinkAttributes::default(),
                            );
                            node.replace(&link, false)?;
                            link.append(std::slice::from_ref(node))?;
                            Ok(None)
                        }))
                        .clone()
                    });
                    let nodes = read(r"@a\\\*b", Some(transformer));
                    assert_eq!(
                        nodes.last().unwrap(),
                        &Read {
                            format: 0.0,
                            text: r"@a\*b".to_owned(),
                            url: Some("https://example.com".to_owned()),
                        }
                    );
                }),
            ));
            cases.push((
                format!("{describe} > does not unescape a returned node twice"),
                Box::new(move || {
                    let transformer = format_in_place(|node| {
                        let text: Vec<u16> = node.get_text_content().encode_utf16().collect();
                        node.set_text_content(&String::from_utf16_lossy(&text[1..]));
                        Ok(Some(node.clone()))
                    });
                    let nodes = read(r"@a\\\*b", Some(transformer));
                    assert_eq!(joined(&nodes), format!(r"{prefix}a\*b"));
                }),
            ));
            cases.push((
                format!(
                    "{describe} > keeps literal content when a transformer formats the node as code"
                ),
                Box::new(move || {
                    let text = r"@a\*&#65;b";
                    let transformer = format_in_place(|node| {
                        node.toggle_format("code");
                        Ok(None)
                    });
                    let nodes = read(text, Some(transformer));
                    assert_eq!(
                        nodes.last().unwrap(),
                        &Read {
                            format: format_of("code"),
                            text: text.to_owned(),
                            url: None,
                        }
                    );
                }),
            ));
            cases.push((
                format!("{describe} > leaves newly generated replacement text literal"),
                Box::new(move || {
                    let text = r"literal\*&#65;";
                    let transformer = format_in_place(move |node| {
                        node.replace(&create_text_node(text), false)?;
                        Ok(None)
                    });
                    let nodes = read("@replace", Some(transformer));
                    assert_eq!(joined(&nodes), format!("{prefix}{text}"));
                }),
            ));
        }
    }
    run_cases(
        include_str!("retained_text_match_import.rs"),
        FILE,
        "retained_text_matches",
        cases,
    );
}
