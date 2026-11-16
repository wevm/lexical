//! Ports of `packages/lexical-link/src/__tests__/unit/AutoLinkAnnounceExtension.test.ts`.
//!
//! The live region is a [`LiveRegion`] mounted on the root element's
//! [`LiveRegionHost`] instead of a `div[aria-live]` in its document, as in
//! Swift's port. JS's `clearLiveRegion` empties the region's
//! `textContent`, which only the extension writes here: the host notes the
//! point of the clear, and reads an empty region until the next write.

use crate::a11y::{LiveRegion, LiveRegionHost, aria_live_region_extension};
use crate::extension::{
    LexicalEditorWithDispose, build_editor_from_extensions, get_extension_dependency_from_editor,
};
use crate::link::{
    AutoLinkAnnounceExtensionConfigOverrides, AutoLinkAttributes, AutoLinkConfigOverrides,
    auto_link_announce_extension, auto_link_extension, auto_link_url_matcher,
    create_auto_link_node,
};
use crate::rich_text::rich_text_extension;
use crate::{
    AnyLexicalExtensionArgument, LexicalExtension, NodeRef, RootElement, RootElementRef,
    UpdateOptions, config_extension, create_paragraph_node, create_text_node, define_extension,
    get_root,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// A root element whose document body hosts the live region.
#[derive(Default)]
struct LiveRegionHostRoot {
    regions: RefCell<Vec<Rc<LiveRegion>>>,
    writes: Cell<usize>,
    cleared_at: Cell<Option<usize>>,
}

impl LiveRegionHost for LiveRegionHostRoot {
    fn mount_live_region(&self, region: &Rc<LiveRegion>) {
        self.regions.borrow_mut().push(region.clone());
    }
    fn unmount_live_region(&self, region: &Rc<LiveRegion>) {
        self.regions.borrow_mut().retain(|r| !Rc::ptr_eq(r, region));
    }
    fn live_region_did_update(&self, _region: &Rc<LiveRegion>) {
        self.writes.set(self.writes.get() + 1);
    }
}

impl RootElement for LiveRegionHostRoot {
    fn live_region_host(self: Rc<Self>) -> Option<Rc<dyn LiveRegionHost>> {
        Some(self)
    }
}

/// `readLiveRegion()`: the region's text, without the zero-width space a
/// repeat announcement gets.
fn read_live_region(root: &LiveRegionHostRoot) -> String {
    let regions = root.regions.borrow();
    let region = regions.first().expect("a live region");
    if root.cleared_at.get() == Some(root.writes.get()) {
        return String::new();
    }
    region.text_content().replace('\u{200B}', "")
}

/// `clearLiveRegion()`.
fn clear_live_region(root: &LiveRegionHostRoot) {
    root.cleared_at.set(Some(root.writes.get()));
}

fn mount_root(editor: &LexicalEditorWithDispose) -> Rc<LiveRegionHostRoot> {
    let root = Rc::new(LiveRegionHostRoot::default());
    editor
        .set_root_element(Some(RootElementRef(root.clone())))
        .unwrap();
    root
}

/// `withMatchers`: `AutoLinkExtension` with `autoLinkUrlMatcher`.
fn with_matchers() -> AnyLexicalExtensionArgument {
    config_extension(
        &auto_link_extension(),
        [AutoLinkConfigOverrides {
            matchers: Some(vec![Rc::new(auto_link_url_matcher)]),
            ..AutoLinkConfigOverrides::default()
        }],
    )
}

fn build(dependencies: Vec<AnyLexicalExtensionArgument>) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies,
        ..LexicalExtension::<(), (), ()>::new("[root]", ())
    })
    .into()])
    .unwrap()
}

fn build_editor() -> (LexicalEditorWithDispose, Rc<LiveRegionHostRoot>) {
    let editor = build(vec![rich_text_extension().into(), with_matchers()]);
    let root = mount_root(&editor);
    (editor, root)
}

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

fn auto_link_paragraph(url: &str) -> Result<NodeRef, crate::LexicalError> {
    let paragraph = create_paragraph_node();
    let link = create_auto_link_node(url, &AutoLinkAttributes::default());
    link.append(&[create_text_node(url)])?;
    paragraph.append(&[link])?;
    Ok(paragraph)
}

/// `addAutoLink(editor, url)`.
fn add_auto_link(editor: &LexicalEditorWithDispose, url: &str) {
    let url = url.to_owned();
    editor
        .update(
            move || get_root().append(&[auto_link_paragraph(&url)?]),
            discrete(),
        )
        .unwrap();
}

/// `$onlyLink()`.
fn only_link() -> NodeRef {
    let block = get_root().get_last_child().unwrap();
    assert!(block.is_element());
    let link = block.get_first_child().unwrap();
    assert!(link.is_element());
    link
}

// upstream: lexical-link/src/__tests__/unit/AutoLinkAnnounceExtension.test.ts::AutoLinkAnnounceExtension > announces a typed address becoming a link
#[test]
fn announces_a_typed_address_becoming_a_link() {
    let (editor, root) = build_editor();
    add_auto_link(&editor, "https://example.com");
    assert_eq!(read_live_region(&root), "Link");
}

// upstream: lexical-link/src/__tests__/unit/AutoLinkAnnounceExtension.test.ts::AutoLinkAnnounceExtension > announces a link being removed
#[test]
fn announces_a_link_being_removed() {
    let (editor, root) = build_editor();
    add_auto_link(&editor, "https://example.com");
    clear_live_region(&root);
    editor
        .update(|| only_link().remove(false), discrete())
        .unwrap();
    assert_eq!(read_live_region(&root), "Link removed");
}

// upstream: lexical-link/src/__tests__/unit/AutoLinkAnnounceExtension.test.ts::AutoLinkAnnounceExtension > says how many links went when a selection takes several
#[test]
fn says_how_many_links_went_when_a_selection_takes_several() {
    let (editor, root) = build_editor();
    for url in [
        "https://a.example",
        "https://b.example",
        "https://c.example",
    ] {
        add_auto_link(&editor, url);
    }
    clear_live_region(&root);
    editor
        .update(
            || {
                for paragraph in get_root().get_children() {
                    paragraph.remove(false)?;
                }
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    assert_eq!(read_live_region(&root), "3 links removed");
}

// upstream: lexical-link/src/__tests__/unit/AutoLinkAnnounceExtension.test.ts::AutoLinkAnnounceExtension > says how many links arrived when several appear at once
#[test]
fn says_how_many_links_arrived_when_several_appear_at_once() {
    let (editor, root) = build_editor();
    clear_live_region(&root);
    editor
        .update(
            || {
                for url in ["https://a.example", "https://b.example"] {
                    get_root().append(&[auto_link_paragraph(url)?])?;
                }
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    assert_eq!(read_live_region(&root), "2 links");
}

// upstream: lexical-link/src/__tests__/unit/AutoLinkAnnounceExtension.test.ts::AutoLinkAnnounceExtension > stays silent when a link is rebuilt in place
#[test]
fn stays_silent_when_a_link_is_rebuilt_in_place() {
    let (editor, root) = build_editor();
    add_auto_link(&editor, "https://example.com");
    clear_live_region(&root);
    editor
        .update(
            || {
                let replacement = create_auto_link_node(
                    "https://example.com/deeper",
                    &AutoLinkAttributes::default(),
                );
                replacement.append(&[create_text_node("https://example.com/deeper")])?;
                only_link().replace(&replacement, false).map(|_| ())
            },
            discrete(),
        )
        .unwrap();
    assert_eq!(read_live_region(&root), "");
}

// upstream: lexical-link/src/__tests__/unit/AutoLinkAnnounceExtension.test.ts::AutoLinkAnnounceExtension > stays silent while typing inside a link
#[test]
fn stays_silent_while_typing_inside_a_link() {
    let (editor, root) = build_editor();
    add_auto_link(&editor, "https://example.com");
    clear_live_region(&root);
    editor
        .update(
            || only_link().append(&[create_text_node("/more")]),
            discrete(),
        )
        .unwrap();
    assert_eq!(read_live_region(&root), "");
}

// upstream: lexical-link/src/__tests__/unit/AutoLinkAnnounceExtension.test.ts::AutoLinkAnnounceExtension > honours a configured message
#[test]
fn honours_a_configured_message() {
    let editor = build(vec![
        rich_text_extension().into(),
        with_matchers(),
        config_extension(
            &auto_link_announce_extension(),
            [AutoLinkAnnounceExtensionConfigOverrides {
                created: Some("Linked".to_owned()),
                ..AutoLinkAnnounceExtensionConfigOverrides::default()
            }],
        ),
    ]);
    let root = mount_root(&editor);
    add_auto_link(&editor, "https://example.com");
    assert_eq!(read_live_region(&root), "Linked");
}

// upstream: lexical-link/src/__tests__/unit/AutoLinkAnnounceExtension.test.ts::AutoLinkAnnounceExtension > says nothing when disabled
#[test]
fn says_nothing_when_disabled() {
    let (editor, root) = build_editor();
    let disabled = get_extension_dependency_from_editor(&editor, &auto_link_announce_extension())
        .unwrap()
        .output
        .expect("an output")
        .disabled
        .clone();
    disabled.set_value(true);
    add_auto_link(&editor, "https://example.com");
    assert_eq!(read_live_region(&root), "");
    disabled.set_value(false);
    add_auto_link(&editor, "https://second.example");
    assert_eq!(read_live_region(&root), "Link");
}

// upstream: lexical-link/src/__tests__/unit/AutoLinkAnnounceExtension.test.ts::AutoLinkAnnounceExtension > leaves an editor without automatic links alone
#[test]
fn leaves_an_editor_without_automatic_links_alone() {
    let editor = build(vec![
        aria_live_region_extension().into(),
        rich_text_extension().into(),
    ]);
    let root = mount_root(&editor);
    editor
        .update(|| get_root().append(&[create_paragraph_node()]), discrete())
        .unwrap();
    assert_eq!(read_live_region(&root), "");
}
