//! `getExtensionDependencyFromEditor.ts`.

use super::lexical_builder::LexicalBuilder;
use crate::error::invariant;
use crate::extension_core::{ExtensionConfigBase, LexicalExtension, LexicalExtensionDependency};
use crate::{Editor, LexicalError};
use std::rc::Rc;

/// `getExtensionDependencyFromEditor(editor, extension)`: the merged config,
/// init result and output of an extension the editor was built with. An
/// error when the editor wasn't built from extensions, or without this
/// one.
pub fn get_extension_dependency_from_editor<C: ExtensionConfigBase, O: 'static, I: 'static>(
    editor: &Editor,
    extension: &Rc<LexicalExtension<C, O, I>>,
) -> Result<LexicalExtensionDependency<C, O, I>, LexicalError> {
    let builder = LexicalBuilder::from_editor(editor)?;
    let rep = builder.0.get_extension_rep(extension.as_ref())?;
    invariant!(
        rep.is_some(),
        "getExtensionDependencyFromEditor: Extension {} was not built when creating this editor",
        extension.name
    );
    let rep = rep.expect("checked");
    rep.get_extension_dependency()?.typed(&extension.name)
}
