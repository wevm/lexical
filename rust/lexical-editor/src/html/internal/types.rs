//! The render half of `internal/types.ts`: what `DOMRenderExtension`'s
//! overrides are made of.
//!
//! JS matches an override to nodes by class (`instanceof`, so a subclass's
//! nodes too) or by a guard. Rust's nodes have no class hierarchy, so a
//! [`NodeMatch::Class`] holds the class's node type and the types of the
//! classes it extends ([`LexicalNode::super_types`]), and matches a node
//! whose type, or whose class's ancestors, include it.

use super::context_record::{
    ContextConfig, ContextPairOrUpdater, ContextRecord, get_context_value,
};
use crate::dom::DomNode;
use crate::lexical_dom::DomExportOutput;
use crate::lexical_selection::SelectionRef;
use crate::node::{LexicalNode, NodeRef};
use crate::{Editor, LexicalError};
use std::rc::Rc;

/// `RenderStateConfig<V>`: a typed key of the render context, made by
/// [`create_render_state`](crate::html::create_render_state).
pub type RenderStateConfig<V> = ContextConfig<V>;

/// `RenderContextReader`: reads render context values, for the
/// [`DomOverrideOptions`] gates.
pub struct RenderContextReader {
    pub(crate) record: Option<Rc<ContextRecord>>,
}

impl RenderContextReader {
    /// `get(cfg)`.
    pub fn get<V: Clone + PartialEq + 'static>(&self, cfg: &RenderStateConfig<V>) -> V {
        get_context_value(self.record.as_ref(), cfg)
    }
}

/// A guard: whether an override applies to a node value and its handle.
pub type NodePredicate = Rc<dyn Fn(&dyn LexicalNode, &NodeRef) -> bool>;

/// `NodeMatch`: the nodes an override applies to, of a class (and the
/// classes that extend it) or that a guard accepts.
#[derive(Clone)]
pub enum NodeMatch {
    /// A class, by its node type and those of the classes it extends,
    /// nearest first. Make one with [`NodeMatch::class`].
    Class {
        node_type: &'static str,
        super_types: Vec<&'static str>,
    },
    Predicate(NodePredicate),
}

impl NodeMatch {
    /// The class of `T`.
    pub fn class<T: LexicalNode + Default>() -> Self {
        let node = T::default();
        NodeMatch::Class {
            node_type: node.get_type(),
            super_types: node.super_types(),
        }
    }

    /// A guard.
    pub fn predicate(f: impl Fn(&dyn LexicalNode, &NodeRef) -> bool + 'static) -> Self {
        NodeMatch::Predicate(Rc::new(f))
    }
}

/// `node instanceof klass`, for the class with node type `node_type`.
pub(crate) fn is_instance(node: &dyn LexicalNode, node_type: &str) -> bool {
    node.get_type() == node_type || node.super_types().contains(&node_type)
}

/// A gate on an override, from the render context.
pub type RenderGate = Rc<dyn Fn(&RenderContextReader) -> bool>;

/// `DOMOverrideOptions`: whether an override is installed, from the render
/// context. `disabledForEditor` gates it in the editor's render config
/// (against the editor's context); `disabledForSession` gates it for one
/// export (against that export's context).
#[derive(Clone, Default)]
pub struct DomOverrideOptions {
    pub disabled_for_editor: Option<RenderGate>,
    pub disabled_for_session: Option<RenderGate>,
}

/// An override's `$next`: the behavior it overrides.
pub type RenderNext<'a, T> = &'a dyn Fn() -> Result<T, LexicalError>;

/// `$createDOM(node, $next, editor)`.
pub type CreateDomOverride = Rc<
    dyn Fn(
        &dyn LexicalNode,
        &NodeRef,
        RenderNext<'_, DomNode>,
        &Editor,
    ) -> Result<DomNode, LexicalError>,
>;

/// `$exportDOM(node, $next, editor)`.
pub type ExportDomOverride = Rc<
    dyn Fn(
        &dyn LexicalNode,
        &NodeRef,
        RenderNext<'_, DomExportOutput>,
        &Editor,
    ) -> Result<DomExportOutput, LexicalError>,
>;

/// `$extractWithChild(node, childNode, selection, destination, $next,
/// editor)`.
pub type ExtractWithChildOverride = Rc<
    dyn Fn(
        &NodeRef,
        &NodeRef,
        Option<&SelectionRef>,
        &str,
        RenderNext<'_, bool>,
        &Editor,
    ) -> Result<bool, LexicalError>,
>;

/// `$shouldInclude` and `$shouldExclude(node, selection, $next, editor)`.
pub type SelectionOverride = Rc<
    dyn Fn(
        &NodeRef,
        Option<&SelectionRef>,
        RenderNext<'_, bool>,
        &Editor,
    ) -> Result<bool, LexicalError>,
>;

/// `DOMRenderMatchConfig`: the hooks of an override, each given `$next`,
/// the behavior it overrides. The reconciler's hooks (`$updateDOM`,
/// `$decorateDOM`, `$getDOMSlot`, `$getSlotTargetElement`) have no use
/// without a DOM to reconcile, as in
/// [`EditorDomRenderConfig`](crate::lexical_dom::EditorDomRenderConfig).
#[derive(Clone, Default)]
pub struct DomRenderMatchConfig {
    pub create_dom: Option<CreateDomOverride>,
    pub export_dom: Option<ExportDomOverride>,
    pub extract_with_child: Option<ExtractWithChildOverride>,
    pub should_exclude: Option<SelectionOverride>,
    pub should_include: Option<SelectionOverride>,
}

/// `DOMRenderMatch`: an override, the nodes it applies to (`None` for all,
/// JS `'*'`) and its gates. Shared, as the runtime tells overrides apart
/// by identity, as JS does.
pub struct DomRenderMatch {
    pub nodes: Option<Vec<NodeMatch>>,
    pub config: DomRenderMatchConfig,
    pub options: DomOverrideOptions,
}

/// `DOMRenderConfig`: `DOMRenderExtension`'s config.
#[derive(Clone, Default)]
pub struct DomRenderConfig {
    /// The overrides. Contributions are appended; when they compile, class
    /// overrides come first (a class before the classes that extend it),
    /// then guards, then wildcards, each wrapping those before it.
    pub overrides: Vec<Rc<DomRenderMatch>>,
    /// The editor's render context. Contributions are appended.
    pub context_defaults: Vec<ContextPairOrUpdater>,
}

/// `Partial<DOMRenderConfig>`.
#[derive(Clone, Default)]
pub struct DomRenderConfigOverrides {
    pub overrides: Option<Vec<Rc<DomRenderMatch>>>,
    pub context_defaults: Option<Vec<ContextPairOrUpdater>>,
}
