//! `getPeerDependencyFromEditor.ts`.

use super::lexical_builder::LexicalBuilder;
use crate::extension_core::LexicalExtensionDependency;
use crate::{Editor, LexicalError};

/// `getPeerDependencyFromEditor(editor, name)`: the merged config, init
/// result and output of the extension named `name`, as an extension of the
/// given types, if the editor was built with it; `None` if it wasn't, or
/// wasn't built from extensions. JS takes the extension's type as a type
/// argument for the same reason: the caller has no reference to it.
pub fn get_peer_dependency_from_editor<C: 'static, O: 'static, I: 'static>(
    editor: &Editor,
    extension_name: &str,
) -> Result<Option<LexicalExtensionDependency<C, O, I>>, LexicalError> {
    let Some(builder) = LexicalBuilder::maybe_from_editor(editor) else {
        return Ok(None);
    };
    let peer = builder.0.extension_rep_by_name(extension_name);
    match peer {
        None => Ok(None),
        Some(peer) => peer
            .get_extension_dependency()?
            .typed(extension_name)
            .map(Some),
    }
}

/// `getPeerDependencyFromEditorOrThrow(editor, name)`: as
/// [`get_peer_dependency_from_editor`], an error when the editor wasn't
/// built with the extension.
pub fn get_peer_dependency_from_editor_or_throw<C: 'static, O: 'static, I: 'static>(
    editor: &Editor,
    extension_name: &str,
) -> Result<LexicalExtensionDependency<C, O, I>, LexicalError> {
    get_peer_dependency_from_editor(editor, extension_name)?.ok_or_else(|| {
        LexicalError::new(format!(
            "getPeerDependencyFromEditorOrThrow: Editor was not built with Extension {extension_name}"
        ))
    })
}
