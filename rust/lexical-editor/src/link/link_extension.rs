//! `LexicalLinkExtension.ts`: `LinkConfig` and `registerLink`.

use super::link_node::{
    LinkAttributes, TOGGLE_LINK_COMMAND, ToggleLinkPayload, link_node_transform, toggle_link,
};
use crate::editor::COMMAND_PRIORITY_EDITOR;
use crate::{Editor, Unregister};
use std::fmt;
use std::rc::Rc;

/// `validateUrl`'s function.
pub type ValidateUrl = Rc<dyn Fn(&str) -> bool>;

/// `LinkConfig`, whose defaults (`LinkExtension`'s) leave both unset.
#[derive(Clone, Default)]
pub struct LinkConfig {
    /// `validateUrl`: when set, `TOGGLE_LINK_COMMAND` rejects (doesn't
    /// handle) a URL it returns false for. JS also links the selection when
    /// such a URL is pasted, which needs clipboard data and isn't ported.
    pub validate_url: Option<ValidateUrl>,
    /// `attributes`: the attributes `TOGGLE_LINK_COMMAND` gives a link when
    /// its payload doesn't.
    pub attributes: Option<LinkAttributes>,
}

impl fmt::Debug for LinkConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinkConfig")
            .field("validate_url", &self.validate_url.as_ref().map(|_| "fn"))
            .field("attributes", &self.attributes)
            .finish()
    }
}

pub(super) fn merge(unregisters: Vec<Unregister>) -> Unregister {
    Box::new(move || {
        for unregister in unregisters {
            unregister();
        }
    })
}

/// `registerLink(editor, stores)`: [`link_node_transform`] for `LinkNode`
/// and the `TOGGLE_LINK_COMMAND` handler, and returns what unregisters
/// them. JS reads the config from signals each time; here it is fixed when
/// registering (`LinkExtension` passes its signals).
///
/// With `validateUrl` set, JS also handles `PASTE_COMMAND`, linking the
/// selection to a pasted URL; that reads a `ClipboardEvent`'s data and waits
/// for an input layer.
pub fn register_link(editor: &Editor, config: LinkConfig) -> Unregister {
    let LinkConfig {
        validate_url,
        attributes,
    } = config;
    register_link_with(
        editor,
        Rc::new(move || validate_url.clone()),
        Rc::new(move || attributes.clone()),
    )
}

/// `registerLink(editor, stores)` with the config read through
/// `validate_url` and `attributes` (`stores.validateUrl.peek()`,
/// `stores.attributes.peek()`) each time the command is handled.
pub(crate) fn register_link_with(
    editor: &Editor,
    validate_url: Rc<dyn Fn() -> Option<ValidateUrl>>,
    attributes: Rc<dyn Fn() -> Option<LinkAttributes>>,
) -> Unregister {
    merge(vec![
        editor.register_node_transform("link", link_node_transform),
        editor.register_command(
            &TOGGLE_LINK_COMMAND,
            move |payload, _| {
                let validate_url = validate_url();
                let attributes = attributes().unwrap_or_default();
                match payload {
                    None => {
                        toggle_link(None, &LinkAttributes::default())?;
                        Ok(true)
                    }
                    Some(ToggleLinkPayload::Url(url)) => {
                        if validate_url.as_ref().is_none_or(|validate| validate(url)) {
                            toggle_link(Some(url), &attributes)?;
                            return Ok(true);
                        }
                        Ok(false)
                    }
                    Some(ToggleLinkPayload::Attributes {
                        url,
                        attributes: payload_attributes,
                    }) => {
                        // Only the attributes the payload actually carries
                        // may override the configured defaults. Same gate
                        // as the string payload above: validateUrl is
                        // documented as rejecting URLs for this command,
                        // not for one of its two shapes.
                        if validate_url.as_ref().is_some_and(|validate| !validate(url)) {
                            return Ok(false);
                        }
                        toggle_link(Some(url), &attributes.merging(payload_attributes))?;
                        Ok(true)
                    }
                }
            },
            COMMAND_PRIORITY_EDITOR,
        ),
    ])
}

#[cfg(feature = "extension")]
pub use extension::*;

#[cfg(feature = "extension")]
mod extension {
    use super::{LinkConfig, ValidateUrl, register_link_with};
    use crate::extension::signals::{Signal, signal};
    use crate::extension_core::{
        ExtensionBuildState, ExtensionConfigBase, ExtensionRegisterState, LexicalExtension,
        define_extension,
    };
    use crate::link::{LinkAttributes, LinkNode};
    use crate::{Editor, NodeClass};
    use std::rc::Rc;

    /// `LinkConfig.validateUrl` as a signal holds it: compared by identity,
    /// as JS compares functions.
    #[derive(Clone)]
    pub struct ValidateUrlFn(pub ValidateUrl);

    impl PartialEq for ValidateUrlFn {
        fn eq(&self, other: &Self) -> bool {
            Rc::ptr_eq(&self.0, &other.0)
        }
    }

    /// `Partial<LinkConfig>`: `Some(None)` sets a field to `undefined`.
    #[derive(Clone, Default)]
    pub struct LinkConfigOverrides {
        pub validate_url: Option<Option<ValidateUrl>>,
        pub attributes: Option<Option<LinkAttributes>>,
    }

    impl ExtensionConfigBase for LinkConfig {
        type Partial = LinkConfigOverrides;

        fn shallow_merge(&self, overrides: &LinkConfigOverrides) -> Self {
            LinkConfig {
                validate_url: overrides
                    .validate_url
                    .clone()
                    .unwrap_or_else(|| self.validate_url.clone()),
                attributes: overrides
                    .attributes
                    .clone()
                    .unwrap_or_else(|| self.attributes.clone()),
            }
        }
    }

    /// `LinkExtension.mergeConfig`: a shallow merge, except that configured
    /// `attributes` merge key by key with the overrides' (`undefined`
    /// overrides leave them as they are).
    fn merge_link_config(config: &LinkConfig, overrides: &LinkConfigOverrides) -> LinkConfig {
        let mut merged = config.shallow_merge(overrides);
        if let Some(attributes) = &config.attributes {
            merged.attributes = Some(match &merged.attributes {
                None => attributes.clone(),
                Some(over) => attributes.merging(over),
            });
        }
        merged
    }

    /// `namedSignals(LinkConfig)`: `LinkExtension`'s output.
    #[derive(Clone)]
    pub struct LinkOutput {
        pub validate_url: Signal<Option<ValidateUrlFn>>,
        pub attributes: Signal<Option<LinkAttributes>>,
    }

    /// `LinkExtension`.
    pub type LinkExtensionType = LexicalExtension<LinkConfig, LinkOutput, ()>;

    fn make() -> Rc<LinkExtensionType> {
        define_extension(LexicalExtension {
            build: Some(Rc::new(
                |_: &Editor, config: &LinkConfig, _: &ExtensionBuildState<()>| {
                    Ok(LinkOutput {
                        validate_url: signal(config.validate_url.clone().map(ValidateUrlFn)),
                        attributes: signal(config.attributes.clone()),
                    })
                },
            )),
            // JS's dependencies, `CoreImportExtension` and
            // `DOMImportExtension` with `LinkImportRules`, are the HTML
            // import, which comes with `html`.
            merge_config: Some(Rc::new(merge_link_config)),
            nodes: vec![NodeClass::of::<LinkNode>()],
            register: Some(Rc::new(
                |editor: &Editor,
                 _: &LinkConfig,
                 state: &ExtensionRegisterState<(), LinkOutput>| {
                    let stores = state.get_output().expect("LinkExtension has a build");
                    let attributes = stores.attributes.clone();
                    Ok(register_link_with(
                        editor,
                        Rc::new(move || stores.validate_url.peek().map(|v| v.0)),
                        Rc::new(move || attributes.peek()),
                    ))
                },
            )),
            ..LexicalExtension::new("@lexical/link/Link", LinkConfig::default())
        })
    }

    thread_local! {
        static LINK_EXTENSION: Rc<LinkExtensionType> = make();
    }

    /// `LinkExtension`: `LinkNode` and `TOGGLE_LINK_COMMAND`
    /// ([`register_link`](super::register_link)), reading its config from
    /// the signals of its output when the command is handled.
    pub fn link_extension() -> Rc<LinkExtensionType> {
        LINK_EXTENSION.with(Rc::clone)
    }
}
