//! Ports of `packages/lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx`
//! against [`register_auto_embed`], the plugin's logic without React. JS
//! renders the menu with a `menuRenderFn` and clicks its button; here the
//! node menu's options stand for what it draws (`getMenu()` is an open menu
//! with options), and choosing one with `select_option_and_clean_up` is the
//! click. A `parseUrl` that returns a promise is a config whose
//! `parse_url_async` keeps its `ParseUrlDone` (`deferParseUrl()`), and
//! resolving it is the promise resolving.

#![cfg(all(
    feature = "auto-embed",
    feature = "autolink",
    feature = "extension",
    feature = "rich-text",
    feature = "html"
))]

use super::super::support::set_test_root_element;
use crate::auto_embed::{
    AutoEmbedConfig, AutoEmbedOption, DisableMenuFor, EmbedConfig, EmbedMatchResult, ParseUrlDone,
    register_auto_embed,
};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::link::{AutoLinkConfigOverrides, auto_link_extension, create_link_matcher_with_reg_exp};
use crate::menu::NodeMenu;
use crate::{
    COMMAND_PRIORITY_LOW, ClipboardEvent, DataTransfer, Editor, LexicalError, LexicalExtension,
    NodeRef, PASTE_COMMAND, PASTE_TAG, PasteCommandType, Unregister, UpdateOptions,
    config_extension, create_paragraph_node, create_text_node, define_extension, get_node_by_key,
    get_root, get_selection, set_selection,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const YOUTUBE_URL: &str = "https://www.youtube.com/watch?v=jNQXAC9IVRw";
const SHORT_YOUTUBE_URL: &str = "youtu.be/jNQXAC9IVRw";
/// `URL_MATCHER`'s source.
const URL_MATCHER: &str = r"((https?://(www\.)?)|(www\.))[-a-zA-Z0-9@:%._+~#=]{1,256}\.[a-zA-Z0-9()]{1,6}\b([-a-zA-Z0-9()@:%_+.~#?&//=]*)";

/// The `embedConfig` whose `parseUrl` and `insertNode` are mocks.
#[derive(Default)]
struct TestEmbedConfig {
    parsed: RefCell<Vec<String>>,
    inserted: RefCell<Vec<(String, String)>>,
    /// `parseUrl.mockReturnValueOnce(new Promise(...))`: the next parse
    /// waits.
    defer_next: Cell<bool>,
    /// The parses that wait, to resolve.
    deferred: RefCell<Vec<ParseUrlDone>>,
}

impl EmbedConfig for TestEmbedConfig {
    fn embed_type(&self) -> &str {
        "youtube-video"
    }

    fn parse_url_async(&self, url: &str, done: ParseUrlDone) {
        self.parsed.borrow_mut().push(url.to_owned());
        if self.defer_next.replace(false) {
            self.deferred.borrow_mut().push(done);
        } else {
            done.resolve(self.parse_url(url));
        }
    }

    /// `/(?:youtube\.com\/watch\?v=|youtu\.be\/)([\w-]+)/.exec(url)`.
    fn parse_url(&self, url: &str) -> Option<EmbedMatchResult> {
        let re = regex::Regex::new(r"(?:youtube\.com/watch\?v=|youtu\.be/)([\w-]+)").unwrap();
        let id = re.captures(url)?.get(1)?.as_str().to_owned();
        Some(EmbedMatchResult {
            url: url.to_owned(),
            id,
            data: None,
        })
    }

    fn insert_node(&self, _editor: &Editor, result: &EmbedMatchResult) -> Result<(), LexicalError> {
        self.inserted
            .borrow_mut()
            .push((result.id.clone(), result.url.clone()));
        Ok(())
    }
}

struct Fixture {
    editor: LexicalEditorWithDispose,
    config: Rc<TestEmbedConfig>,
    menu: NodeMenu<AutoEmbedOption>,
    unregister: Option<Unregister>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(unregister) = self.unregister.take() {
            unregister();
        }
        self.editor.dispose();
    }
}

impl Fixture {
    fn new(disable_menu_for: Option<DisableMenuFor>) -> Self {
        let editor = build_editor_from_extensions([define_extension(LexicalExtension {
            dependencies: vec![
                crate::rich_text::rich_text_extension().into(),
                crate::link::link_extension().into(),
                config_extension(
                    &auto_link_extension(),
                    [AutoLinkConfigOverrides {
                        matchers: Some(vec![
                            create_link_matcher_with_reg_exp(
                                regex::Regex::new(URL_MATCHER).unwrap(),
                                None,
                            ),
                            // A scheme-less matcher, broader than URL_MATCHER,
                            // as an app might add.
                            create_link_matcher_with_reg_exp(
                                regex::Regex::new(r"youtu\.be/[\w-]+").unwrap(),
                                None,
                            ),
                        ]),
                        ..Default::default()
                    }],
                ),
            ],
            ..LexicalExtension::<(), (), ()>::new("test-auto-embed", ())
        })
        .into()])
        .unwrap();
        // Mounted, so commits reconcile and report mutations.
        set_test_root_element(&editor);
        let config = Rc::new(TestEmbedConfig::default());
        let (menu, unregister) = register_auto_embed(
            &editor,
            AutoEmbedConfig {
                embed_configs: vec![config.clone()],
                get_menu_options: Rc::new(|_, embed, _| {
                    vec![Rc::new(AutoEmbedOption {
                        title: "Embed".to_owned(),
                        on_select: Rc::new(move |_| {
                            embed();
                            Ok(())
                        }),
                    })]
                }),
                on_open_embed_modal_for_config: None,
                disable_menu_for,
                menu_command_priority: COMMAND_PRIORITY_LOW,
            },
        )
        .unwrap();
        Fixture {
            editor,
            config,
            menu,
            unregister: Some(unregister),
        }
    }

    /// `getMenu() !== null`: the menu is open, with options.
    fn is_menu_shown(&self) -> bool {
        self.menu.menu().resolution().is_some() && !self.menu.menu().options().is_empty()
    }

    fn paste_with(
        &self,
        data: &[(&str, &str)],
        select: impl FnOnce() -> Result<(), LexicalError> + 'static,
    ) {
        let clipboard_data = DataTransfer::new();
        for (format, value) in data {
            clipboard_data.set_data(format, value);
        }
        let event = Rc::new(ClipboardEvent::new("paste", Some(Rc::new(clipboard_data))));
        let editor = Editor::clone(&self.editor);
        self.editor
            .update(
                move || {
                    select()?;
                    editor.dispatch_command(&PASTE_COMMAND, PasteCommandType::Clipboard(event))?;
                    Ok(())
                },
                UpdateOptions {
                    discrete: true,
                    ..UpdateOptions::default()
                },
            )
            .unwrap();
        crate::updates::commit_pending_updates(&self.editor).unwrap();
    }

    fn paste(&self, data: &[(&str, &str)]) {
        self.paste_with(data, || get_root().select_end().map(|_| ()));
    }

    /// Clicks the menu's embed button.
    fn select_embed_option(&self) {
        let option = self.menu.menu().options()[0].clone();
        self.menu
            .menu()
            .select_option_and_clean_up(&option)
            .unwrap();
        crate::updates::commit_pending_updates(&self.editor).unwrap();
    }

    /// `editor.update(f)`, left pending.
    fn pending_update(&self, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
        self.editor.update(f, UpdateOptions::default()).unwrap();
    }

    fn text(&self) -> String {
        self.editor.read(|| get_root().get_text_content()).unwrap()
    }

    /// `deferParseUrl()`: the next parse waits until [`Self::resolve`].
    fn defer_parse_url(&self) {
        self.config.defer_next.set(true);
    }

    /// Resolves the oldest parse that waits.
    fn resolve(&self, id: &str, url: &str) {
        let done = self.config.deferred.borrow_mut().remove(0);
        done.resolve(Some(EmbedMatchResult {
            url: url.to_owned(),
            id: id.to_owned(),
            data: None,
        }));
    }

    fn parsed(&self) -> Vec<String> {
        self.config.parsed.borrow().clone()
    }

    fn inserted(&self) -> Vec<(String, String)> {
        self.config.inserted.borrow().clone()
    }
}

fn plain(text: &str) -> [(&str, &str); 1] {
    [("text/plain", text)]
}

/// `$getParagraph()`.
fn paragraph() -> NodeRef {
    let paragraph = get_root().get_first_child().unwrap();
    assert_eq!(paragraph.get_type(), "paragraph");
    paragraph
}

/// `$getPastedLink()`.
fn pasted_link() -> NodeRef {
    let link = paragraph().get_first_child().unwrap();
    assert!(crate::link::is_link_node(&link), "{}", link.get_type());
    link
}

fn youtube() -> (String, String) {
    ("jNQXAC9IVRw".to_owned(), YOUTUBE_URL.to_owned())
}

// upstream: lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > embeds an unchanged link without parsing its URL again
#[test]
fn embeds_an_unchanged_link_without_parsing_its_url_again() {
    let f = Fixture::new(None);
    f.paste(&plain(YOUTUBE_URL));
    f.select_embed_option();
    assert_eq!(f.parsed().len(), 1);
    assert_eq!(f.inserted(), [youtube()]);
    assert_eq!(f.text(), "");
}

// upstream: lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > embeds an unchanged link with no selection
#[test]
fn embeds_an_unchanged_link_with_no_selection() {
    let f = Fixture::new(None);
    f.paste(&plain(YOUTUBE_URL));
    f.pending_update(|| {
        set_selection(None);
        Ok(())
    });
    crate::updates::commit_pending_updates(&f.editor).unwrap();
    f.select_embed_option();
    assert_eq!(f.parsed().len(), 1);
    assert_eq!(f.inserted(), [youtube()]);
    assert_eq!(f.text(), "");
    assert!(f.editor.read(|| get_selection().is_some()).unwrap());
}

// upstream: lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > does not embed a link removed by a pending update
#[test]
fn does_not_embed_a_link_removed_by_a_pending_update() {
    let f = Fixture::new(None);
    f.paste(&plain(YOUTUBE_URL));
    let option = f.menu.menu().options()[0].clone();
    f.pending_update(|| {
        pasted_link().remove(false)?;
        paragraph().select_end().map(|_| ())
    });
    f.menu.menu().select_option_and_clean_up(&option).unwrap();
    crate::updates::commit_pending_updates(&f.editor).unwrap();
    assert!(f.inserted().is_empty());
    assert_eq!(f.text(), "");
}

// upstream: lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > does not embed a link whose URL changed in a pending update
#[test]
fn does_not_embed_a_link_whose_url_changed_in_a_pending_update() {
    let f = Fixture::new(None);
    f.paste(&plain(YOUTUBE_URL));
    let option = f.menu.menu().options()[0].clone();
    f.pending_update(|| {
        pasted_link().set_url("https://example.com/");
        Ok(())
    });
    f.menu.menu().select_option_and_clean_up(&option).unwrap();
    crate::updates::commit_pending_updates(&f.editor).unwrap();
    assert!(f.inserted().is_empty());
    assert_eq!(
        f.editor
            .read(|| pasted_link().get_url())
            .unwrap()
            .as_deref(),
        Some("https://example.com/")
    );
}

// upstream: lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > does not let an older paste that resolves late replace the menu
#[test]
fn does_not_let_an_older_paste_that_resolves_late_replace_the_menu() {
    const OTHER_URL: &str = "https://www.youtube.com/watch?v=OTHERvid123";
    let f = Fixture::new(None);
    f.defer_parse_url();
    f.paste(&plain(YOUTUBE_URL));
    assert!(!f.is_menu_shown());
    f.paste_with(&plain(OTHER_URL), || {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.select(None, None).map(|_| ())
    });
    assert!(f.is_menu_shown());
    f.resolve("jNQXAC9IVRw", YOUTUBE_URL);
    f.select_embed_option();
    assert_eq!(
        f.inserted(),
        [("OTHERvid123".to_owned(), OTHER_URL.to_owned())]
    );
}

// upstream: lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > does not offer to embed a link whose URL changed while parsing
#[test]
fn does_not_offer_to_embed_a_link_whose_url_changed_while_parsing() {
    let f = Fixture::new(None);
    f.defer_parse_url();
    f.paste(&plain(YOUTUBE_URL));
    assert!(!f.is_menu_shown());
    f.pending_update(|| {
        pasted_link().set_url("https://example.com/");
        Ok(())
    });
    crate::updates::commit_pending_updates(&f.editor).unwrap();
    f.resolve("jNQXAC9IVRw", YOUTUBE_URL);
    assert!(!f.is_menu_shown());
}

// upstream: lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > does not offer to embed a link with a pending URL change when parsing resolves
#[test]
fn does_not_offer_to_embed_a_link_with_a_pending_url_change_when_parsing_resolves() {
    let f = Fixture::new(None);
    f.defer_parse_url();
    f.paste(&plain(YOUTUBE_URL));
    assert!(!f.is_menu_shown());
    // Resolved while the URL change is pending, before it commits.
    f.pending_update(|| {
        pasted_link().set_url("https://example.com/");
        Ok(())
    });
    f.resolve("jNQXAC9IVRw", YOUTUBE_URL);
    // The check reads the pending state, so the menu doesn't open even
    // before the change commits.
    assert!(!f.is_menu_shown());
    crate::updates::commit_pending_updates(&f.editor).unwrap();
    assert!(!f.is_menu_shown());
}

// upstream: lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > offers to embed a bare URL pasted as plain text
#[test]
fn offers_to_embed_a_bare_url_pasted_as_plain_text() {
    let f = Fixture::new(None);
    f.paste(&plain(YOUTUBE_URL));
    assert_eq!(f.parsed(), [YOUTUBE_URL]);
    assert!(f.is_menu_shown());
}

// upstream: lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > does not offer to embed a pasted sentence that contains a URL
#[test]
fn does_not_offer_to_embed_a_pasted_sentence_that_contains_a_url() {
    let f = Fixture::new(None);
    let html = format!(r#"<p>Look at this <a href="{YOUTUBE_URL}">{YOUTUBE_URL}</a> please</p>"#);
    let text = format!("Look at this {YOUTUBE_URL} please");
    f.paste(&[("text/html", &html), ("text/plain", &text)]);
    assert!(f.parsed().is_empty());
    assert!(!f.is_menu_shown());
}

// upstream: lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > does not offer to embed a pasted plain text sentence that contains a URL
#[test]
fn does_not_offer_to_embed_a_pasted_plain_text_sentence_that_contains_a_url() {
    let f = Fixture::new(None);
    f.paste(&plain(&format!("Look at this {YOUTUBE_URL} please")));
    assert!(f.parsed().is_empty());
    assert!(!f.is_menu_shown());
}

// upstream: lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > offers to embed a copied link whose text is its URL
#[test]
fn offers_to_embed_a_copied_link_whose_text_is_its_url() {
    let f = Fixture::new(None);
    let html = format!(r#"<a href="{YOUTUBE_URL}">{YOUTUBE_URL}</a>"#);
    f.paste(&[("text/html", &html), ("text/plain", YOUTUBE_URL)]);
    assert_eq!(f.parsed(), [YOUTUBE_URL]);
    assert!(f.is_menu_shown());
}

// upstream: lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > does not offer to embed a copied link with a label
#[test]
fn does_not_offer_to_embed_a_copied_link_with_a_label() {
    let f = Fixture::new(None);
    let html = format!(r#"<a href="{YOUTUBE_URL}">first video</a>"#);
    f.paste(&[("text/html", &html), ("text/plain", "first video")]);
    assert!(f.parsed().is_empty());
    assert!(!f.is_menu_shown());
}

// upstream: lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > offers to embed a bare address that only a custom matcher links
#[test]
fn offers_to_embed_a_bare_address_that_only_a_custom_matcher_links() {
    let f = Fixture::new(None);
    f.paste(&plain(SHORT_YOUTUBE_URL));
    assert_eq!(f.parsed(), [SHORT_YOUTUBE_URL]);
    assert!(f.is_menu_shown());
}

// upstream: lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > still offers to embed a bare URL pasted after typed text
#[test]
fn still_offers_to_embed_a_bare_url_pasted_after_typed_text() {
    let f = Fixture::new(None);
    f.pending_update(|| {
        let p = create_paragraph_node();
        p.append(&[create_text_node("Check this out: ")])?;
        let root = get_root();
        root.clear()?;
        root.append(&[p])
    });
    crate::updates::commit_pending_updates(&f.editor).unwrap();
    f.paste(&plain(YOUTUBE_URL));
    assert_eq!(f.parsed(), [YOUTUBE_URL]);
    assert!(f.is_menu_shown());
    assert_eq!(f.text(), format!("Check this out: {YOUTUBE_URL}"));
}

// upstream: lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > offers to embed a bare URL pasted into formatted text
#[test]
fn offers_to_embed_a_bare_url_pasted_into_formatted_text() {
    let f = Fixture::new(None);
    let middle_key = Rc::new(RefCell::new(String::new()));
    let key = middle_key.clone();
    f.pending_update(move || {
        let middle = create_text_node(" and ");
        *key.borrow_mut() = middle.get_key().to_owned();
        let this = create_text_node("this");
        this.toggle_format("bold");
        let that = create_text_node("that");
        that.toggle_format("italic");
        let p = create_paragraph_node();
        p.append(&[
            create_text_node("Read "),
            this,
            middle,
            that,
            create_text_node(" now"),
        ])?;
        let root = get_root();
        root.clear()?;
        root.append(&[p])
    });
    crate::updates::commit_pending_updates(&f.editor).unwrap();
    let paste_dirty_leaves = Rc::new(Cell::new(0));
    let dirty = paste_dirty_leaves.clone();
    let remove_update_listener = f.editor.register_update_listener(move |payload| {
        if payload.tags.iter().any(|t| t == PASTE_TAG) {
            dirty.set(payload.dirty_leaves.len());
        }
    });
    // Paste over the word between the bold and italic runs.
    let key = middle_key.borrow().clone();
    f.paste_with(&plain(YOUTUBE_URL), move || {
        let middle = get_node_by_key(&key).unwrap();
        assert!(middle.is_text());
        middle.select(Some(1), Some(4)).map(|_| ())
    });
    remove_update_listener();
    // The formatted siblings and the split text nodes are all dirty, so a
    // dirty leaf count cannot tell this paste apart from pasted prose.
    assert!(paste_dirty_leaves.get() > 3, "{}", paste_dirty_leaves.get());
    assert_eq!(f.parsed(), [YOUTUBE_URL]);
    assert!(f.is_menu_shown());
    assert_eq!(f.text(), format!("Read this {YOUTUBE_URL} that now"));
}

// upstream: lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > does not throw when the clipboard reports no plain text
#[test]
fn does_not_throw_when_the_clipboard_reports_no_plain_text() {
    let f = Fixture::new(None);
    f.paste(&[("text/html", "replaced")]);
    assert!(f.parsed().is_empty());
    assert!(!f.is_menu_shown());
    assert_eq!(f.text(), "replaced");
}

/// `$getNearestNodeOfType(node, QuoteNode) !== null`.
fn in_quote() -> DisableMenuFor {
    Rc::new(|node: &NodeRef| {
        let mut current = Some(node.clone());
        while let Some(n) = current {
            if crate::rich_text::is_quote_node(&n) {
                return true;
            }
            current = n.get_parent();
        }
        false
    })
}

// upstream: lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > does not show the menu when a $disableMenuFor predicate matches the link location
#[test]
fn does_not_show_the_menu_when_a_disable_menu_for_predicate_matches() {
    let f = Fixture::new(Some(in_quote()));
    f.editor
        .update(
            || {
                let quote = crate::rich_text::create_quote_node(false);
                quote.append(&[create_paragraph_node()])?;
                get_root().append(&[quote])
            },
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
    f.paste_with(&plain(YOUTUBE_URL), || {
        let quote = get_root().get_last_child().unwrap();
        assert!(crate::rich_text::is_quote_node(&quote));
        quote.get_first_child().unwrap().select_end().map(|_| ())
    });
    // The menu never opens, so the URL is never even parsed and the pasted
    // link is left untouched.
    assert!(f.parsed().is_empty());
    assert!(!f.is_menu_shown());
    f.editor
        .read(|| {
            let quote = get_root().get_last_child().unwrap();
            assert!(crate::rich_text::is_quote_node(&quote));
            let paragraph = quote.get_first_child().unwrap();
            assert_eq!(paragraph.get_type(), "paragraph");
            assert!(crate::link::is_link_node(
                &paragraph.get_first_child().unwrap()
            ));
        })
        .unwrap();
}

// upstream: lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > shows the menu when no $disableMenuFor predicate matches
#[test]
fn shows_the_menu_when_no_disable_menu_for_predicate_matches() {
    let f = Fixture::new(Some(in_quote()));
    // The default paste target is the root paragraph, outside any quote.
    f.paste(&plain(YOUTUBE_URL));
    assert!(f.is_menu_shown());
    f.select_embed_option();
    assert_eq!(f.inserted(), [youtube()]);
}
