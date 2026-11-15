//! `LexicalLinkExtension.ts`: `LinkConfig` and `registerLink`.

use super::link_node::{
    LinkAttributes, TOGGLE_LINK_COMMAND, ToggleLinkPayload, link_node_transform, toggle_link,
};
use crate::editor::COMMAND_PRIORITY_EDITOR;
use crate::{Editor, Unregister};
use std::fmt;
use std::rc::Rc;

/// `validateUrl`'s function.
type ValidateUrl = Rc<dyn Fn(&str) -> bool>;

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

fn merge(unregisters: Vec<Unregister>) -> Unregister {
    Box::new(move || {
        for unregister in unregisters {
            unregister();
        }
    })
}

/// `registerLink(editor, stores)`: [`link_node_transform`] for `LinkNode`
/// and the `TOGGLE_LINK_COMMAND` handler, and returns what unregisters
/// them. JS reads the config from signals each time; here it is fixed when
/// registering.
///
/// With `validateUrl` set, JS also handles `PASTE_COMMAND`, linking the
/// selection to a pasted URL; that reads a `ClipboardEvent`'s data and waits
/// for an input layer.
pub fn register_link(editor: &Editor, config: LinkConfig) -> Unregister {
    let LinkConfig {
        validate_url,
        attributes,
    } = config;
    let attributes = attributes.unwrap_or_default();
    merge(vec![
        editor.register_node_transform("link", link_node_transform),
        editor.register_command(
            &TOGGLE_LINK_COMMAND,
            move |payload, _| match payload {
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
                    // Only the attributes the payload actually carries may
                    // override the configured defaults.
                    // Same gate as the string payload above: validateUrl is
                    // documented as rejecting URLs for this command, not for
                    // one of its two shapes.
                    if validate_url.as_ref().is_some_and(|validate| !validate(url)) {
                        return Ok(false);
                    }
                    toggle_link(Some(url), &attributes.merging(payload_attributes))?;
                    Ok(true)
                }
            },
            COMMAND_PRIORITY_EDITOR,
        ),
    ])
}
