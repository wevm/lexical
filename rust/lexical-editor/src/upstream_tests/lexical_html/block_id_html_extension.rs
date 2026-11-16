//! Ports of `packages/lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts`.
//! The editors register `ListExtension`, as upstream's do.

#![cfg(all(feature = "extension", feature = "rich-text", feature = "list"))]

use super::super::support::update;
use super::support::{as_edited, set_block_text, with_ids};
use crate::dom::{DomNode, parse_from_string};
use crate::extension::{
    BlockHandleOptions, BlockHandleResult, BlockIdConfigOverrides, IsIdentifiedBlock,
    LexicalEditorWithDispose, Signal, block_id_extension, block_id_state,
    build_editor_from_extensions, get_block_by_id, get_block_handles, get_block_id,
    get_extension_dependency_from_editor, is_root_child_block, resolve_block_handle,
};
use crate::html::import::generate_nodes_from_dom_via_extension;
use crate::html::{
    BLOCK_HANDLE_ATTRIBUTE, BLOCK_ID_ATTRIBUTE, BLOCK_WRAPPER_ID_ATTRIBUTE, BlockIdHtmlConfig,
    BlockIdHtmlConfigOverrides, BlockIdHtmlOutput, block_id_html_extension,
    generate_dom_from_nodes, generate_html_from_nodes, get_session_dom_render_config,
};
use crate::node_state::set_state;
use crate::{
    Editor, InitialEditorStateType, LexicalExtension, NodeRef, config_extension,
    create_paragraph_node, create_text_node, define_extension, get_root,
};
use std::collections::HashSet;
use std::rc::Rc;

#[derive(Default)]
struct BuildOptions<'a> {
    /// Skip `asEdited`: the first content the editor gets is a load.
    fresh: bool,
    /// Ids that `generateId` hands out first, before the counter.
    ids: &'a [&'a str],
    /// `None` leaves `BlockIdHTMLExtension` out (`html: false`).
    html: Option<BlockIdHtmlConfigOverrides>,
    prefix: Option<&'a str>,
    /// Identify the root's children only.
    root_only: bool,
}

fn html(overrides: BlockIdHtmlConfigOverrides) -> Option<BlockIdHtmlConfigOverrides> {
    Some(overrides)
}

fn default_html() -> Option<BlockIdHtmlConfigOverrides> {
    html(BlockIdHtmlConfigOverrides::default())
}

fn build(options: BuildOptions<'_>) -> LexicalEditorWithDispose {
    let mut dependencies = vec![
        crate::rich_text::rich_text_extension().into(),
        crate::list::list_extension().into(),
        config_extension(
            &block_id_extension(),
            [BlockIdConfigOverrides {
                generate_id: Some(with_ids(options.ids, options.prefix.unwrap_or("b"))),
                is_identified_block: options
                    .root_only
                    .then(|| IsIdentifiedBlock(Rc::new(is_root_child_block))),
                ..Default::default()
            }],
        ),
    ];
    if let Some(overrides) = options.html {
        dependencies.push(config_extension(&block_id_html_extension(), [overrides]));
    }
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies,
        ..LexicalExtension::<(), (), ()>::new("[block-id-html]", ())
    })
    .into()])
    .unwrap();
    if !options.fresh {
        as_edited(&editor);
    }
    editor
}

fn build_with_html(overrides: BlockIdHtmlConfigOverrides) -> LexicalEditorWithDispose {
    build(BuildOptions {
        html: html(overrides),
        ..Default::default()
    })
}

fn build_prefix(prefix: &str) -> LexicalEditorWithDispose {
    build(BuildOptions {
        html: default_html(),
        prefix: Some(prefix),
        ..Default::default()
    })
}

/// `parseFixture(fixture)`.
fn parse_fixture(fixture: &str) -> DomNode {
    parse_from_string(&format!(
        "<!doctype html><html><body>{fixture}</body></html>"
    ))
}

/// `paragraphsDocument(entries)`: one element per entry, built with DOM
/// APIs.
fn paragraphs_document(entries: &[(Option<&str>, &str, &str)]) -> DomNode {
    let doc = parse_fixture("");
    let body = doc.body().unwrap();
    for (id, tag, text) in entries {
        let element = doc.create_element(tag);
        if let Some(id) = id {
            element.set_attribute("data-block-id", id);
        }
        element.set_text_content(text);
        body.append_child(&element);
    }
    doc
}

fn append(editor: &Editor, dom: &DomNode) {
    let dom = dom.clone();
    update(editor, move || {
        let nodes = generate_nodes_from_dom_via_extension(&dom, None)?;
        get_root().append(&nodes)
    });
}

/// The editor's content exported as a DOM document.
fn export_document(editor: &Editor) -> DomNode {
    let doc = parse_fixture("");
    let body = doc.body().unwrap();
    editor
        .read(|| generate_dom_from_nodes(&body, None, editor))
        .unwrap()
        .unwrap();
    doc
}

/// The exported document and the handles it shows, in one read.
fn export_with_handles(editor: &Editor) -> (DomNode, Vec<(String, String)>) {
    let doc = parse_fixture("");
    let body = doc.body().unwrap();
    let shown = editor
        .read(|| {
            generate_dom_from_nodes(&body, None, editor)?;
            get_block_handles()
        })
        .unwrap()
        .unwrap();
    (doc, shown)
}

fn export_html(editor: &Editor) -> String {
    editor
        .read(|| generate_html_from_nodes(editor, None))
        .unwrap()
        .unwrap()
}

/// `querySelectorAll('[attribute]')`.
fn with_attribute(doc: &DomNode, attribute: &str) -> Vec<DomNode> {
    doc.body()
        .unwrap()
        .descendant_elements()
        .into_iter()
        .filter(|e| e.has_attribute(attribute))
        .collect()
}

fn tree(editor: &Editor) -> Vec<String> {
    fn walk(node: &NodeRef, out: &mut Vec<String>) {
        if node.is_element() {
            if node.get_type() != "root" {
                let id = get_block_id(node).unwrap();
                out.push(format!(
                    "{}:{}",
                    node.get_type(),
                    id.as_deref().unwrap_or("null")
                ));
            }
            for child in node.get_children() {
                walk(&child, out);
            }
        }
    }
    editor
        .read(|| {
            let mut out = Vec::new();
            walk(&get_root(), &mut out);
            out
        })
        .unwrap()
}

fn id_of(entry: &str) -> &str {
    &entry[entry.find(':').unwrap() + 1..]
}

fn output(editor: &Editor) -> Rc<BlockIdHtmlOutput> {
    get_extension_dependency_from_editor(editor, &block_id_html_extension())
        .unwrap()
        .output
        .unwrap()
}

fn state_json(editor: &Editor) -> String {
    crate::json::stringify(&editor.get_editor_state().to_json(false)).unwrap()
}

fn is_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension > without it, HTML carries no ids
#[test]
fn without_it_html_carries_no_ids() {
    let editor = build(BuildOptions::default());
    update(&editor, || {
        let p = create_paragraph_node();
        p.append(&[create_text_node("one")])?;
        get_root().append(&[p])
    });
    assert!(!export_html(&editor).contains("data-block-id"));
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension > exports data-block-id on identified blocks only
#[test]
fn exports_data_block_id_on_identified_blocks_only() {
    let editor = build_with_html(Default::default());
    append(
        &editor,
        &parse_fixture("<h1>Title</h1><p>Text</p><ul><li>a</li><li>b</li></ul>"),
    );
    assert_eq!(
        tree(&editor),
        [
            "heading:b1",
            "paragraph:b2",
            "list:b3",
            "listitem:b4",
            "listitem:b5"
        ]
    );
    assert_eq!(
        export_html(&editor),
        [
            r#"<h1 data-block-id="b1"><span style="white-space: pre-wrap;">Title</span></h1>"#,
            r#"<p data-block-id="b2"><span style="white-space: pre-wrap;">Text</span></p>"#,
            r#"<ul data-block-id="b3">"#,
            r#"<li value="1" data-block-id="b4"><span style="white-space: pre-wrap;">a</span></li>"#,
            r#"<li value="2" data-block-id="b5"><span style="white-space: pre-wrap;">b</span></li>"#,
            "</ul>",
        ]
        .concat()
    );
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension > a root-only predicate exports no list item ids
#[test]
fn a_root_only_predicate_exports_no_list_item_ids() {
    let editor = build(BuildOptions {
        html: default_html(),
        root_only: true,
        ..Default::default()
    });
    append(&editor, &parse_fixture("<ul><li>a</li><li>b</li></ul>"));
    assert_eq!(tree(&editor), ["list:b1", "listitem:null", "listitem:null"]);
    assert_eq!(
        export_html(&editor),
        [
            r#"<ul data-block-id="b1">"#,
            r#"<li value="1"><span style="white-space: pre-wrap;">a</span></li>"#,
            r#"<li value="2"><span style="white-space: pre-wrap;">b</span></li>"#,
            "</ul>",
        ]
        .concat()
    );
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension > round-trips ids through HTML
#[test]
fn round_trips_ids_through_html() {
    let source = build_prefix("s");
    append(
        &source,
        &parse_fixture("<h2>Title</h2><p>Text</p><ul><li>a</li></ul><blockquote>q</blockquote>"),
    );
    let exported = export_document(&source);
    assert_eq!(with_attribute(&exported, "data-block-id").len(), 5);
    let target = build_prefix("t");
    append(&target, &exported);
    assert_eq!(tree(&target), tree(&source));
    assert_eq!(
        tree(&target),
        [
            "heading:s1",
            "paragraph:s2",
            "list:s3",
            "listitem:s4",
            "quote:s5"
        ]
    );
    source.dispose();
    target.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension > imported ids pass through the uniqueness rule
#[test]
fn imported_ids_pass_through_the_uniqueness_rule() {
    let editor = build_with_html(Default::default());
    append(&editor, &parse_fixture("<p>existing</p>"));
    let existing = tree(&editor)[0].clone();
    let existing_id = id_of(&existing).to_owned();
    append(
        &editor,
        &paragraphs_document(&[
            (Some("x"), "p", "first x"),
            (Some("x"), "p", "second x"),
            (Some(&existing_id), "p", "clashes with the document"),
            (Some("kept"), "h3", "kept"),
        ]),
    );
    let after = tree(&editor);
    assert_eq!(after[0], existing);
    assert_eq!(after[1], "paragraph:x");
    assert_eq!(after[4], "heading:kept");
    let ids: Vec<&str> = after.iter().map(|e| id_of(e)).collect();
    assert_eq!(ids.iter().collect::<HashSet<_>>().len(), ids.len());
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension > invalid imported ids are ignored; valid foreign ids are kept
#[test]
fn invalid_imported_ids_are_ignored_valid_foreign_ids_are_kept() {
    let editor = build_with_html(Default::default());
    let long_invalid = "z".repeat(65);
    let invalid = ["", &long_invalid, "a b", "line\nbreak", "\"><x", "é"];
    let longest = "y".repeat(64);
    let mut entries: Vec<(Option<&str>, &str, &str)> = invalid
        .iter()
        .map(|id| (Some(*id), "p", "invalid"))
        .collect();
    entries.push((Some(&longest), "p", "longest valid"));
    entries.push((Some("bk3j9x0a2zq"), "p", "an 11-character id"));
    append(&editor, &paragraphs_document(&entries));
    let after = tree(&editor);
    let ids: Vec<&str> = after.iter().map(|e| id_of(e)).collect();
    assert_eq!(ids[ids.len() - 2..], [longest.as_str(), "bk3j9x0a2zq"]);
    for id in &ids {
        assert!(
            (1..=64).contains(&id.len()) && id.chars().all(is_id_char),
            "{id:?}"
        );
        assert!(!invalid.contains(id));
    }
    assert_eq!(ids.iter().collect::<HashSet<_>>().len(), ids.len());
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension > an imported id is dropped from a block that is not identified
#[test]
fn an_imported_id_is_dropped_from_a_block_that_is_not_identified() {
    let editor = build_with_html(Default::default());
    append(
        &editor,
        &parse_fixture(
            r#"<ul data-block-id="outer"><li data-block-id="item">a<ul data-block-id="nested"><li>b</li></ul></li></ul>"#,
        ),
    );
    let outline = tree(&editor);
    assert_eq!(outline[0], "list:outer");
    assert_eq!(outline[1], "listitem:item");
    // The nested list is not identified: it keeps no id.
    assert!(outline.contains(&"list:null".to_owned()));
    assert!(!state_json(&editor).contains("\"nested\""));
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension > export: false leaves the export override out of every export session
#[test]
fn export_false_leaves_the_export_override_out_of_every_export_session() {
    let editor = build_with_html(BlockIdHtmlConfigOverrides {
        export: Some(false),
        ..Default::default()
    });
    append(&editor, &parse_fixture("<p>a</p>"));
    let out = output(&editor);
    // Whether an export session runs with the editor's own render config
    // (every override, this extension's included) or one compiled without
    // an override that its disabledForSession turned off. A Rust render
    // config is a struct of `Rc` hooks: the same config holds the same
    // hooks.
    let session_has_all_overrides = || {
        editor
            .read(|| {
                let session = get_session_dom_render_config(&editor).unwrap();
                let resident = crate::lexical_dom::get_editor_dom_render_config(&editor);
                Rc::ptr_eq(&session.export_dom, &resident.export_dom)
            })
            .unwrap()
    };
    assert!(!session_has_all_overrides());
    assert!(!export_html(&editor).contains("data-block"));
    // handles alone does not turn the export on.
    out.handles.set_value(true);
    assert!(!session_has_all_overrides());
    assert!(!export_html(&editor).contains("data-block"));
    out.export.set_value(true);
    assert!(session_has_all_overrides());
    assert!(export_html(&editor).contains("data-block-handle"));
    out.handles.set_value(false);
    assert!(session_has_all_overrides());
    assert!(export_html(&editor).contains("data-block-id"));
    out.export.set_value(false);
    assert!(!session_has_all_overrides());
    assert!(!export_html(&editor).contains("data-block"));
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension > the released types of export still hold: a boolean, and a Signal<boolean>
#[test]
fn the_released_types_of_export_still_hold() {
    let editor = build_with_html(BlockIdHtmlConfigOverrides {
        handles: Some(true),
        ..Default::default()
    });
    let out = output(&editor);
    // The types are checked by the compiler: bool fields, Signal<bool>
    // outputs.
    let on: bool = out.export.value();
    let export_signal: &Signal<bool> = &out.export;
    let _: &Signal<bool> = &out.handles;
    let _: &Signal<bool> = &out.import;
    // A whole config as written for v0.53.0, without handles.
    let config = BlockIdHtmlConfig {
        export: true,
        import: true,
        ..Default::default()
    };
    let partial = BlockIdHtmlConfigOverrides {
        export: Some(false),
        ..Default::default()
    };
    let _: bool = config.export;
    assert_eq!(
        [
            on,
            export_signal.value(),
            config.export,
            partial.export.unwrap()
        ],
        [true, true, true, false]
    );
    // The default configuration is unchanged, with handles off.
    let defaults = build_with_html(Default::default());
    assert!(!output(&defaults).handles.value());
    editor.dispose();
    defaults.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension > import: false ignores the attribute; export: false omits it
#[test]
fn import_false_ignores_the_attribute_export_false_omits_it() {
    let editor = build_with_html(BlockIdHtmlConfigOverrides {
        export: Some(false),
        import: Some(false),
        ..Default::default()
    });
    append(
        &editor,
        &parse_fixture(r#"<p data-block-id="from-html">a</p>"#),
    );
    assert_eq!(tree(&editor), ["paragraph:b1"]);
    assert!(!export_html(&editor).contains("data-block-id"));
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: documents read without ids > every reader exports the same data-block-id for a document stored without ids
#[test]
fn every_reader_exports_the_same_data_block_id() {
    let mut html: Vec<Vec<String>> = Vec::new();
    for prefix in ["x", "y"] {
        let editor = build(BuildOptions {
            fresh: true,
            html: default_html(),
            prefix: Some(prefix),
            ..Default::default()
        });
        // The first content of an empty editor is a load: positional ids,
        // not `generateId`'s.
        append(
            &editor,
            &paragraphs_document(&[(None, "p", "one"), (None, "h2", "two")]),
        );
        let ids: Vec<String> = with_attribute(&export_document(&editor), "data-block-id")
            .iter()
            .map(|e| e.get_attribute("data-block-id").unwrap())
            .collect();
        let stored: Vec<String> = editor
            .read(|| {
                get_root()
                    .get_children()
                    .iter()
                    .map(|n| get_block_id(n).unwrap().unwrap())
                    .collect()
            })
            .unwrap();
        assert_eq!(stored, ids);
        html.push(ids);
        editor.dispose();
    }
    assert_eq!(html[0], html[1]);
    for id in &html[0] {
        assert!(id.len() == 21 && id.chars().all(is_id_char), "{id:?}");
    }
}

const IDS: [&str; 5] = [
    "V1StGXR8_Z5jdHi6B-myT",
    "9xQeLm2Pq0aK7cTb_wYuA",
    "Rk8pZc41Uo-HsVd2NnFjB",
    // Its hash starts like the first id's: both handles take 5 characters.
    "qfapyr",
    "u7NnPz2Kq0aK7cTb_wYuA",
];

fn handles_editor(ids: &[&str]) -> LexicalEditorWithDispose {
    build(BuildOptions {
        html: html(BlockIdHtmlConfigOverrides {
            handles: Some(true),
            ..Default::default()
        }),
        ids,
        ..Default::default()
    })
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: handles: true > exports data-block-handle instead of data-block-id
#[test]
fn exports_data_block_handle_instead_of_data_block_id() {
    let editor = handles_editor(&IDS);
    append(
        &editor,
        &parse_fixture(
            "<h2>Q3 plan</h2><p>Revenue grew 4% in Q3.</p><ul><li>a</li><li>b</li></ul>",
        ),
    );
    assert_eq!(
        export_html(&editor),
        [
            r#"<h2 data-block-handle="2bNoq"><span style="white-space: pre-wrap;">Q3 plan</span></h2>"#,
            r#"<p data-block-handle="gTtF"><span style="white-space: pre-wrap;">Revenue grew 4% in Q3.</span></p>"#,
            r#"<ul data-block-handle="xt6z">"#,
            r#"<li value="1" data-block-handle="2bNoV"><span style="white-space: pre-wrap;">a</span></li>"#,
            r#"<li value="2" data-block-handle="Wbjb"><span style="white-space: pre-wrap;">b</span></li>"#,
            "</ul>",
        ]
        .concat()
    );
    assert_eq!(BLOCK_HANDLE_ATTRIBUTE, "data-block-handle");
    assert_eq!(BLOCK_ID_ATTRIBUTE, "data-block-id");
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: handles: true > export: true and export: false are unchanged
#[test]
fn export_true_and_export_false_are_unchanged() {
    let fixture = "<h2>Q3 plan</h2><p>Revenue grew 4% in Q3.</p><ul><li>a</li><li>b</li></ul>";
    let export_with = |config: Option<BlockIdHtmlConfigOverrides>| {
        let editor = build(BuildOptions {
            html: config,
            ids: &IDS,
            ..Default::default()
        });
        append(&editor, &parse_fixture(fixture));
        let out = export_html(&editor);
        editor.dispose();
        out
    };
    let default = export_with(default_html());
    let on = export_with(html(BlockIdHtmlConfigOverrides {
        export: Some(true),
        ..Default::default()
    }));
    let off = export_with(html(BlockIdHtmlConfigOverrides {
        export: Some(false),
        ..Default::default()
    }));
    let without = export_with(None);
    assert_eq!(on, default);
    assert_eq!(
        on,
        [
            r#"<h2 data-block-id="V1StGXR8_Z5jdHi6B-myT"><span style="white-space: pre-wrap;">Q3 plan</span></h2>"#,
            r#"<p data-block-id="9xQeLm2Pq0aK7cTb_wYuA"><span style="white-space: pre-wrap;">Revenue grew 4% in Q3.</span></p>"#,
            r#"<ul data-block-id="Rk8pZc41Uo-HsVd2NnFjB">"#,
            r#"<li value="1" data-block-id="qfapyr"><span style="white-space: pre-wrap;">a</span></li>"#,
            r#"<li value="2" data-block-id="u7NnPz2Kq0aK7cTb_wYuA"><span style="white-space: pre-wrap;">b</span></li>"#,
            "</ul>",
        ]
        .concat()
    );
    assert_eq!(off, without);
    assert!(!off.contains("data-block"));
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: handles: true > import never reads data-block-handle as an id
#[test]
fn import_never_reads_data_block_handle_as_an_id() {
    let editor = build(BuildOptions {
        html: default_html(),
        ids: &["abcdEFGH", "V1StGXR8_Z5jdHi6B-myT"],
        ..Default::default()
    });
    append(&editor, &parse_fixture("<p>existing</p>"));
    assert_eq!(tree(&editor), ["paragraph:abcdEFGH"]);
    append(
        &editor,
        &parse_fixture(concat!(
            // A handle of a block of this document, a handle that looks
            // like a whole id, and a handle next to a real id.
            r#"<p data-block-handle="abcd">handle of existing</p>"#,
            r#"<p data-block-handle="kept-if-read-as-id">no id</p>"#,
            r#"<h3 data-block-handle="zzzz" data-block-id="real-id">with id</h3>"#,
        )),
    );
    assert_eq!(
        tree(&editor),
        [
            "paragraph:abcdEFGH",
            "paragraph:V1StGXR8_Z5jdHi6B-myT",
            "paragraph:b1",
            "heading:real-id"
        ]
    );
    // With the import on, as by default, and an export of handles.
    let handles = handles_editor(&[]);
    append(
        &handles,
        &parse_fixture(r#"<p data-block-handle="from-html">a</p>"#),
    );
    assert_eq!(tree(&handles), ["paragraph:b1"]);
    editor.dispose();
    handles.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: handles: true > round trip: export handles, edit by handle, resolve
#[test]
fn round_trip_export_handles_edit_by_handle_resolve() {
    let editor = handles_editor(&IDS);
    append(
        &editor,
        &parse_fixture("<h2>Q3 plan</h2><p>Revenue grew 4% in Q3.</p><p>Hiring is on track.</p>"),
    );
    // 1. The HTML the model reads, and the handles it is shown, in one read.
    let (exported, shown) = export_with_handles(&editor);
    let elements = with_attribute(&exported, BLOCK_HANDLE_ATTRIBUTE);
    assert_eq!(with_attribute(&exported, BLOCK_ID_ATTRIBUTE).len(), 0);
    assert_eq!(
        elements
            .iter()
            .map(|e| e.get_attribute(BLOCK_HANDLE_ATTRIBUTE).unwrap())
            .collect::<Vec<_>>(),
        ["2bNo", "gTtF", "xt6z"]
    );
    for handle in ["2bNo", "gTtF", "xt6z"] {
        assert!(shown.iter().any(|(_, h)| h == handle), "{handle}");
    }
    let key_before = editor
        .read(|| {
            get_block_by_id(IDS[1])
                .unwrap()
                .unwrap()
                .get_key()
                .to_owned()
        })
        .unwrap();
    // 2. What the model answers.
    let revenue = elements
        .iter()
        .find(|e| e.text_content().starts_with("Revenue"))
        .unwrap();
    let ops = [
        (
            revenue.get_attribute(BLOCK_HANDLE_ATTRIBUTE).unwrap(),
            "Revenue grew 6% in Q3.",
        ),
        ("9xQf".to_owned(), "a typo"),
    ];
    // 3. Applied by the app, one update per operation, in place.
    let statuses = Rc::new(std::cell::RefCell::new(Vec::new()));
    for (block, text) in ops {
        let (statuses, shown) = (statuses.clone(), shown.clone());
        update(&editor, move || {
            let result =
                resolve_block_handle(&block, Some(&BlockHandleOptions { read: Some(shown) }))?;
            statuses.borrow_mut().push(result.status());
            if let BlockHandleResult::Found { node } = result {
                assert!(set_block_text(&node, text));
            }
            Ok(())
        });
    }
    assert_eq!(*statuses.borrow(), ["found", "not-found"]);
    editor
        .read(|| {
            let block = get_block_by_id(IDS[1]).unwrap().unwrap();
            assert_eq!(block.get_text_content(), "Revenue grew 6% in Q3.");
            // Changed in place: the same block.
            assert_eq!(block.get_key(), key_before);
        })
        .unwrap();
    assert_eq!(
        export_html(&editor),
        [
            r#"<h2 data-block-handle="2bNo"><span style="white-space: pre-wrap;">Q3 plan</span></h2>"#,
            r#"<p data-block-handle="gTtF"><span style="white-space: pre-wrap;">Revenue grew 6% in Q3.</span></p>"#,
            r#"<p data-block-handle="xt6z"><span style="white-space: pre-wrap;">Hiring is on track.</span></p>"#,
        ]
        .concat()
    );
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: handles: true > a handle from exported HTML never names a block added since whose handle would be the same
#[test]
fn a_handle_from_exported_html_never_names_a_block_added_since() {
    let editor = handles_editor(&["p18f", "Rk8pZc41"]);
    append(
        &editor,
        &parse_fixture("<p>Revenue grew 4%.</p><p>Hiring.</p>"),
    );
    let (exported, shown) = export_with_handles(&editor);
    assert_eq!(
        with_attribute(&exported, BLOCK_HANDLE_ATTRIBUTE)[0]
            .get_attribute(BLOCK_HANDLE_ATTRIBUTE)
            .as_deref(),
        Some("NnGk")
    );
    // Before the model's answer arrives, the block it was shown is deleted
    // and a block whose hash starts the same way is added.
    update(&editor, || {
        get_block_by_id("p18f")?.unwrap().remove(false)?;
        let node = create_paragraph_node();
        node.append(&[create_text_node("Not shown to the model.")])?;
        set_state(&node, &block_id_state(), Some("p2cs".to_owned()))?;
        get_root().append(&[node])
    });
    update(&editor, move || {
        // Without the read, the handle names the new block.
        let BlockHandleResult::Found { node } = resolve_block_handle("NnGk", None)? else {
            panic!("expected found");
        };
        assert_eq!(get_block_id(&node)?.as_deref(), Some("p2cs"));
        // With the handles of the export: not found.
        let result = resolve_block_handle("NnGk", Some(&BlockHandleOptions { read: Some(shown) }))?;
        assert_eq!(result.status(), "not-found");
        if let BlockHandleResult::Found { node } = result {
            set_block_text(&node, "Revenue grew 6%.");
        }
        Ok(())
    });
    editor
        .read(|| {
            assert_eq!(
                get_block_by_id("p2cs").unwrap().unwrap().get_text_content(),
                "Not shown to the model."
            );
        })
        .unwrap();
    editor.dispose();
}

const NESTED: &str = concat!(
    "<ul><li>first</li><li>second<ul><li>inner one</li>",
    "<li>inner two<ol><li>deep</li></ol></li></ul></li><li>third</li></ul>"
);

fn known_ids(outline: &[String]) -> Vec<String> {
    outline
        .iter()
        .map(|e| id_of(e).to_owned())
        .filter(|id| id != "null")
        .collect()
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: a list item that only wraps a nested list > exports its id on the nested list, and round-trips it
#[test]
fn wrapper_item_exports_its_id_on_the_nested_list_and_round_trips_it() {
    let source = build_prefix("s");
    append(&source, &parse_fixture(NESTED));
    let outline = tree(&source);
    // first, second, the inner list's wrapper, inner one, inner two, the
    // deep list's wrapper, deep, third.
    assert_eq!(
        outline
            .iter()
            .filter(|e| e.starts_with("listitem:"))
            .count(),
        8
    );
    let exported = export_document(&source);
    let wrapped = with_attribute(&exported, BLOCK_WRAPPER_ID_ATTRIBUTE);
    assert_eq!(
        wrapped.iter().map(DomNode::tag_name).collect::<Vec<_>>(),
        ["UL", "OL"]
    );
    // Every identified block's id is in the HTML exactly once.
    let mut written: Vec<String> = exported
        .body()
        .unwrap()
        .descendant_elements()
        .iter()
        .flat_map(|el| {
            [BLOCK_ID_ATTRIBUTE, BLOCK_WRAPPER_ID_ATTRIBUTE]
                .into_iter()
                .filter_map(|name| el.get_attribute(name))
                .collect::<Vec<_>>()
        })
        .collect();
    let mut ids = known_ids(&outline);
    written.sort();
    ids.sort();
    assert_eq!(written, ids);
    let target = build_prefix("t");
    append(&target, &exported);
    assert_eq!(tree(&target), outline);
    source.dispose();
    target.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: a list item that only wraps a nested list > pasted twice, the second copy gets fresh ids, the wrapper items too
#[test]
fn pasted_twice_the_second_copy_gets_fresh_ids() {
    let source = build_prefix("s");
    append(&source, &parse_fixture(NESTED));
    let exported = export_document(&source);
    let target = build_prefix("t");
    append(&target, &exported);
    append(&target, &exported);
    let ids = known_ids(&tree(&target));
    assert_eq!(ids.iter().collect::<HashSet<_>>().len(), ids.len());
    let source_tree = tree(&source);
    assert_eq!(tree(&target)[..source_tree.len()], source_tree[..]);
    source.dispose();
    target.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: a list item that only wraps a nested list > export: false writes no wrapper id; import: false reads none
#[test]
fn export_false_writes_no_wrapper_id_import_false_reads_none() {
    let off = build(BuildOptions {
        html: html(BlockIdHtmlConfigOverrides {
            export: Some(false),
            ..Default::default()
        }),
        prefix: Some("s"),
        ..Default::default()
    });
    append(&off, &parse_fixture(NESTED));
    assert!(with_attribute(&export_document(&off), BLOCK_WRAPPER_ID_ATTRIBUTE).is_empty());
    let source = build_prefix("s");
    append(&source, &parse_fixture(NESTED));
    let target = build(BuildOptions {
        html: html(BlockIdHtmlConfigOverrides {
            import: Some(false),
            ..Default::default()
        }),
        prefix: Some("t"),
        ..Default::default()
    });
    append(&target, &export_document(&source));
    assert!(!tree(&target).iter().any(|e| e.contains(":s")));
    off.dispose();
    source.dispose();
    target.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: a list item that only wraps a nested list > a wrapper id goes only on a block that holds that list alone
#[test]
fn a_wrapper_id_goes_only_on_a_block_that_holds_that_list_alone() {
    let editor = build_prefix("e");
    append(
        &editor,
        &parse_fixture(concat!(
            // A top-level list: its parent is the root, which takes no id.
            r#"<ul data-block-wrapper-id="top"><li>a</li></ul>"#,
            // A nested list after the item's text: the importer wraps it in
            // an item of its own, which takes the id.
            r#"<ul><li data-block-id="item">text"#,
            r#"<ul data-block-wrapper-id="w"><li>inner</li></ul></li></ul>"#,
        )),
    );
    let outline = tree(&editor);
    assert!(outline.contains(&"listitem:item".to_owned()));
    assert!(outline.contains(&"listitem:w".to_owned()));
    assert!(!outline.iter().any(|e| e.ends_with(":top")));
    assert!(!state_json(&editor).contains("\"top\""));
    editor.dispose();
}

// upstream: lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: a list item that only wraps a nested list > handles: true writes no wrapper id
#[test]
fn handles_true_writes_no_wrapper_id() {
    let editor = build(BuildOptions {
        html: html(BlockIdHtmlConfigOverrides {
            handles: Some(true),
            ..Default::default()
        }),
        prefix: Some("h"),
        ..Default::default()
    });
    append(&editor, &parse_fixture(NESTED));
    let (doc, _) = export_with_handles(&editor);
    assert!(with_attribute(&doc, BLOCK_WRAPPER_ID_ATTRIBUTE).is_empty());
    assert!(with_attribute(&doc, BLOCK_ID_ATTRIBUTE).is_empty());
    editor.dispose();
}
