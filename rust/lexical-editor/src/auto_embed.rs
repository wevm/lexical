//! `@lexical/react`'s auto-embed without React (the `auto-embed` feature):
//! `LexicalAutoEmbedPluginUtils.ts` ([`EmbedMatchResult`], [`EmbedConfig`],
//! [`URL_MATCHER`](url_matcher), [`INSERT_EMBED_COMMAND`],
//! [`AutoEmbedOption`]) and the logic of `LexicalAutoEmbedPlugin.tsx`
//! ([`register_auto_embed`]): a pasted link that an embed config recognizes
//! (a video, a post) opens a node menu at it that offers to replace it with
//! the embed. React keeps the match in component state and renders
//! `LexicalNodeMenuPlugin`; here [`register_auto_embed`] keeps it and
//! drives a [`NodeMenu`], which the app's view draws.
//!
//! `parseUrl` is synchronous: JS awaits what it returns, which may be a
//! promise, so the menu opens after the paste's commit; here it opens in
//! the commit's mutation listener (rust/PARITY.md).

use crate::lexical_selection::get_selection;
use crate::link::is_link_node;
use crate::menu::{MenuOption, MenuSelectOption, NodeMenu};
use crate::merge_register::merge_register;
use crate::updates::on_update;
use crate::{
    COMMAND_PRIORITY_EDITOR, Editor, LexicalCommand, LexicalError, MutationListenerOptions,
    NodeKey, NodeMutation, NodeRef, PASTE_COMMAND, PASTE_TAG, Unregister, UpdateOptions,
    get_node_by_key,
};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// `EmbedMatchResult`: the matched `url`, the `id` of the embedded
/// resource, and optional provider `data`.
#[derive(Clone)]
pub struct EmbedMatchResult {
    pub url: String,
    pub id: String,
    pub data: Option<Rc<dyn Any>>,
}

/// `EmbedConfig`: a kind of embed, with its `type`, `parseUrl`, which
/// decides whether a URL is one and extracts its data, and `insertNode`,
/// which inserts its node.
pub trait EmbedConfig {
    /// `type`.
    fn embed_type(&self) -> &str;
    /// `parseUrl(text)`.
    fn parse_url(&self, text: &str) -> Option<EmbedMatchResult>;
    /// `insertNode(editor, result)`, in an update.
    fn insert_node(&self, editor: &Editor, result: &EmbedMatchResult) -> Result<(), LexicalError>;
}

/// `INSERT_EMBED_COMMAND`: starts inserting the embed whose config has the
/// payload as its `type`.
pub const INSERT_EMBED_COMMAND: LexicalCommand<String> =
    LexicalCommand::constant(u64::MAX - 30, "INSERT_EMBED_COMMAND");

/// `AutoEmbedOption`: an item of the embed menu, its key its title, with
/// what choosing it does.
pub struct AutoEmbedOption {
    pub title: String,
    /// `onSelect(targetNode)`.
    pub on_select: Rc<dyn Fn(Option<NodeRef>) -> Result<(), LexicalError>>,
}

impl MenuOption for AutoEmbedOption {
    fn key(&self) -> &str {
        &self.title
    }
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }
}

fn is_word(unit: u16) -> bool {
    matches!(unit, 0x30..=0x39 | 0x41..=0x5A | 0x61..=0x7A | 0x5F)
}

/// `[-a-zA-Z0-9@:%._+~#=]`.
fn is_host_unit(unit: u16) -> bool {
    is_word(unit) && unit != 0x5F || b"-@:%._+~#=".iter().any(|&c| u16::from(c) == unit)
}

/// `[a-zA-Z0-9()]`.
fn is_tld_unit(unit: u16) -> bool {
    is_word(unit) && unit != 0x5F || unit == u16::from(b'(') || unit == u16::from(b')')
}

/// `[-a-zA-Z0-9()@:%_+.~#?&//=]`.
fn is_path_unit(unit: u16) -> bool {
    is_word(unit) || b"-()@:%+.~#?&/=".iter().any(|&c| u16::from(c) == unit)
}

/// The match of `URL_MATCHER` at `start`, its end, as `exec` finds it.
fn match_url_at(units: &[u16], start: usize) -> Option<usize> {
    let has = |at: usize, text: &str| {
        text.encode_utf16()
            .enumerate()
            .all(|(i, u)| units.get(at + i) == Some(&u))
    };
    // `((https?:\/\/(www\.)?)|(www\.))`, each alternative's ends in the order
    // the backtracking tries them.
    let mut prefixes = Vec::new();
    if has(start, "http") {
        let scheme = if has(start + 4, "s") {
            start + 5
        } else {
            start + 4
        };
        if has(scheme, "://") {
            let after = scheme + 3;
            if has(after, "www.") {
                prefixes.push(after + 4);
            }
            prefixes.push(after);
        }
    }
    if has(start, "www.") {
        prefixes.push(start + 4);
    }
    prefixes.into_iter().find_map(|prefix| {
        // `[-a-zA-Z0-9@:%._+~#=]{1,256}`, longest first.
        let host_max = units[prefix..]
            .iter()
            .take(256)
            .take_while(|&&u| is_host_unit(u))
            .count();
        (1..=host_max).rev().find_map(|host| {
            let dot = prefix + host;
            if units.get(dot) != Some(&u16::from(b'.')) {
                return None;
            }
            // `\.[a-zA-Z0-9()]{1,6}\b`, longest first.
            let tld_max = units[dot + 1..]
                .iter()
                .take(6)
                .take_while(|&&u| is_tld_unit(u))
                .count();
            (1..=tld_max).rev().find_map(|tld| {
                let end = dot + 1 + tld;
                let before = is_word(units[end - 1]);
                let after = units.get(end).is_some_and(|&u| is_word(u));
                (before != after).then(|| {
                    // `([-a-zA-Z0-9()@:%_+.~#?&//=]*)`.
                    end + units[end..]
                        .iter()
                        .take_while(|&&u| is_path_unit(u))
                        .count()
                })
            })
        })
    })
}

/// `URL_MATCHER.exec(text)`: the UTF-16 index and the text of the first
/// match of
/// `((https?:\/\/(www\.)?)|(www\.))[-a-zA-Z0-9@:%._+~#=]{1,256}\.[a-zA-Z0-9()]{1,6}\b([-a-zA-Z0-9()@:%_+.~#?&//=]*)`,
/// a general-purpose URL expression for a `parseUrl`, matched by hand as JS
/// matches it.
pub fn url_matcher(text: &str) -> Option<(usize, String)> {
    let units: Vec<u16> = text.encode_utf16().collect();
    (0..units.len()).find_map(|start| {
        match_url_at(&units, start).map(|end| (start, String::from_utf16_lossy(&units[start..end])))
    })
}

/// `/^\S+$/.test(text)`: whether a paste is one token, which JS's `\s`
/// (Unicode's white space, less U+0085 and U+180E, and U+FEFF) does not
/// split.
pub fn is_single_token(text: &str) -> bool {
    !text.is_empty()
        && !text
            .chars()
            .any(crate::selection_deletion::is_js_whitespace)
}

/// `getMenuOptions(activeEmbedConfig, embedFn, dismissFn)`.
pub type GetMenuOptions =
    Rc<dyn Fn(&Rc<dyn EmbedConfig>, Rc<dyn Fn()>, Rc<dyn Fn()>) -> Vec<Rc<AutoEmbedOption>>>;

/// `onOpenEmbedModalForConfig(embedConfig)`.
pub type OpenEmbedModalForConfig = Rc<dyn Fn(&Rc<dyn EmbedConfig>)>;

/// `$disableMenuFor(node)`.
pub type DisableMenuFor = Rc<dyn Fn(&NodeRef) -> bool>;

/// `registerAutoEmbed`'s arguments: `LexicalAutoEmbedPlugin`'s props
/// besides the view's.
pub struct AutoEmbedConfig {
    /// `embedConfigs`: when several match a URL, the last wins.
    pub embed_configs: Vec<Rc<dyn EmbedConfig>>,
    /// `getMenuOptions(activeEmbedConfig, embedFn, dismissFn)`: the menu's
    /// options for a match; `embedFn` replaces the link with the embed,
    /// `dismissFn` closes the menu.
    pub get_menu_options: GetMenuOptions,
    /// `onOpenEmbedModalForConfig`: handles `INSERT_EMBED_COMMAND` for a
    /// config's type.
    pub on_open_embed_modal_for_config: Option<OpenEmbedModalForConfig>,
    /// `$disableMenuFor`: no menu for a link where it returns true.
    pub disable_menu_for: Option<DisableMenuFor>,
}

/// `EmbedMatch`.
#[derive(Clone)]
struct EmbedMatch {
    embed_config: Rc<dyn EmbedConfig>,
    key: NodeKey,
    result: EmbedMatchResult,
    url: String,
}

/// The plugin's state: its config, the current match, and the menu.
struct AutoEmbedState {
    editor: Editor,
    config: AutoEmbedConfig,
    menu: NodeMenu<AutoEmbedOption>,
    embed_match: RefCell<Option<EmbedMatch>>,
    is_single_token_paste: Cell<bool>,
    latest_check: Cell<u64>,
}

/// `$getCurrentLink(key, url)`: the link `key`, while it is attached and
/// its URL is still `url`.
fn get_current_link(key: &str, url: &str) -> Option<NodeRef> {
    get_node_by_key(key).filter(|link| {
        is_link_node(link) && link.is_attached() && link.get_url().as_deref() == Some(url)
    })
}

impl AutoEmbedState {
    /// `$isMenuDisabledForNode(linkNode, $disableMenuFor)`.
    fn is_menu_disabled_for_node(&self, link: &NodeRef) -> bool {
        self.config
            .disable_menu_for
            .as_ref()
            .is_some_and(|f| f(link))
    }

    /// `reset()`: `setEmbedMatch(null)`.
    fn reset(self: &Rc<Self>) {
        if self.embed_match.borrow_mut().take().is_some() {
            self.render();
        }
    }

    /// What the component renders for the match: a node menu at its link
    /// with `getMenuOptions`' options, or nothing.
    fn render(self: &Rc<Self>) {
        let embed_match = self.embed_match.borrow().clone();
        match embed_match {
            None => {
                self.menu.set_node_key(None);
                self.menu.menu().set_options(Vec::new());
            }
            Some(embed_match) => {
                let (embed, dismiss) = (Rc::downgrade(self), Rc::downgrade(self));
                let options = (self.config.get_menu_options)(
                    &embed_match.embed_config,
                    Rc::new(move || {
                        if let Some(state) = embed.upgrade() {
                            state.embed_link_via_active_embed_config();
                        }
                    }),
                    Rc::new(move || {
                        if let Some(state) = dismiss.upgrade() {
                            state.reset();
                        }
                    }),
                );
                self.menu.set_node_key(Some(embed_match.key));
                self.menu.menu().set_options(options);
            }
        }
    }

    /// `checkIfLinkNodeIsEmbeddable(key)`.
    fn check_if_link_node_is_embeddable(self: &Rc<Self>, key: &str) {
        let check = self.latest_check.get() + 1;
        self.latest_check.set(check);
        let url = self.editor.get_editor_state().read(|| {
            let link = get_node_by_key(key).filter(is_link_node)?;
            if self.is_menu_disabled_for_node(&link) {
                return None;
            }
            link.get_url()
        });
        let Some(url) = url else { return };
        // The last config that matches wins.
        for embed_config in self.config.embed_configs.iter().rev() {
            let result = embed_config.parse_url(&url);
            if check != self.latest_check.get() {
                return;
            }
            if let Some(result) = result {
                let embeddable = self.editor.get_editor_state().read(|| {
                    get_current_link(key, &url)
                        .is_some_and(|link| !self.is_menu_disabled_for_node(&link))
                });
                if embeddable {
                    *self.embed_match.borrow_mut() = Some(EmbedMatch {
                        embed_config: embed_config.clone(),
                        key: key.to_owned(),
                        result,
                        url,
                    });
                    self.render();
                }
                return;
            }
        }
    }

    /// `embedLinkViaActiveEmbedConfig()`: in an update, the config inserts
    /// its node, then the link is removed (unless it was removed or its URL
    /// changed meanwhile).
    fn embed_link_via_active_embed_config(&self) {
        let Some(EmbedMatch {
            embed_config,
            key,
            result,
            url,
        }) = self.embed_match.borrow().clone()
        else {
            return;
        };
        let editor = self.editor.clone();
        let outcome = self.editor.update(
            move || {
                let Some(link) = get_current_link(&key, &url) else {
                    return Ok(());
                };
                if get_selection().is_none() {
                    link.select_end()?;
                }
                embed_config.insert_node(&editor, &result)?;
                if link.is_attached() {
                    link.remove(false)?;
                }
                Ok(())
            },
            UpdateOptions::default(),
        );
        // JS's `editor.update` reports an error to `onError`, which throws.
        let _ = outcome;
    }
}

/// `LexicalAutoEmbedPlugin`'s logic: watches the links (`LinkNode`,
/// `AutoLinkNode`) that a paste of one token creates, and when one of
/// `embed_configs` parses the URL, opens the returned menu at the link with
/// `get_menu_options`' options; choosing one runs its `on_select` with the
/// link. The match is cleared when the link changes again or the menu
/// closes. `on_open_embed_modal_for_config` handles `INSERT_EMBED_COMMAND`.
/// Returns the menu and what unregisters it; fails when the editor doesn't
/// register the link nodes.
pub fn register_auto_embed(
    editor: &Editor,
    config: AutoEmbedConfig,
) -> Result<(NodeMenu<AutoEmbedOption>, Unregister), LexicalError> {
    let on_select_option: MenuSelectOption<AutoEmbedOption> =
        Rc::new(|option, target_node, close_menu, _| {
            (option.on_select)(target_node)?;
            close_menu();
            Ok(())
        });
    let state = Rc::new(AutoEmbedState {
        editor: editor.clone(),
        config,
        menu: NodeMenu::new(editor, on_select_option),
        embed_match: RefCell::new(None),
        is_single_token_paste: Cell::new(false),
        latest_check: Cell::new(0),
    });
    // `onClose={reset}`.
    let closing = Rc::downgrade(&state);
    *state.menu.menu().callbacks().on_close.borrow_mut() = Some(Rc::new(move || {
        if let Some(state) = closing.upgrade() {
            state.reset();
        }
    }));
    let pasting = Rc::downgrade(&state);
    let mut unregisters = vec![editor.register_command_before(
        &PASTE_COMMAND,
        move |payload, _| {
            let Some(state) = pasting.upgrade() else {
                return Ok(false);
            };
            let is_single_token_paste = payload
                .as_ref()
                .is_some_and(|data| is_single_token(&data.get_data("text/plain")));
            state.is_single_token_paste.set(is_single_token_paste);
            if is_single_token_paste {
                let state = Rc::downgrade(&state);
                on_update(move || {
                    if let Some(state) = state.upgrade() {
                        state.is_single_token_paste.set(false);
                    }
                });
            }
            Ok(false)
        },
        COMMAND_PRIORITY_EDITOR,
    )];
    for node_type in ["link", "autolink"] {
        let listening = Rc::downgrade(&state);
        unregisters.push(editor.register_mutation_listener(
            node_type,
            move |mutations, payload| {
                let Some(state) = listening.upgrade() else {
                    return;
                };
                for (key, mutation) in mutations {
                    let node_key = state.embed_match.borrow().as_ref().map(|m| m.key.clone());
                    if *mutation == NodeMutation::Created
                        && payload.update_tags.iter().any(|t| t == PASTE_TAG)
                        && state.is_single_token_paste.get()
                    {
                        state.check_if_link_node_is_embeddable(key);
                    } else if node_key.as_ref() == Some(key) {
                        state.reset();
                    }
                }
            },
            MutationListenerOptions {
                skip_initialization: true,
            },
        )?);
    }
    if state.config.on_open_embed_modal_for_config.is_some() {
        let opening = Rc::downgrade(&state);
        unregisters.push(editor.register_command(
            &INSERT_EMBED_COMMAND,
            move |embed_config_type, _| {
                let Some(state) = opening.upgrade() else {
                    return Ok(false);
                };
                let Some(embed_config) = state
                    .config
                    .embed_configs
                    .iter()
                    .find(|c| c.embed_type() == embed_config_type)
                else {
                    return Ok(false);
                };
                if let Some(open) = &state.config.on_open_embed_modal_for_config {
                    open(embed_config);
                }
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ));
    }
    let menu = state.menu.clone();
    let unregister = merge_register(unregisters);
    Ok((
        menu,
        Box::new(move || {
            unregister();
            state.reset();
        }),
    ))
}
