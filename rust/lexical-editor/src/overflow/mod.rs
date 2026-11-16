//! `@lexical/overflow` (the `overflow` feature), from
//! `packages/lexical-overflow/src/index.ts`: `OverflowNode`, the element
//! `@lexical/react`'s character limit wraps the text past the limit in,
//! `$createOverflowNode` and `$isOverflowNode`. It has no schema of its
//! own, so its JSON is `ElementNode`'s.
//!
//! With the `extension` feature, `overflow_extension` is
//! `OverflowExtension`.
//!
//! `character_limit` is `@lexical/react`'s character limit
//! (`shared/useCharacterLimit.ts`), which makes overflow nodes:
//! `register_character_limit` (`useCharacterLimit`), `find_offset`,
//! `wrap_overflowed_nodes` and `merge_previous`, with `grapheme`, the
//! extended grapheme clusters JS's `Intl.Segmenter` gives `findOffset`.
//!
//! Not ported: `createDOM` and `updateDOM`, which come with `html`.

mod character_limit;
mod grapheme;
mod grapheme_tables;

pub use character_limit::{
    Strlen, find_offset, merge_previous, register_character_limit, wrap_overflowed_nodes,
};

use crate::generated_json::{
    export_compact_element_node, export_element_node, update_element_node,
};
use crate::json::Object;
use crate::lexical_selection::SelectionRef;
use crate::lexical_utils::set_node_key;
use crate::node::{LexicalNode, NodeBase, NodeRef};
use crate::nodes::element::ElementData;
use crate::{LexicalError, NodeClass};

/// `OverflowNode`: an element holding the content past a character limit.
/// It keeps `ElementNode.isInline()` (false), though the character limit
/// puts it in a paragraph, and it removes itself when it has no children
/// left.
#[derive(Clone, Debug, Default)]
pub struct OverflowNode {
    base: NodeBase,
    element: ElementData,
}

/// `$createOverflowNode()`.
pub fn create_overflow_node() -> NodeRef {
    set_node_key(Box::new(OverflowNode::default()))
}

/// `$isOverflowNode(node)`.
pub fn is_overflow_node(node: &NodeRef) -> bool {
    node.get_latest().is::<OverflowNode>()
}

/// The node classes of this package (`OverflowExtension`'s `nodes`):
/// `OverflowNode`.
pub fn overflow_nodes() -> Vec<NodeClass> {
    vec![NodeClass::of::<OverflowNode>()]
}

/// `OverflowExtension`.
#[cfg(feature = "extension")]
pub type OverflowExtensionType = crate::LexicalExtension;

#[cfg(feature = "extension")]
thread_local! {
    static OVERFLOW_EXTENSION: std::rc::Rc<OverflowExtensionType> =
        crate::define_extension(crate::LexicalExtension {
            nodes: overflow_nodes(),
            ..crate::LexicalExtension::new("@lexical/overflow", ())
        });
}

/// `OverflowExtension`: registers `OverflowNode`, whose transform comes
/// with the class.
#[cfg(feature = "extension")]
pub fn overflow_extension() -> std::rc::Rc<OverflowExtensionType> {
    OVERFLOW_EXTENSION.with(std::rc::Rc::clone)
}

impl LexicalNode for OverflowNode {
    fn get_type(&self) -> &'static str {
        "overflow"
    }
    /// `OverflowNode.createDOM(config)`: a `<span>` whose class is the
    /// theme's `characterLimit`.
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        config: &crate::EditorConfig,
        _editor: &crate::Editor,
    ) -> Result<crate::dom::DomNode, crate::LexicalError> {
        let span = crate::lexical_dom::get_document().create_element("span");
        if let Some(class_name) = crate::lexical_dom::theme_string(&config.theme, "characterLimit")
        {
            span.set_class_name(class_name);
        }
        Ok(span)
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn element(&self) -> Option<&ElementData> {
        Some(&self.element)
    }
    fn element_mut(&mut self) -> Option<&mut ElementData> {
        Some(&mut self.element)
    }

    /// `OverflowNode.insertNewAfter(selection, restoreSelection)`: what its
    /// parent's does.
    fn insert_new_after(
        &self,
        this: &NodeRef,
        selection: &SelectionRef,
        restore_selection: bool,
    ) -> Result<Option<NodeRef>, LexicalError> {
        let parent = this.get_parent_or_throw()?;
        parent.insert_new_after(selection, restore_selection)
    }

    /// `OverflowNode.excludeFromCopy()`.
    fn exclude_from_copy(&self, _: &str) -> bool {
        true
    }

    /// The `$transform` of `OverflowNode.$config()`: an empty overflow node
    /// is removed.
    fn transform(&self, this: &NodeRef) -> Result<(), LexicalError> {
        if this.is_empty() {
            this.remove(false)?;
        }
        Ok(())
    }

    /// `exportElementNode` and `exportCompactElementNode`.
    fn generated_export_json(&self, compact: bool) -> Option<Object> {
        Some(if compact {
            export_compact_element_node(self)
        } else {
            export_element_node(self)
        })
    }

    /// `updateElementNode`.
    fn generated_update_from_json(&mut self, json: &Object) -> bool {
        update_element_node(&mut self.element, json);
        true
    }
}
