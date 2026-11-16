//! Ports of `packages/lexical-link/src/__tests__/unit/LinkImportExtension.test.ts`.

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::html::import::generate_nodes_from_dom_via_extension;
use crate::link::{is_link_node, link_extension};
use crate::{
    AnyLexicalExtensionArgument, Editor, LexicalExtension, NodeRef, UpdateOptions,
    define_extension, get_root,
};

fn build(name: &str, dependencies: Vec<AnyLexicalExtensionArgument>) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

fn build_editor() -> LexicalEditorWithDispose {
    build("link-host", vec![link_extension().into()])
}

fn import_into(editor: &Editor, html: &str) {
    let html = format!("<!doctype html><html><body>{html}</body></html>");
    editor
        .update(
            move || {
                let dom = crate::dom::parse_from_string(&html);
                let nodes = generate_nodes_from_dom_via_extension(&dom, None)?;
                let root = get_root();
                root.clear()?;
                root.append(&nodes)
            },
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
}

fn is_paragraph(node: &NodeRef) -> bool {
    node.get_type() == "paragraph"
}

fn first_link() -> NodeRef {
    let para = get_root().get_first_child().unwrap();
    assert!(is_paragraph(&para), "expected paragraph");
    let link = para.get_first_child().unwrap();
    assert!(is_link_node(&link), "expected LinkNode");
    link
}

// upstream: lexical-link/src/__tests__/unit/LinkImportExtension.test.ts::LinkImportExtension > <a href="…">text</a> → LinkNode with TextNode child
#[test]
fn a_href_text_link_node_with_text_node_child() {
    let editor = build_editor();
    import_into(&editor, r#"<p><a href="https://example.com">click</a></p>"#);
    editor
        .read(|| {
            let link = first_link();
            assert_eq!(link.get_url().as_deref(), Some("https://example.com"));
            assert_eq!(link.get_text_content(), "click");
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LinkImportExtension.test.ts::LinkImportExtension > rel, target, title preserved
#[test]
fn rel_target_title_preserved() {
    let editor = build_editor();
    import_into(
        &editor,
        r#"<p><a href="/x" target="_blank" rel="noopener" title="hello">x</a></p>"#,
    );
    editor
        .read(|| {
            let link = first_link();
            assert_eq!(link.get_target().as_deref(), Some("_blank"));
            assert_eq!(link.get_rel().as_deref(), Some("noopener"));
            assert_eq!(link.get_title().as_deref(), Some("hello"));
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LinkImportExtension.test.ts::LinkImportExtension > empty <a> with no children is skipped
#[test]
fn empty_a_with_no_children_is_skipped() {
    let editor = build_editor();
    import_into(&editor, r#"<p>before<a href="/x"></a>after</p>"#);
    editor
        .read(|| {
            let para = get_root().get_first_child().unwrap();
            assert!(is_paragraph(&para), "expected paragraph");
            assert_eq!(para.get_text_content(), "beforeafter");
        })
        .unwrap();
}

// LinkImportExtension — block children lifted out of inline parent

#[cfg(feature = "rich-text")]
fn build_rich_editor() -> LexicalEditorWithDispose {
    build(
        "rich-link-host",
        vec![
            link_extension().into(),
            crate::rich_text::rich_text_extension().into(),
        ],
    )
}

// upstream: lexical-link/src/__tests__/unit/LinkImportExtension.test.ts::LinkImportExtension — block children lifted out of inline parent > <a><h1>x</h1><div>y</div></a> lifts the heading and re-wraps both runs with the link
#[cfg(feature = "rich-text")]
#[test]
fn a_h1_div_lifts_the_heading_and_re_wraps_both_runs_with_the_link() {
    use crate::rich_text::is_heading_node;
    let editor = build_rich_editor();
    import_into(
        &editor,
        r#"<a href="https://example.com"><h1>some text</h1><div>more text</div></a>"#,
    );
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 2);
            assert!(
                is_heading_node(&children[0]),
                "expected HeadingNode at index 0"
            );
            let heading_link = children[0].get_first_child().unwrap();
            assert!(
                is_link_node(&heading_link),
                "expected LinkNode inside heading"
            );
            assert_eq!(
                heading_link.get_url().as_deref(),
                Some("https://example.com")
            );
            assert_eq!(heading_link.get_text_content(), "some text");
            assert!(
                is_paragraph(&children[1]),
                "expected ParagraphNode at index 1"
            );
            let paragraph_link = children[1].get_first_child().unwrap();
            assert!(
                is_link_node(&paragraph_link),
                "expected LinkNode inside paragraph"
            );
            assert_eq!(
                paragraph_link.get_url().as_deref(),
                Some("https://example.com")
            );
            assert_eq!(paragraph_link.get_text_content(), "more text");
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LinkImportExtension.test.ts::LinkImportExtension — block children lifted out of inline parent > <a><h1>x</h1>y<h1>z</h1></a> wraps the middle inline run in its own paragraph + link
#[cfg(feature = "rich-text")]
#[test]
fn a_h1_y_h1_wraps_the_middle_inline_run_in_its_own_paragraph_and_link() {
    use crate::rich_text::is_heading_node;
    let editor = build_rich_editor();
    import_into(&editor, r#"<a href="/u"><h1>x</h1>middle<h1>z</h1></a>"#);
    editor
        .read(|| {
            let children = get_root().get_children();
            assert_eq!(children.len(), 3);
            let [h1, mid, h3] = &children[..] else {
                unreachable!()
            };
            assert!(is_heading_node(h1), "expected HeadingNode");
            let h1_link = h1.get_first_child().unwrap();
            assert!(is_link_node(&h1_link), "expected LinkNode");
            assert_eq!(h1_link.get_text_content(), "x");
            assert!(is_paragraph(mid), "expected ParagraphNode");
            let mid_link = mid.get_first_child().unwrap();
            assert!(is_link_node(&mid_link), "expected LinkNode");
            assert_eq!(mid_link.get_text_content(), "middle");
            // Each lifted block gets its own fresh link wrapper.
            assert_ne!(mid_link, h1_link);
            assert!(is_heading_node(h3), "expected HeadingNode");
            let h3_link = h3.get_first_child().unwrap();
            assert!(is_link_node(&h3_link), "expected LinkNode");
            assert_eq!(h3_link.get_text_content(), "z");
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LinkImportExtension.test.ts::LinkImportExtension — block children lifted out of inline parent > all-inline <a> retains the single-wrapping fast path
#[cfg(feature = "rich-text")]
#[test]
fn all_inline_a_retains_the_single_wrapping_fast_path() {
    let editor = build_rich_editor();
    import_into(&editor, r#"<p><a href="/x">just text</a></p>"#);
    editor
        .read(|| {
            let link = first_link();
            assert_eq!(link.get_text_content(), "just text");
            assert_eq!(link.get_url().as_deref(), Some("/x"));
        })
        .unwrap();
}
