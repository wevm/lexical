//! `ClickableLinkExtension.ts`: `registerClickableLink` and
//! `ClickableLinkExtension` (with the `extension` feature).
//!
//! JS listens to `click` and `auxclick` on the root element and opens the
//! link with `window.open`. Here the app reports a click as
//! [`CLICK_COMMAND`], with the node under the pointer as its
//! [`target_key`](crate::MouseEvent::target_key) and an `auxclick`'s type,
//! and a [`LinkWindow`] stands in for the window, as in Swift. JS also
//! opens a plain `<a>` that isn't a `LinkNode` (`findMatchingDOM`); a
//! model has no such element, so that branch isn't ported.

use super::link_extension::link_extension;
use super::link_node::sanitize::sanitize_url;
use super::link_node::{is_auto_link_node, is_link_node};
use crate::extension::named_signals_config;
use crate::extension_core::{
    ExtensionBuildState, ExtensionRegisterState, LexicalExtension, define_extension,
};
use crate::selection_insert_nodes::find_matching_parent;
use crate::{
    CLICK_COMMAND, COMMAND_PRIORITY_LOW, Editor, NodeRef, Unregister, get_node_by_key,
    get_selection,
};
use std::rc::Rc;

/// What [`register_clickable_link`] opens a link with: JS's `Window`.
pub trait LinkWindow {
    /// `window.open(url, target, features)`: `target` is `_blank` with the
    /// `features` `noopener` for a new tab, else `_self` without features.
    fn open(&self, url: &str, target: &str, features: Option<&str>);
}

/// The function of a [`GetLinkWindow`].
pub type GetLinkWindowFn = dyn Fn(&Editor) -> Option<Rc<dyn LinkWindow>>;

/// `getWindow`: the window to open links in, for an editor, or `None` to
/// leave the click alone; compared by identity, as JS compares functions.
#[derive(Clone)]
pub struct GetLinkWindow(pub Rc<GetLinkWindowFn>);

impl PartialEq for GetLinkWindow {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Default for GetLinkWindow {
    /// No window: JS's default is the global `window`, which a native app
    /// doesn't have, so it supplies one.
    fn default() -> Self {
        GetLinkWindow(Rc::new(|_| None))
    }
}

named_signals_config! {
    /// `ClickableLinkConfig`.
    pub struct ClickableLinkConfig;
    /// `Partial<ClickableLinkConfig>`.
    pub struct ClickableLinkConfigOverrides;
    /// `namedSignals(ClickableLinkConfig)`: the extension's output.
    pub struct ClickableLinkOutput {
        /// `newTab`: open every clicked link in a new tab.
        new_tab: bool,
        /// `disabled`.
        disabled: bool,
        /// `getWindow`: none by default (see [`GetLinkWindow`]).
        get_window: GetLinkWindow,
    }
}

/// `registerClickableLink(editor, stores)`: on [`CLICK_COMMAND`] on a link
/// (or inside one), opens its sanitized URL, unless it is an unlinked
/// autolink, the URL is empty, or the click ends a selection that isn't
/// collapsed (which it lets select the link's text). A middle click (an
/// `auxclick` of button 1), a click with Command or Control, a `_blank`
/// target or `newTab` opens it in a new tab; an `auxclick` of any other
/// button is left alone. The handler never handles the command, so the
/// other click handlers still run, at [`COMMAND_PRIORITY_LOW`] as in Swift;
/// it prevents the event's default once it opens the link (or keeps a
/// selection). The signals are read when a click arrives.
pub fn register_clickable_link(editor: &Editor, stores: &ClickableLinkOutput) -> Unregister {
    let ClickableLinkOutput {
        new_tab,
        disabled,
        get_window,
    } = stores.clone();
    editor.register_command(
        &CLICK_COMMAND,
        move |event, editor| {
            // `click` only fires for the primary button; every other button
            // arrives as `auxclick`, and only the middle one follows the link.
            let is_middle = event.button == 1;
            if event.type_ == "auxclick" && !is_middle {
                return Ok(false);
            }
            let Some(clicked_node) = event.target_key.as_deref().and_then(get_node_by_key) else {
                return Ok(false);
            };
            if disabled.peek() {
                return Ok(false);
            }
            let Some(link) =
                find_matching_parent(&clicked_node, NodeRef::is_element).filter(is_link_node)
            else {
                return Ok(false);
            };
            if is_auto_link_node(&link) && link.get_is_unlinked() == Some(true) {
                return Ok(false);
            }
            let url = sanitize_url(&link.get_url().unwrap_or_default());
            if url.is_empty() {
                return Ok(false);
            }
            // Allow the user to select link text without following the URL.
            if let Some(range) = get_selection().and_then(|s| s.range())
                && !range.is_collapsed()
            {
                event.prevent_default();
                return Ok(false);
            }
            let new_tab = new_tab.peek()
                || is_middle
                || event.meta_key
                || event.ctrl_key
                || link.get_target().as_deref() == Some("_blank");
            let Some(target_window) = (get_window.peek().0)(editor) else {
                return Ok(false);
            };
            if new_tab {
                target_window.open(&url, "_blank", Some("noopener"));
            } else {
                target_window.open(&url, "_self", None);
            }
            event.prevent_default();
            Ok(false)
        },
        COMMAND_PRIORITY_LOW,
    )
}

/// `ClickableLinkExtension`.
pub type ClickableLinkExtensionType =
    LexicalExtension<ClickableLinkConfig, ClickableLinkOutput, ()>;

fn make() -> Rc<ClickableLinkExtensionType> {
    define_extension(LexicalExtension {
        dependencies: vec![link_extension().into()],
        build: Some(Rc::new(
            |_: &Editor, config: &ClickableLinkConfig, _: &ExtensionBuildState<()>| {
                Ok(ClickableLinkOutput::of(config))
            },
        )),
        register: Some(Rc::new(
            |editor: &Editor,
             _: &ClickableLinkConfig,
             state: &ExtensionRegisterState<(), ClickableLinkOutput>| {
                let stores = state
                    .get_output()
                    .expect("ClickableLinkExtension has a build");
                Ok(register_clickable_link(editor, &stores))
            },
        )),
        ..LexicalExtension::new(
            "@lexical/link/ClickableLink",
            ClickableLinkConfig {
                new_tab: false,
                disabled: false,
                get_window: GetLinkWindow::default(),
            },
        )
    })
}

thread_local! {
    static CLICKABLE_LINK_EXTENSION: Rc<ClickableLinkExtensionType> = make();
}

/// `ClickableLinkExtension`: opens links when they are clicked (see
/// [`register_clickable_link`]), which a click on a `LinkNode` otherwise
/// only selects; for an editor that isn't editable, say. It depends on
/// `LinkExtension`.
pub fn clickable_link_extension() -> Rc<ClickableLinkExtensionType> {
    CLICKABLE_LINK_EXTENSION.with(Rc::clone)
}
