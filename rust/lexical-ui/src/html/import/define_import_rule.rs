//! `import/defineImportRule.ts`, with the rule types of `import/types.ts`.

use super::run_import::DomImportContext;
use super::sel::CompiledSelector;
use crate::LexicalError;
use crate::dom::DomNode;
use crate::node::NodeRef;
use std::rc::Rc;

/// A rule's `$next`: the result of the next matching rule (or, past the
/// last, of hoisting the node's children).
pub type ImportNext<'a> = &'a dyn Fn() -> Result<Vec<NodeRef>, LexicalError>;

/// `DOMImportFn`: converts a matched DOM node into Lexical nodes. Return
/// `[]` to drop the node, or call `next` to delegate.
pub type DomImportFn =
    Rc<dyn Fn(&DomImportContext, &DomNode, ImportNext<'_>) -> Result<Vec<NodeRef>, LexicalError>>;

/// `DOMImportRule`: converts the DOM nodes its `match_` accepts.
#[derive(Clone)]
pub struct DomImportRule {
    /// `name`: shown in errors; `'@scope/package/rule-id'` by convention.
    pub name: Option<String>,
    /// `match`.
    pub match_: CompiledSelector,
    /// `$import`.
    pub import: DomImportFn,
}

/// `defineImportRule({name, match, $import})`.
pub fn define_import_rule(
    name: Option<&str>,
    match_: CompiledSelector,
    import: impl Fn(&DomImportContext, &DomNode, ImportNext<'_>) -> Result<Vec<NodeRef>, LexicalError>
    + 'static,
) -> DomImportRule {
    DomImportRule {
        name: name.map(str::to_owned),
        match_,
        import: Rc::new(import),
    }
}
