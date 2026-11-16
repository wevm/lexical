//! `getExtensionDependency.ts`: the `$` forms of
//! `getExtensionDependencyFromEditor` and `getPeerDependencyFromEditor`, for
//! the editor whose update or read is running.

use super::get_extension_dependency_from_editor::get_extension_dependency_from_editor;
use super::get_peer_dependency_from_editor::get_peer_dependency_from_editor;
use crate::extension_core::{ExtensionConfigBase, LexicalExtension, LexicalExtensionDependency};
use crate::updates::get_active_editor;
use crate::{Editor, LexicalError};
use std::rc::Rc;

/// `$getEditor()`.
fn active_editor() -> Result<Editor, LexicalError> {
    get_active_editor().ok_or_else(|| {
        LexicalError::new(
            "Unable to find an active editor. This method can only be used synchronously during the callback of editor.update(), editor.read(), or editor.getEditorState().read(..., {editor}).",
        )
    })
}

/// `$getExtensionDependency(extension)`:
/// `getExtensionDependencyFromEditor($getEditor(), extension)`.
pub fn get_extension_dependency<C: ExtensionConfigBase, O: 'static, I: 'static>(
    extension: &Rc<LexicalExtension<C, O, I>>,
) -> Result<LexicalExtensionDependency<C, O, I>, LexicalError> {
    get_extension_dependency_from_editor(&active_editor()?, extension)
}

/// `$getExtensionOutput(extension)`: `$getExtensionDependency(extension).output`.
pub fn get_extension_output<C: ExtensionConfigBase, O: 'static, I: 'static>(
    extension: &Rc<LexicalExtension<C, O, I>>,
) -> Result<Option<Rc<O>>, LexicalError> {
    Ok(get_extension_dependency(extension)?.output)
}

/// `$getPeerDependency(name)`:
/// `getPeerDependencyFromEditor($getEditor(), name)`.
pub fn get_peer_dependency<C: 'static, O: 'static, I: 'static>(
    extension_name: &str,
) -> Result<Option<LexicalExtensionDependency<C, O, I>>, LexicalError> {
    get_peer_dependency_from_editor(&active_editor()?, extension_name)
}
