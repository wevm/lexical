//! `HeadingAnnounceExtension.ts` (with the `a11y` feature).

use super::HeadingTagType;
use crate::a11y::{aria_live_region_extension, replace_first};
use crate::error::invariant;
use crate::extension::named_signals_config;
use crate::extension::signals::effect;
use crate::extension_core::{
    ExtensionBuildState, ExtensionRegisterState, LexicalExtension, define_extension,
};
use crate::{Editor, MutationListenerOptions, NodeMutation, Unregister, get_node_by_key};
use std::rc::Rc;

named_signals_config! {
    /// `HeadingAnnounceExtensionConfig`.
    pub struct HeadingAnnounceExtensionConfig;
    /// `Partial<HeadingAnnounceExtensionConfig>`.
    pub struct HeadingAnnounceExtensionConfigOverrides;
    /// `namedSignals(HeadingAnnounceExtensionConfig)`:
    /// `HeadingAnnounceExtension`'s output.
    pub struct HeadingAnnounceOutput {
        /// `created`: announced when a block becomes a heading; `%s` is
        /// replaced with the level (1-6).
        created: String,
        /// `destroyed`: announced when a heading stops being one; `%s` is
        /// replaced with the level it was.
        destroyed: String,
        /// `disabled`: headings are not announced.
        disabled: bool,
    }
}

/// `HeadingAnnounceExtension`.
pub type HeadingAnnounceExtensionType =
    LexicalExtension<HeadingAnnounceExtensionConfig, HeadingAnnounceOutput, ()>;

/// `$readHeadingTag(key)`.
fn read_heading_tag(key: &str) -> Option<HeadingTagType> {
    get_node_by_key(key).and_then(|node| node.get_heading_tag())
}

fn make() -> Rc<HeadingAnnounceExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |_: &Editor, config: &HeadingAnnounceExtensionConfig, _: &ExtensionBuildState<()>| {
                Ok(HeadingAnnounceOutput::of(config))
            },
        )),
        dependencies: vec![aria_live_region_extension().into()],
        register: Some(Rc::new(
            |editor: &Editor,
             _: &HeadingAnnounceExtensionConfig,
             state: &ExtensionRegisterState<(), HeadingAnnounceOutput>| {
                let output = state
                    .get_output()
                    .expect("HeadingAnnounceExtension has a build");
                let announcer = (*state
                    .get_dependency(&aria_live_region_extension())?
                    .output
                    .expect("AriaLiveRegionExtension has a build"))
                .clone();
                invariant!(
                    editor.registered_nodes().contains_key("heading"),
                    "Node HeadingNode has not been registered. Ensure node has been passed to createEditor."
                );
                let weak = editor.weak();
                // Gate on `disabled` from an effect, so a disabled announcer
                // registers no listener at all. The messages are peeked at
                // announce time, so editing them does not register again.
                Ok(effect(move || {
                    if output.disabled.value() {
                        return None;
                    }
                    let editor = weak.upgrade().map(Editor)?;
                    let (created, destroyed) = (output.created.clone(), output.destroyed.clone());
                    let announcer = announcer.clone();
                    let latest = editor.weak();
                    editor
                        .register_mutation_listener(
                            "heading",
                            move |nodes, payload| {
                                // A level change fires both a removal and a
                                // creation; prefer the creation, and announce a
                                // removal only when nothing replaced it.
                                let (mut created_tag, mut destroyed_tag) = (None, None);
                                for (key, mutation) in nodes {
                                    if *mutation == NodeMutation::Created && created_tag.is_none() {
                                        // `editor.read('latest', ...)`.
                                        created_tag =
                                            latest.upgrade().map(Editor).and_then(|editor| {
                                                editor
                                                    .get_editor_state()
                                                    .read(|| read_heading_tag(key))
                                            });
                                    } else if *mutation == NodeMutation::Destroyed
                                        && destroyed_tag.is_none()
                                    {
                                        destroyed_tag = payload
                                            .prev_editor_state
                                            .read(|| read_heading_tag(key));
                                    }
                                }
                                if let Some(tag) = created_tag {
                                    announcer.announce(&replace_first(
                                        &created.peek(),
                                        &tag.as_str()[1..],
                                    ));
                                } else if let Some(tag) = destroyed_tag {
                                    announcer.announce(&replace_first(
                                        &destroyed.peek(),
                                        &tag.as_str()[1..],
                                    ));
                                }
                            },
                            MutationListenerOptions {
                                skip_initialization: true,
                            },
                        )
                        .ok()
                        .map(|unregister| unregister as Unregister)
                }))
            },
        )),
        ..LexicalExtension::new(
            "@lexical/rich-text/HeadingAnnounce",
            HeadingAnnounceExtensionConfig {
                created: "Heading level %s".to_owned(),
                destroyed: "Heading level %s removed".to_owned(),
                disabled: false,
            },
        )
    })
}

thread_local! {
    static HEADING_ANNOUNCE_EXTENSION: Rc<HeadingAnnounceExtensionType> = make();
}

/// `HeadingAnnounceExtension`: announces headings that are created (a block
/// made a heading, or a heading of another level) and removed, through
/// [`aria_live_region_extension`]'s sink, unless `disabled`. It listens to
/// `HeadingNode`'s mutations, which the editor reports with a root element.
pub fn heading_announce_extension() -> Rc<HeadingAnnounceExtensionType> {
    HEADING_ANNOUNCE_EXTENSION.with(Rc::clone)
}
