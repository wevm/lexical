//! `AutoLinkAnnounceExtension.ts` (with the `a11y` feature).

use crate::a11y::{aria_live_region_extension, replace_first};
use crate::error::invariant;
use crate::extension::named_signals_config;
use crate::extension::signals::effect;
use crate::extension_core::{
    ExtensionBuildState, ExtensionRegisterState, LexicalExtension, define_extension,
};
use crate::{Editor, MutationListenerOptions, NodeMutation, Unregister};
use std::rc::Rc;

named_signals_config! {
    /// `AutoLinkAnnounceExtensionConfig`.
    pub struct AutoLinkAnnounceExtensionConfig;
    /// `Partial<AutoLinkAnnounceExtensionConfig>`.
    pub struct AutoLinkAnnounceExtensionConfigOverrides;
    /// `namedSignals(AutoLinkAnnounceExtensionConfig)`:
    /// `AutoLinkAnnounceExtension`'s output.
    pub struct AutoLinkAnnounceOutput {
        /// `created`: announced when typing turns text into a link.
        created: String,
        /// `createdMany`: announced when several links appear at once;
        /// `%s` is replaced with how many.
        created_many: String,
        /// `destroyed`: announced when an automatic link stops being one.
        destroyed: String,
        /// `destroyedMany`: announced when several links go at once; `%s`
        /// is replaced with how many.
        destroyed_many: String,
        /// `disabled`: automatic links are not announced.
        disabled: bool,
    }
}

/// `AutoLinkAnnounceExtension`.
pub type AutoLinkAnnounceExtensionType =
    LexicalExtension<AutoLinkAnnounceExtensionConfig, AutoLinkAnnounceOutput, ()>;

fn make() -> Rc<AutoLinkAnnounceExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |_: &Editor, config: &AutoLinkAnnounceExtensionConfig, _: &ExtensionBuildState<()>| {
                Ok(AutoLinkAnnounceOutput::of(config))
            },
        )),
        dependencies: vec![aria_live_region_extension().into()],
        register: Some(Rc::new(
            |editor: &Editor,
             _: &AutoLinkAnnounceExtensionConfig,
             state: &ExtensionRegisterState<(), AutoLinkAnnounceOutput>| {
                let output = state
                    .get_output()
                    .expect("AutoLinkAnnounceExtension has a build");
                let announcer = (*state
                    .get_dependency(&aria_live_region_extension())?
                    .output
                    .expect("AriaLiveRegionExtension has a build"))
                .clone();
                invariant!(
                    editor.registered_nodes().contains_key("autolink"),
                    "Node AutoLinkNode has not been registered. Ensure node has been passed to createEditor."
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
                    let output = output.clone();
                    let announcer = announcer.clone();
                    editor
                        .register_mutation_listener(
                            "autolink",
                            move |nodes, _| {
                                let created_count = nodes
                                    .iter()
                                    .filter(|(_, m)| *m == NodeMutation::Created)
                                    .count();
                                let destroyed_count = nodes
                                    .iter()
                                    .filter(|(_, m)| *m == NodeMutation::Destroyed)
                                    .count();
                                // Deleting a selection takes every link in it
                                // at once, so say how many.
                                if created_count > 0 && destroyed_count == 0 {
                                    announcer.announce(&if created_count == 1 {
                                        output.created.peek()
                                    } else {
                                        replace_first(
                                            &output.created_many.peek(),
                                            &created_count.to_string(),
                                        )
                                    });
                                } else if destroyed_count > 0 && created_count == 0 {
                                    announcer.announce(&if destroyed_count == 1 {
                                        output.destroyed.peek()
                                    } else {
                                        replace_first(
                                            &output.destroyed_many.peek(),
                                            &destroyed_count.to_string(),
                                        )
                                    });
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
            "@lexical/link/AutoLinkAnnounce",
            AutoLinkAnnounceExtensionConfig {
                created: "Link".to_owned(),
                created_many: "%s links".to_owned(),
                destroyed: "Link removed".to_owned(),
                destroyed_many: "%s links removed".to_owned(),
                disabled: false,
            },
        )
    })
}

thread_local! {
    static AUTO_LINK_ANNOUNCE_EXTENSION: Rc<AutoLinkAnnounceExtensionType> = make();
}

/// `AutoLinkAnnounceExtension`: announces automatic links that are created
/// and removed (one, or how many at once) through
/// [`aria_live_region_extension`]'s sink, unless `disabled`. It listens to
/// `AutoLinkNode`'s mutations, which the editor reports with a root
/// element.
pub fn auto_link_announce_extension() -> Rc<AutoLinkAnnounceExtensionType> {
    AUTO_LINK_ANNOUNCE_EXTENSION.with(Rc::clone)
}
