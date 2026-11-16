//! `@lexical/a11y`'s announcements (the `a11y` feature), from
//! `packages/lexical-a11y/src/index.ts`: the live region an editor
//! announces through ([`LiveRegion`], shown by the app's
//! [`LiveRegionHost`]), `AriaLiveRegionExtension`, whose output's
//! [`announce`](AriaLiveRegion::announce) the other announcers write to,
//! `HistoryAnnounceExtension` and `EditorModeAnnounceExtension`. With
//! `rich-text`, `@lexical/rich-text`'s `HeadingAnnounceExtension` is
//! [`heading_announce_extension`](crate::rich_text::heading_announce_extension),
//! and with `link`, `@lexical/link`'s `AutoLinkAnnounceExtension` is
//! [`auto_link_announce_extension`](crate::link::auto_link_announce_extension).
//!
//! JS mounts a visually hidden `role="status"` element with `aria-live`
//! and `aria-atomic="true"` in the root element's document body, or in a
//! configured `owner`: here a [`RootElement`](crate::RootElement) names
//! its host ([`RootElement::live_region_host`](crate::RootElement)), and
//! the host speaks each new `textContent` of the region it mounts, at a
//! priority from its politeness.
//!
//! Not ported: the focus trap, the roving tabindex and the focus manager,
//! which move DOM focus between HTML elements; a native view gets that
//! from its platform (rust/PARITY.md).

use crate::extension::named_signals_config;
use crate::extension::root_element_extension;
use crate::extension::signals::{Signal, effect, signal};
use crate::extension_core::{
    ExtensionBuildState, ExtensionConfigBase, ExtensionInitState, ExtensionRegisterState,
    InitialEditorConfig, LexicalExtension, define_extension,
};
use crate::merge_register::merge_register;
use crate::{
    COMMAND_PRIORITY_LOW, Editor, LexicalError, REDO_COMMAND, RootElementRef, UNDO_COMMAND,
    Unregister,
};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// `AriaPoliteness`: how urgently a screen reader speaks an announcement
/// (`aria-live`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AriaPoliteness {
    /// `'polite'`: after what the reader is saying.
    #[default]
    Polite,
    /// `'assertive'`: at once.
    Assertive,
}

impl AriaPoliteness {
    /// The politeness as JS names it.
    pub fn as_str(&self) -> &'static str {
        match self {
            AriaPoliteness::Polite => "polite",
            AriaPoliteness::Assertive => "assertive",
        }
    }

    /// The politeness a string names.
    pub fn parse(politeness: &str) -> Option<Self> {
        match politeness {
            "polite" => Some(AriaPoliteness::Polite),
            "assertive" => Some(AriaPoliteness::Assertive),
            _ => None,
        }
    }
}

/// The editor's live region, the element `createLiveRegion` makes (a
/// `role="status"` element with `aria-atomic="true"`, its `aria-live` and
/// `textContent` set as they change). A [`LiveRegionHost`] shows it.
#[derive(Debug)]
pub struct LiveRegion {
    politeness: Cell<Option<AriaPoliteness>>,
    text_content: RefCell<String>,
    host: RefCell<Option<Weak<dyn LiveRegionHost>>>,
}

impl LiveRegion {
    /// `role`: always `"status"`.
    pub fn role(&self) -> &'static str {
        "status"
    }

    /// `aria-atomic`: always true, so the whole region is read.
    pub fn atomic(&self) -> bool {
        true
    }

    /// `aria-live`: `None` until the extension applies its politeness,
    /// right after mounting the region.
    pub fn politeness(&self) -> Option<AriaPoliteness> {
        self.politeness.get()
    }

    /// `textContent`: the last announcement. A repeat of the same text ends
    /// in U+200B, as in JS, so the change still registers; a host drops it
    /// before speaking.
    pub fn text_content(&self) -> String {
        self.text_content.borrow().clone()
    }

    fn host(&self) -> Option<Rc<dyn LiveRegionHost>> {
        self.host.borrow().as_ref().and_then(Weak::upgrade)
    }

    fn set_politeness(self: &Rc<Self>, politeness: AriaPoliteness) {
        self.politeness.set(Some(politeness));
        if let Some(host) = self.host() {
            host.live_region_did_update(self);
        }
    }

    fn set_text_content(self: &Rc<Self>, text: String) {
        *self.text_content.borrow_mut() = text;
        if let Some(host) = self.host() {
            host.live_region_did_update(self);
        }
    }

    /// `el.remove()`.
    fn remove(self: &Rc<Self>) {
        let host = self.host.borrow_mut().take().and_then(|h| h.upgrade());
        if let Some(host) = host {
            host.unmount_live_region(self);
        }
    }
}

/// What JS's live region is appended to: the configured `owner`, or the
/// root element's document body ([`RootElement::live_region_host`]). The
/// host keeps the regions it mounts, and speaks each change of their
/// `textContent`.
///
/// [`RootElement::live_region_host`]: crate::RootElement::live_region_host
pub trait LiveRegionHost {
    /// `owner.appendChild(region)`: the region was made on this host.
    fn mount_live_region(&self, region: &Rc<LiveRegion>) {
        let _ = region;
    }
    /// `region.remove()`: the root element or the `owner` changed, or the
    /// editor was disposed.
    fn unmount_live_region(&self, region: &Rc<LiveRegion>) {
        let _ = region;
    }
    /// The region's `textContent` or `aria-live` changed.
    fn live_region_did_update(&self, region: &Rc<LiveRegion>) {
        let _ = region;
    }
}

/// A live region host, compared by identity (as JS's `owner` element).
#[derive(Clone)]
pub struct LiveRegionHostRef(pub Rc<dyn LiveRegionHost>);

impl PartialEq for LiveRegionHostRef {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(Rc::as_ptr(&self.0), Rc::as_ptr(&other.0))
    }
}

impl std::fmt::Debug for LiveRegionHostRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "LiveRegionHostRef({:p})",
            Rc::as_ptr(&self.0) as *const ()
        )
    }
}

/// A mounted region, compared by identity.
#[derive(Clone)]
struct RegionRef(Rc<LiveRegion>);

impl PartialEq for RegionRef {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// `AriaLiveRegionExtensionConfig`.
#[derive(Clone, Default)]
pub struct AriaLiveRegionExtensionConfig {
    /// `politeness`: the [`AriaPoliteness`] of the mounted region.
    pub politeness: AriaPoliteness,
    /// `owner`: the host to mount the region on; `None` mounts it on the
    /// root element's host.
    pub owner: Option<LiveRegionHostRef>,
}

/// `Partial<AriaLiveRegionExtensionConfig>`.
#[derive(Clone, Default)]
pub struct AriaLiveRegionExtensionConfigOverrides {
    pub politeness: Option<AriaPoliteness>,
    pub owner: Option<Option<LiveRegionHostRef>>,
}

impl ExtensionConfigBase for AriaLiveRegionExtensionConfig {
    type Partial = AriaLiveRegionExtensionConfigOverrides;

    fn shallow_merge(&self, overrides: &Self::Partial) -> Self {
        AriaLiveRegionExtensionConfig {
            politeness: overrides.politeness.unwrap_or(self.politeness),
            owner: overrides
                .owner
                .clone()
                .unwrap_or_else(|| self.owner.clone()),
        }
    }
}

/// `AriaLiveRegion`, the output of `AriaLiveRegionExtension`: its config as
/// signals (`namedSignals`), which set the mounted region's `aria-live` and
/// mount it again on another `owner`, and a stable
/// [`announce`](Self::announce) sink.
#[derive(Clone)]
pub struct AriaLiveRegion {
    pub politeness: Signal<AriaPoliteness>,
    pub owner: Signal<Option<LiveRegionHostRef>>,
    message: Signal<String>,
}

impl AriaLiveRegion {
    /// `announce(message)`: writes a message into the editor's live
    /// region. Announcing the same string back to back appends a zero-width
    /// space, so the region still changes. Does nothing until a region is
    /// mounted (the editor has a root element with a host, or an `owner`
    /// is configured).
    pub fn announce(&self, text: &str) {
        // Toggle a trailing zero-width space on repeats so re-announcing the
        // same text still registers as a textContent change downstream.
        let message = if *text == self.message.peek() {
            format!("{text}\u{200B}")
        } else {
            text.to_owned()
        };
        self.message.set_value(message);
    }
}

/// `AriaLiveRegionExtension`.
pub type AriaLiveRegionExtensionType =
    LexicalExtension<AriaLiveRegionExtensionConfig, AriaLiveRegion, Signal<String>>;

fn make_aria_live_region_extension() -> Rc<AriaLiveRegionExtensionType> {
    define_extension(LexicalExtension {
        // Private reactive message buffer shared by `build` (writer) and
        // `register` (renderer).
        init: Some(Rc::new(
            |_: &mut InitialEditorConfig,
             _: &AriaLiveRegionExtensionConfig,
             _: &ExtensionInitState| Ok(signal(String::new())),
        )),
        build: Some(Rc::new(
            |_: &Editor,
             config: &AriaLiveRegionExtensionConfig,
             state: &ExtensionBuildState<Signal<String>>| {
                Ok(AriaLiveRegion {
                    politeness: signal(config.politeness),
                    owner: signal(config.owner.clone()),
                    message: (*state.get_init_result()?).clone(),
                })
            },
        )),
        dependencies: vec![root_element_extension().into()],
        register: Some(Rc::new(
            |_: &Editor,
             _: &AriaLiveRegionExtensionConfig,
             state: &ExtensionRegisterState<Signal<String>, AriaLiveRegion>| {
                let message = (*state.get_init_result()?).clone();
                let output = state
                    .get_output()
                    .expect("AriaLiveRegionExtension has a build");
                let root_element = state
                    .get_dependency(&root_element_extension())?
                    .output
                    .expect("RootElementExtension has a build");
                let region: Signal<Option<RegionRef>> = signal(None);
                let (owner, politeness) = (output.owner.clone(), output.politeness.clone());
                let mounted = region.clone();
                Ok(merge_register(vec![
                    // Create / dispose the region as the root (or `owner`)
                    // changes. Subscribe to the root only when no explicit
                    // `owner` is configured, so a fixed-owner region isn't
                    // torn down on unrelated root churn.
                    effect(move || {
                        let host = match owner.value() {
                            Some(owner) => owner.0,
                            None => root_element
                                .value()
                                .and_then(|RootElementRef(root)| root.live_region_host())?,
                        };
                        let el = Rc::new(LiveRegion {
                            politeness: Cell::new(None),
                            text_content: RefCell::new(String::new()),
                            host: RefCell::new(Some(Rc::downgrade(&host))),
                        });
                        host.mount_live_region(&el);
                        mounted.set_value(Some(RegionRef(el.clone())));
                        let mounted = mounted.clone();
                        Some(Box::new(move || {
                            el.remove();
                            mounted.set_value(None);
                        }) as Unregister)
                    }),
                    // Apply the politeness to whatever region is mounted.
                    // This tracks `region`, so it applies it to a new region
                    // too (an attribute, not an announcement).
                    {
                        let region = region.clone();
                        effect(move || {
                            if let Some(RegionRef(el)) = region.value() {
                                el.set_politeness(politeness.value());
                            }
                            None
                        })
                    },
                    // Mirror the current message into whatever region is
                    // mounted. The region is peeked, so this runs again only
                    // when the message changes: a new region does not replay
                    // the last message.
                    effect(move || {
                        let text = message.value();
                        if let Some(RegionRef(el)) = region.peek() {
                            el.set_text_content(text);
                        }
                        None
                    }),
                ]))
            },
        )),
        ..LexicalExtension::new(
            "@lexical/a11y/AriaLiveRegion",
            AriaLiveRegionExtensionConfig::default(),
        )
    })
}

named_signals_config! {
    /// `HistoryAnnounceExtensionConfig`.
    pub struct HistoryAnnounceExtensionConfig;
    /// `Partial<HistoryAnnounceExtensionConfig>`.
    pub struct HistoryAnnounceExtensionConfigOverrides;
    /// `namedSignals(HistoryAnnounceExtensionConfig)`:
    /// `HistoryAnnounceExtension`'s output.
    pub struct HistoryAnnounceOutput {
        /// `undone`: announced after an undo.
        undone: String,
        /// `redone`: announced after a redo.
        redone: String,
        /// `disabled`: undo and redo are not announced.
        disabled: bool,
    }
}

/// `HistoryAnnounceExtension`.
pub type HistoryAnnounceExtensionType =
    LexicalExtension<HistoryAnnounceExtensionConfig, HistoryAnnounceOutput, ()>;

/// The `announce` of the editor's `AriaLiveRegionExtension`.
fn announcer<I: 'static, O: 'static>(
    state: &ExtensionRegisterState<I, O>,
) -> Result<AriaLiveRegion, LexicalError> {
    Ok((*state
        .get_dependency(&aria_live_region_extension())?
        .output
        .expect("AriaLiveRegionExtension has a build"))
    .clone())
}

fn make_history_announce_extension() -> Rc<HistoryAnnounceExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |_: &Editor, config: &HistoryAnnounceExtensionConfig, _: &ExtensionBuildState<()>| {
                Ok(HistoryAnnounceOutput::of(config))
            },
        )),
        dependencies: vec![aria_live_region_extension().into()],
        register: Some(Rc::new(
            |editor: &Editor,
             _: &HistoryAnnounceExtensionConfig,
             state: &ExtensionRegisterState<(), HistoryAnnounceOutput>| {
                let output = state
                    .get_output()
                    .expect("HistoryAnnounceExtension has a build");
                let announcer = announcer(state)?;
                let editor = editor.weak();
                // Gate registration on `disabled` from an effect, so a
                // disabled announcer registers nothing.
                Ok(effect(move || {
                    if output.disabled.value() {
                        return None;
                    }
                    let editor = editor.upgrade().map(Editor)?;
                    let (undone, redone) = (output.undone.clone(), output.redone.clone());
                    let (on_undo, on_redo) = (announcer.clone(), announcer.clone());
                    // Both return false, to keep the history command chain
                    // intact.
                    Some(merge_register(vec![
                        editor.register_command(
                            &UNDO_COMMAND,
                            move |_, _| {
                                on_undo.announce(&undone.peek());
                                Ok(false)
                            },
                            COMMAND_PRIORITY_LOW,
                        ),
                        editor.register_command(
                            &REDO_COMMAND,
                            move |_, _| {
                                on_redo.announce(&redone.peek());
                                Ok(false)
                            },
                            COMMAND_PRIORITY_LOW,
                        ),
                    ]))
                }))
            },
        )),
        ..LexicalExtension::new(
            "@lexical/a11y/HistoryAnnounce",
            HistoryAnnounceExtensionConfig {
                undone: "Undone".to_owned(),
                redone: "Redone".to_owned(),
                disabled: false,
            },
        )
    })
}

named_signals_config! {
    /// `EditorModeAnnounceExtensionConfig`.
    pub struct EditorModeAnnounceExtensionConfig;
    /// `Partial<EditorModeAnnounceExtensionConfig>`.
    pub struct EditorModeAnnounceExtensionConfigOverrides;
    /// `namedSignals(EditorModeAnnounceExtensionConfig)`:
    /// `EditorModeAnnounceExtension`'s output.
    pub struct EditorModeAnnounceOutput {
        /// `editable`: announced when the editor becomes editable.
        editable: String,
        /// `readOnly`: announced when the editor becomes read-only.
        read_only: String,
        /// `disabled`: editable and read-only transitions are not
        /// announced.
        disabled: bool,
    }
}

/// `EditorModeAnnounceExtension`.
pub type EditorModeAnnounceExtensionType =
    LexicalExtension<EditorModeAnnounceExtensionConfig, EditorModeAnnounceOutput, ()>;

fn make_editor_mode_announce_extension() -> Rc<EditorModeAnnounceExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |_: &Editor,
             config: &EditorModeAnnounceExtensionConfig,
             _: &ExtensionBuildState<()>| { Ok(EditorModeAnnounceOutput::of(config)) },
        )),
        dependencies: vec![aria_live_region_extension().into()],
        register: Some(Rc::new(
            |editor: &Editor,
             _: &EditorModeAnnounceExtensionConfig,
             state: &ExtensionRegisterState<(), EditorModeAnnounceOutput>| {
                let output = state
                    .get_output()
                    .expect("EditorModeAnnounceExtension has a build");
                let announcer = announcer(state)?;
                let editor = editor.weak();
                Ok(effect(move || {
                    if output.disabled.value() {
                        return None;
                    }
                    let editor = editor.upgrade().map(Editor)?;
                    let (editable, read_only) = (output.editable.clone(), output.read_only.clone());
                    let announcer = announcer.clone();
                    Some(editor.register_editable_listener(move |is_editable| {
                        announcer.announce(&if is_editable {
                            editable.peek()
                        } else {
                            read_only.peek()
                        });
                    }))
                }))
            },
        )),
        ..LexicalExtension::new(
            "@lexical/a11y/EditorModeAnnounce",
            EditorModeAnnounceExtensionConfig {
                editable: "Editor is editable".to_owned(),
                read_only: "Editor is read-only".to_owned(),
                disabled: false,
            },
        )
    })
}

thread_local! {
    static ARIA_LIVE_REGION_EXTENSION: Rc<AriaLiveRegionExtensionType> =
        make_aria_live_region_extension();
    static HISTORY_ANNOUNCE_EXTENSION: Rc<HistoryAnnounceExtensionType> =
        make_history_announce_extension();
    static EDITOR_MODE_ANNOUNCE_EXTENSION: Rc<EditorModeAnnounceExtensionType> =
        make_editor_mode_announce_extension();
}

/// `AriaLiveRegionExtension`: owns one live region for the editor, on the
/// configured `owner` or the root element's host, and exposes the
/// [`AriaLiveRegion`] sink the other announcers write to.
pub fn aria_live_region_extension() -> Rc<AriaLiveRegionExtensionType> {
    ARIA_LIVE_REGION_EXTENSION.with(Rc::clone)
}

/// `HistoryAnnounceExtension`: announces undo and redo
/// (`UNDO_COMMAND`, `REDO_COMMAND`) through
/// [`aria_live_region_extension`]'s sink, unless `disabled`.
pub fn history_announce_extension() -> Rc<HistoryAnnounceExtensionType> {
    HISTORY_ANNOUNCE_EXTENSION.with(Rc::clone)
}

/// `EditorModeAnnounceExtension`: announces
/// [`set_editable`](Editor::set_editable) transitions through
/// [`aria_live_region_extension`]'s sink, unless `disabled`.
pub fn editor_mode_announce_extension() -> Rc<EditorModeAnnounceExtensionType> {
    EDITOR_MODE_ANNOUNCE_EXTENSION.with(Rc::clone)
}

/// `message.replace('%s', value)`: the first `%s` only, as JS's string
/// `replace` does.
#[cfg(any(feature = "rich-text", feature = "link"))]
pub(crate) fn replace_first(message: &str, value: &str) -> String {
    message.replacen("%s", value, 1)
}
