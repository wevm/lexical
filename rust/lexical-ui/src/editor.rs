//! `LexicalEditor.ts`: creating an editor, its registered nodes, and the
//! registration of transforms, listeners and commands.

use crate::editor_state::{clone_editor_state, create_empty_editor_state};
use crate::error::invariant;
use crate::json::{self, Object, Value};
use crate::lexical_utils::{create_uid, set_node_key};
use crate::node::{LexicalNode, NodeKey, NodeRef};
use crate::node_state::{RequiredNodeStateConfig, SharedNodeState, create_shared_node_state};
use crate::nodes::{
    artificial::ArtificialNode, line_break::LineBreakNode, paragraph::ParagraphNode,
    root::RootNode, tab::TabNode, text::TextNode,
};
use crate::ordered::{OrderedMap, OrderedSet};
use crate::reconciler::Decorators;
use crate::selection_deletion::SelectionMovement;
use crate::structure::{
    DeclaredNodeStructure, NodeStructureRegistration, get_declared_node_structure,
    validate_node_structure,
};
use crate::updates::{
    begin_update, commit_pending_updates, parse_editor_state, read_editor_state,
    trigger_command_listeners, update_editor_sync,
};
use crate::{EditorState, LexicalError};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

/// A node class an editor registers: its type and how it imports JSON (the
/// static `importJSON`). As an editor's registration (`RegisteredNode`) it
/// also holds the class's node replacement, if any: what
/// [`LexicalNodeReplacement`] makes of the class.
#[derive(Clone)]
pub struct NodeClass {
    pub(crate) node_type: &'static str,
    /// `klass.name`, which messages name a class by: the Rust type's name
    /// (the node type for a class made with
    /// [`NodeClass::with_import_json`]).
    pub(crate) class_name: &'static str,
    pub(crate) import_json: fn(&Object) -> Result<NodeRef, LexicalError>,
    /// The `stateConfigs` of the class's `$config`.
    pub(crate) state_configs: fn() -> Vec<RequiredNodeStateConfig>,
    /// The class's own static `importDOM` (`None` when it inherits one).
    #[cfg(feature = "html")]
    pub(crate) import_dom: fn() -> Option<crate::lexical_dom::DomConversionMap>,
    /// Whether the class is an `ElementNode` with an `isInline` of its own
    /// (`klass.prototype.isInline !== ElementNode.prototype.isInline`,
    /// which `NormalizeInlineElementsExtension` asks). A trait method can't
    /// tell an override, so this asks a fresh node whether it is an inline
    /// element; every ported class answers the same for all its nodes.
    #[cfg_attr(not(feature = "extension"), allow(dead_code))]
    pub(crate) is_inline_element: fn() -> bool,
    /// The `slots` of the class's `$config()` (`getDeclaredSlots`).
    pub(crate) declared_slots: fn() -> &'static [&'static str],
    /// Whether the class is `TextNode` or a subclass of it
    /// (`klass.prototype instanceof TextNode`).
    #[cfg_attr(not(feature = "extension"), allow(dead_code))]
    pub(crate) is_text: fn() -> bool,
    /// Whether the class is `ElementNode` or `DecoratorNode`, or a subclass
    /// of either, whose nodes `INTERNAL_registerNodeStructure` marks dirty.
    pub(crate) is_element_or_decorator: fn() -> bool,
    /// `getDeclaredNodeStructure(klass)`.
    node_structure: fn() -> DeclaredNodeStructure,
    /// `validateNodeStructure(klass)`, which panics on a bad declaration.
    validate_node_structure: fn(),
    /// The types of the classes the class extends, nearest first
    /// ([`LexicalNode::super_types`]), which stand for `instanceof`.
    pub(crate) super_types: fn() -> Vec<&'static str>,
    /// `replace` and `replaceWithKlass`: the class's node replacement.
    pub(crate) replacement: Option<Rc<LexicalNodeReplacement>>,
    /// `new klass()`, which `$create` calls; `None` for a class made with
    /// [`NodeClass::with_import_json`].
    new_node: Option<fn() -> Box<dyn LexicalNode>>,
}

fn new_node_of<T: LexicalNode + Default>() -> Box<dyn LexicalNode> {
    Box::new(T::default())
}

/// A node replacement's `with`: given a new node of the replaced class
/// (with a key, in the active editor state), the node to use instead, which
/// must have a key of its own.
pub type NodeReplace = Rc<dyn Fn(&NodeRef) -> Result<NodeRef, LexicalError>>;

/// `LexicalNodeReplacement`: an entry of [`CreateEditorArgs::nodes`] (as a
/// [`NodeClass`], through `From`) that makes `$create` and the `$create*`
/// functions of the `replace` class give what `with` makes of their node.
/// `with_klass`, the class `with` makes (which must extend `replace` and be
/// registered too), lets transforms and mutation listeners registered for
/// `replace` reach its nodes.
#[derive(Clone)]
pub struct LexicalNodeReplacement {
    pub replace: NodeClass,
    pub with: NodeReplace,
    pub with_klass: Option<NodeClass>,
}

impl From<LexicalNodeReplacement> for NodeClass {
    fn from(replacement: LexicalNodeReplacement) -> Self {
        let mut class = replacement.replace.clone();
        class.replacement = Some(Rc::new(replacement));
        class
    }
}

fn is_element_or_decorator_of<T: LexicalNode + Default>() -> bool {
    let node = T::default();
    node.element().is_some() || node.is_decorator()
}

fn node_structure_of<T: LexicalNode + Default>() -> DeclaredNodeStructure {
    get_declared_node_structure(&T::default())
}

fn validate_node_structure_of<T: LexicalNode + Default>() {
    validate_node_structure(&T::default());
}

fn declared_slots_of<T: LexicalNode + Default>() -> &'static [&'static str] {
    T::default().declared_slots()
}

#[cfg(feature = "html")]
fn import_dom_of<T: LexicalNode + Default>() -> Option<crate::lexical_dom::DomConversionMap> {
    T::default().import_dom()
}

fn state_configs_of<T: LexicalNode + Default>() -> Vec<RequiredNodeStateConfig> {
    T::default().state_configs()
}

fn is_text_of<T: LexicalNode + Default>() -> bool {
    T::default().text().is_some()
}

fn super_types_of<T: LexicalNode + Default>() -> Vec<&'static str> {
    T::default().super_types()
}

fn is_inline_element_of<T: LexicalNode + Default>() -> bool {
    let node = T::default();
    node.element().is_some() && node.is_inline()
}

/// The `importJSON` of a class: `$create(klass).updateFromJSON(json)`, so
/// with the editor's replacement of the class.
fn import_default<T: LexicalNode + Default>(json: &Object) -> Result<NodeRef, LexicalError> {
    let node_type = T::default().get_type();
    let has_replacement = crate::updates::get_registry_editor().is_some_and(|editor| {
        editor
            .get_registered_node(node_type)
            .is_some_and(|class| class.replacement.is_some())
    });
    let node = if has_replacement {
        crate::lexical_utils::create::<T>()
    } else {
        set_node_key(Box::new(T::default()))
    };
    // `updateFromJSON`: the nested NodeState (`$updateStateFromJSON`), then
    // `$applyJSONSetters`, which applies the flat states before the class's
    // own properties.
    crate::node_state::update_state_from_json(&node, json)?;
    crate::node_state::apply_flat_states(&node, json)?;
    crate::lexical_utils::mutate(node.get_key(), |n| n.update_from_json(json));
    Ok(node)
}

impl NodeClass {
    /// The class of `T`, which imports a node as `T::default()` with the
    /// JSON applied.
    pub fn of<T: LexicalNode + Default>() -> Self {
        Self {
            node_type: T::default().get_type(),
            class_name: std::any::type_name::<T>()
                .rsplit("::")
                .next()
                .unwrap_or_default(),
            import_json: import_default::<T>,
            state_configs: state_configs_of::<T>,
            #[cfg(feature = "html")]
            import_dom: import_dom_of::<T>,
            is_inline_element: is_inline_element_of::<T>,
            declared_slots: declared_slots_of::<T>,
            is_text: is_text_of::<T>,
            is_element_or_decorator: is_element_or_decorator_of::<T>,
            node_structure: node_structure_of::<T>,
            validate_node_structure: validate_node_structure_of::<T>,
            super_types: super_types_of::<T>,
            replacement: None,
            new_node: Some(new_node_of::<T>),
        }
    }

    /// A class with its own `importJSON`, which declares no NodeState and
    /// is not an inline element.
    pub fn with_import_json(
        node_type: &'static str,
        import_json: fn(&Object) -> Result<NodeRef, LexicalError>,
    ) -> Self {
        Self {
            node_type,
            class_name: node_type,
            import_json,
            state_configs: Vec::new,
            #[cfg(feature = "html")]
            import_dom: || None,
            is_inline_element: || false,
            declared_slots: || &[],
            is_text: || false,
            is_element_or_decorator: || false,
            node_structure: DeclaredNodeStructure::default,
            validate_node_structure: || {},
            super_types: Vec::new,
            replacement: None,
            new_node: None,
        }
    }

    pub fn node_type(&self) -> &'static str {
        self.node_type
    }

    /// Whether the class's `importJSON` is the synthesized one
    /// ([`NodeClass::of`]), which applies the NodeState in the JSON, rather
    /// than one of its own ([`NodeClass::with_import_json`]).
    pub(crate) fn imports_node_state(&self) -> bool {
        self.new_node.is_some()
    }

    /// `new klass()`, with a key in the active editor state.
    pub(crate) fn construct(&self) -> Option<NodeRef> {
        self.new_node.map(|f| set_node_key(f()))
    }

    /// `replaceWithKlass`: the type of the class the replacement makes.
    pub(crate) fn replace_with_klass(&self) -> Option<&'static str> {
        self.replacement
            .as_ref()
            .and_then(|r| r.with_klass.as_ref())
            .map(|k| k.node_type)
    }

    /// `getDeclaredNodeStructure(klass)` (`@experimental node-structure`):
    /// the structure the class declares. A class made with
    /// [`with_import_json`](Self::with_import_json) declares none.
    pub fn get_declared_node_structure(&self) -> DeclaredNodeStructure {
        (self.node_structure)()
    }
}

/// `COLLABORATION_TAG` (`LexicalUpdateTags.ts`): the update applies changes
/// that came from another replica.
pub const COLLABORATION_TAG: &str = "collaboration";

/// `HISTORIC_TAG` (`LexicalUpdateTags.ts`): the update applies a state from
/// the undo or redo stack.
pub const HISTORIC_TAG: &str = "historic";

/// `HISTORY_PUSH_TAG` (`LexicalUpdateTags.ts`): the update starts a new
/// history entry.
pub const HISTORY_PUSH_TAG: &str = "history-push";

/// `HISTORY_MERGE_TAG` (`LexicalUpdateTags.ts`): the update joins the
/// previous history entry.
pub const HISTORY_MERGE_TAG: &str = "history-merge";

/// `PASTE_TAG` (`LexicalUpdateTags.ts`): the update is a paste.
pub const PASTE_TAG: &str = "paste";

/// `CUT_TAG` (`LexicalUpdateTags.ts`): the update is a cut.
pub const CUT_TAG: &str = "cut";

/// `COMPOSITION_START_TAG` (`LexicalUpdateTags.ts`): the update starts an
/// input method composition.
pub const COMPOSITION_START_TAG: &str = "composition-start";

/// `COMPOSITION_END_TAG` (`LexicalUpdateTags.ts`): the update ends an input
/// method composition.
pub const COMPOSITION_END_TAG: &str = "composition-end";

/// `DEFER_COMPOSITION_MERGE_TAG` (`LexicalUpdateTags.ts`): the update must
/// not merge text into or out of the node being composed.
pub const DEFER_COMPOSITION_MERGE_TAG: &str = "defer-composition-merge";

/// `SKIP_SELECTION_FOCUS_TAG` (`LexicalUpdateTags.ts`): the update must
/// not move the focus to the editor.
pub const SKIP_SELECTION_FOCUS_TAG: &str = "skip-selection-focus";

/// `SKIP_COLLAB_TAG` (`LexicalUpdateTags.ts`): the update must not be sent
/// to other replicas.
pub const SKIP_COLLAB_TAG: &str = "skip-collab";

/// `SKIP_SCROLL_INTO_VIEW_TAG` (`LexicalUpdateTags.ts`): the update must
/// not scroll the selection into view.
pub const SKIP_SCROLL_INTO_VIEW_TAG: &str = "skip-scroll-into-view";

/// `SKIP_DOM_SELECTION_TAG` (`LexicalUpdateTags.ts`): the update must not
/// change the platform's selection.
pub const SKIP_DOM_SELECTION_TAG: &str = "skip-dom-selection";

/// `FOCUS_TAG` (`LexicalUpdateTags.ts`): the update is
/// [`Editor::focus`]'s.
pub const FOCUS_TAG: &str = "focus";

/// `ErrorHandler` as `onWarn` takes it: it gets a warning the editor has
/// recovered from, and what it returns is what the call that warned
/// returns (JS's default throws in development, as the vectors record).
pub type WarningHandler = Rc<dyn Fn(LexicalError) -> Result<(), LexicalError>>;

/// `CreateEditorArgs`: the node classes (and [`LexicalNodeReplacement`]s,
/// as `NodeClass`es) to register besides the core ones, the editor's
/// `namespace` (when empty, its parent editor's, or a random one,
/// `createUID()`), the initial state, whether it is editable, the theme,
/// the parent editor, `onWarn`, and (with the `html` feature) the HTML
/// import and export overrides and the render config. JS's `onError` has
/// no counterpart: an update's error is returned from
/// [`Editor::update`].
#[derive(Clone, Default)]
pub struct CreateEditorArgs {
    pub nodes: Vec<NodeClass>,
    pub namespace: String,
    /// `editorState`: the state the editor starts with, committed by its
    /// first read or update.
    pub editor_state: Option<EditorState>,
    /// `editable`; `None` is JS's `undefined`, editable.
    pub editable: Option<bool>,
    /// `onWarn`: what hears of recoverable conditions (an update cascade
    /// cut short); by default the warning is returned as an error, as JS's
    /// default throws in development.
    pub on_warn: Option<WarningHandler>,
    /// `parentEditor`: the editor this one is nested in, which commands
    /// dispatched in this one reach after its own listeners.
    pub parent_editor: Option<Editor>,
    /// `theme`: class names by node kind (`EditorThemeClasses`), as JSON:
    /// a string or an array of strings, or (for `text`) an object of them
    /// by format.
    pub theme: Object,
    /// `html`: export overrides by node type and extra import conversions.
    #[cfg(feature = "html")]
    pub html: crate::lexical_dom::HtmlConfig,
    /// `dom`: the render config; `None` for the default.
    #[cfg(feature = "html")]
    pub dom: Option<crate::lexical_dom::EditorDomRenderConfig>,
}

/// `EditorConfig` (`editor._config`): what `createDOM` reads.
#[derive(Clone, Debug, Default)]
pub struct EditorConfig {
    /// `theme`.
    pub theme: Object,
}

/// `EditorReadMode`: which state [`Editor::read_with_mode`] reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorReadMode {
    /// `'force-commit'`: commit the pending update, then read.
    ForceCommit,
    /// `'latest'`: the committed state, without committing.
    Latest,
    /// `'pending'`: the pending state, or the committed one without one.
    Pending,
}

/// `EditorUpdateOptions`. `event` comes with its users.
#[derive(Clone, Default)]
pub struct UpdateOptions {
    /// Commit at the end of the update instead of at the next read.
    pub discrete: bool,
    /// Update tags (`tag`).
    pub tags: Vec<String>,
    pub skip_transforms: bool,
    /// `onUpdate`: runs once the update commits, after its listeners and
    /// before the callbacks of `$onUpdate` ([`on_update`](crate::on_update)).
    pub on_update: Option<Rc<dyn Fn()>>,
}

impl std::fmt::Debug for UpdateOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpdateOptions")
            .field("discrete", &self.discrete)
            .field("tags", &self.tags)
            .field("skip_transforms", &self.skip_transforms)
            .field("on_update", &self.on_update.is_some())
            .finish()
    }
}

/// `EditorFocusOptions`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EditorFocusOptions {
    /// `defaultSelection: 'rootEnd'` (the default, as `undefined` is).
    #[default]
    RootEnd,
    /// `defaultSelection: 'rootStart'`.
    RootStart,
}

/// What an update listener receives (`UpdateListenerPayload`). The dirty
/// nodes are in the order they were marked; each dirty element says whether
/// it was marked itself (`true`) or only because a descendant was.
#[derive(Clone, Debug)]
pub struct UpdateListenerPayload {
    pub dirty_elements: Vec<(NodeKey, bool)>,
    pub dirty_leaves: Vec<NodeKey>,
    pub normalized_nodes: Vec<NodeKey>,
    pub tags: Vec<String>,
    pub editor_state: EditorState,
    pub prev_editor_state: EditorState,
    /// `mutatedNodes`: what the reconciler did to the nodes, by node type,
    /// when it ran (the editor has a root element and something was
    /// dirty), or `None`. Empty unless a mutation listener is registered.
    pub mutated_nodes: Option<MutatedNodes>,
}

/// A node transform (`Transform<T>`).
pub type Transform = Rc<dyn Fn(&NodeRef) -> Result<(), LexicalError>>;

/// `NodeMutation`: what the reconciler did with a node in a commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeMutation {
    /// `'created'`.
    Created,
    /// `'updated'`.
    Updated,
    /// `'destroyed'`.
    Destroyed,
}

impl NodeMutation {
    /// The mutation as JS names it.
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeMutation::Created => "created",
            NodeMutation::Updated => "updated",
            NodeMutation::Destroyed => "destroyed",
        }
    }
}

/// The nodes of one class a commit mutated, in the order the reconciler
/// reached them (JS's `Map<NodeKey, NodeMutation>`).
pub type NodeMutations = Vec<(NodeKey, NodeMutation)>;

/// `MutatedNodes`: the nodes a commit mutated by node type, in the order
/// the reconciler first reached one of the type (JS keys its `Map` by
/// class).
pub type MutatedNodes = Vec<(&'static str, NodeMutations)>;

/// A mutation listener's second argument (`MutationListener`'s
/// `{dirtyLeaves, prevEditorState, updateTags}`).
#[derive(Clone, Debug)]
pub struct MutationListenerPayload {
    pub dirty_leaves: Vec<NodeKey>,
    pub prev_editor_state: EditorState,
    pub update_tags: Vec<String>,
}

/// `MutationListenerOptions`.
#[derive(Clone, Copy, Debug, Default)]
pub struct MutationListenerOptions {
    /// `skipInitialization`: don't call the listener at once with the
    /// class's existing nodes as created (false by default, as in JS).
    pub skip_initialization: bool,
}

/// A mutation listener.
pub(crate) type MutationListener = Rc<dyn Fn(&NodeMutations, &MutationListenerPayload)>;

/// The editor's root element: what JS's `HTMLElement` root is to an app
/// without a DOM, the view that shows the document. Setting one
/// ([`Editor::set_root_element`]) runs the reconciler's model half, so
/// mutation listeners hear from the editor. Root elements compare by
/// identity, as JS's elements do.
pub trait RootElement: Any {
    /// JS appends `@lexical/a11y`'s live region to `owner`, or to the root
    /// element's `ownerDocument.body`: this is where a root element puts it,
    /// or `None` (the default) for a root without one, where announcements
    /// are dropped.
    #[cfg(feature = "a11y")]
    fn live_region_host(self: Rc<Self>) -> Option<Rc<dyn crate::a11y::LiveRegionHost>> {
        None
    }

    /// Gives up the focus and clears the platform's selection, as
    /// [`Editor::blur`] (`editor.blur()`) does to a DOM root. Does nothing
    /// by default.
    fn blur(&self) {}

    /// Takes the focus, as `rootElement.focus()` does. Does nothing by
    /// default. ([`Editor::focus`] doesn't call it: JS's moves the focus by
    /// setting the DOM selection, which the app's view does when it draws
    /// the selection of an update tagged [`FOCUS_TAG`].)
    fn focus(&self) {}

    /// Whether the root (or a view in it) has the focus, as
    /// `rootElement.contains(document.activeElement)` says of a DOM root.
    /// `false` by default.
    fn has_focus(&self) -> bool {
        false
    }

    /// The caret at a point in the root's coordinates, as
    /// `caretFromPoint(x, y, rootElement)` from `@lexical/clipboard`
    /// hit-tests the DOM, which drop and drag handlers ask for the
    /// position of a `MouseEvent` (`clientX`, `clientY`). It is read
    /// inside an update. `None` (the default) when no caret resolves
    /// there.
    fn caret_from_point(&self, _x: f64, _y: f64) -> Option<crate::caret::PointCaret> {
        None
    }

    /// The computed style of the element rendered for the node `key`, as
    /// `getComputedStyle(editor.getElementByKey(key))`, which
    /// `@lexical/selection`'s `$getComputedStyleForElement` reads for the
    /// text direction and writing mode the arrow keys follow. It is read
    /// inside an update. `None` (the default) for a node with no element,
    /// as a JS editor that renders nothing has none.
    #[cfg(feature = "selection")]
    fn computed_style(&self, _key: &str) -> Option<crate::selection::ComputedStyle> {
        None
    }

    /// Whether the root is attached, as a DOM element's `isConnected` says
    /// (for a view: whether it is in a window). The commit announces a
    /// range selection (or none) with `SELECTION_CHANGE_COMMAND` only while
    /// the root is connected. `true` by default.
    fn is_connected(&self) -> bool {
        true
    }
}

/// A stored update listener, which may return a clean-up.
type UpdateListenerFn = dyn Fn(&UpdateListenerPayload) -> Option<Unregister>;
/// A stored text content listener, which may return a clean-up.
type TextContentListenerFn = dyn Fn(&str) -> Option<Unregister>;
/// A stored decorator listener, which may return a clean-up.
type DecoratorListenerFn = dyn Fn(&Decorators) -> Option<Unregister>;

/// A root element, compared by identity.
#[derive(Clone)]
pub struct RootElementRef(pub Rc<dyn RootElement>);

impl PartialEq for RootElementRef {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(Rc::as_ptr(&self.0), Rc::as_ptr(&other.0))
    }
}

impl std::fmt::Debug for RootElementRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RootElementRef({:p})", Rc::as_ptr(&self.0) as *const ())
    }
}

/// A root listener (`RootListener`): the next and the previous root
/// element; what it returns runs before its next call and when it is
/// unregistered.
pub(crate) type RootListener =
    Rc<dyn Fn(Option<&RootElementRef>, Option<&RootElementRef>) -> Option<Unregister>>;

/// A registered root listener: its id, the listener, and what its last call
/// returned.
type RootListenerEntry = (u64, RootListener, Rc<RefCell<Option<Unregister>>>);

/// Calling it removes what was registered, as the function JS's `register*`
/// methods return. Dropping it without calling it keeps the registration.
pub type Unregister = Box<dyn FnOnce()>;

/// What an update, text content, decorator or editable listener returns:
/// nothing (`()`), or a clean-up function (an [`Unregister`], or an
/// `Option` of one), which runs before the listener's next call, or when
/// it is unregistered. JS takes a returned function as the clean-up and
/// ignores any other value (`triggerListeners`).
pub trait ListenerCleanup {
    /// The clean-up, if there is one.
    fn into_cleanup(self) -> Option<Unregister>;
}

impl ListenerCleanup for () {
    fn into_cleanup(self) -> Option<Unregister> {
        None
    }
}

impl ListenerCleanup for Unregister {
    fn into_cleanup(self) -> Option<Unregister> {
        Some(self)
    }
}

impl ListenerCleanup for Option<Unregister> {
    fn into_cleanup(self) -> Option<Unregister> {
        self
    }
}

/// A command (`createCommand`), compared by identity: two commands made
/// with the same name are different commands.
pub struct LexicalCommand<P> {
    id: u64,
    name: CommandName,
    _payload: PhantomData<fn(&P)>,
}

/// A command's name: shared when [`create_command`] makes it, static for
/// the core's own commands (`commands`), which are constants.
#[derive(Clone)]
enum CommandName {
    Static(&'static str),
    Shared(Rc<str>),
}

impl<P> Clone for LexicalCommand<P> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            name: self.name.clone(),
            _payload: PhantomData,
        }
    }
}

impl<P> LexicalCommand<P> {
    /// One of the core's own commands, with an id [`create_command`] never
    /// gives (they count up from 1).
    pub(crate) const fn constant(id: u64, name: &'static str) -> Self {
        Self {
            id,
            name: CommandName::Static(name),
            _payload: PhantomData,
        }
    }

    pub fn name(&self) -> &str {
        match &self.name {
            CommandName::Static(name) => name,
            CommandName::Shared(name) => name,
        }
    }
    pub(crate) fn id(&self) -> u64 {
        self.id
    }
}

thread_local! {
    static NEXT_COMMAND_ID: Cell<u64> = const { Cell::new(1) };
}

/// `createCommand(type)`.
pub fn create_command<P>(name: &str) -> LexicalCommand<P> {
    let id = NEXT_COMMAND_ID.with(|c| c.replace(c.get() + 1));
    LexicalCommand {
        id,
        name: CommandName::Shared(name.into()),
        _payload: PhantomData,
    }
}

/// `COMMAND_PRIORITY_EDITOR`: a listener added to the end of the editor
/// priority queue (after critical, high, normal, low).
pub const COMMAND_PRIORITY_EDITOR: i8 = 0;
/// `COMMAND_PRIORITY_LOW`: the end of the low priority queue.
pub const COMMAND_PRIORITY_LOW: i8 = 1;
/// `COMMAND_PRIORITY_NORMAL`: the end of the normal priority queue.
pub const COMMAND_PRIORITY_NORMAL: i8 = 2;
/// `COMMAND_PRIORITY_HIGH`: the end of the high priority queue.
pub const COMMAND_PRIORITY_HIGH: i8 = 3;
/// `COMMAND_PRIORITY_CRITICAL`: the end of the critical priority queue.
pub const COMMAND_PRIORITY_CRITICAL: i8 = 4;
/// `COMMAND_PRIORITY_BEFORE_EDITOR`: the beginning of the editor priority
/// queue.
pub const COMMAND_PRIORITY_BEFORE_EDITOR: i8 = -8;
/// `COMMAND_PRIORITY_BEFORE_LOW`: the beginning of the low priority queue.
pub const COMMAND_PRIORITY_BEFORE_LOW: i8 = -7;
/// `COMMAND_PRIORITY_BEFORE_NORMAL`: the beginning of the normal priority
/// queue.
pub const COMMAND_PRIORITY_BEFORE_NORMAL: i8 = -6;
/// `COMMAND_PRIORITY_BEFORE_HIGH`: the beginning of the high priority
/// queue.
pub const COMMAND_PRIORITY_BEFORE_HIGH: i8 = -5;
/// `COMMAND_PRIORITY_BEFORE_CRITICAL`: the beginning of the critical
/// priority queue.
pub const COMMAND_PRIORITY_BEFORE_CRITICAL: i8 = -4;

/// A command listener: it returns whether it handled the command, which
/// stops the ones after it.
pub(crate) type CommandListener<P> = Rc<dyn Fn(&P, &Editor) -> Result<bool, LexicalError>>;

/// `NO_DIRTY_NODES`, `HAS_DIRTY_NODES`, `FULL_RECONCILE`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DirtyType {
    #[default]
    None,
    HasDirtyNodes,
    FullReconcile,
}

/// The editor's record of what an update changed: `_dirtyType`,
/// `_dirtyElements`, `_dirtyLeaves` and `_normalizedNodes`.
#[derive(Default)]
pub(crate) struct Dirty {
    pub(crate) dirty_type: DirtyType,
    pub(crate) elements: OrderedMap<bool>,
    pub(crate) leaves: OrderedSet,
    pub(crate) normalized: OrderedSet,
}

/// An update queued while the editor was updating (`_updates`).
pub(crate) struct QueuedUpdate {
    pub(crate) id: u64,
    pub(crate) update_fn: Box<dyn FnOnce() -> Result<(), LexicalError>>,
    pub(crate) options: UpdateOptions,
}

/// A Lexical editor. A handle; cloning it shares the same editor. Without a
/// DOM it is what JS calls a headless editor. It is neither `Send` nor
/// `Sync`.
#[derive(Clone)]
pub struct Editor(pub(crate) Rc<EditorInner>);

/// Editors compare by identity, as JS compares them with `===`.
impl PartialEq for Editor {
    fn eq(&self, other: &Editor) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Editor {}

/// A listener map (`ListenerMap`): each listener with its id and what its
/// last call returned.
type Listeners<F> = RefCell<Vec<(u64, Rc<F>, Rc<RefCell<Option<Unregister>>>)>>;
/// A command's listeners by priority, each a `CommandListener<P>` of the
/// command's payload type.
type CommandListeners = [Vec<(u64, Rc<dyn Any>)>; 5];

pub(crate) struct EditorInner {
    nodes: HashMap<&'static str, NodeClass>,
    /// The types of `nodes` in the order JS's `_nodes` map holds them: the
    /// core nodes, then `args.nodes`, each type where it first came.
    node_order: Vec<&'static str>,
    /// Each registered node's `sharedNodeState`.
    shared_node_states: HashMap<&'static str, Rc<SharedNodeState>>,
    pub(crate) editor_state: RefCell<EditorState>,
    pub(crate) pending: RefCell<Option<EditorState>>,
    pub(crate) dirty: RefCell<Dirty>,
    pub(crate) tags: RefCell<OrderedSet>,
    pub(crate) updating: Cell<bool>,
    pub(crate) dispatching_commit_listeners: Cell<bool>,
    pub(crate) updates: RefCell<VecDeque<QueuedUpdate>>,
    pub(crate) cascade_count: Cell<u32>,
    transforms: RefCell<HashMap<&'static str, Vec<(u64, Transform)>>>,
    pub(crate) update_listeners: Listeners<UpdateListenerFn>,
    text_content_listeners: Listeners<TextContentListenerFn>,
    commands: RefCell<HashMap<u64, CommandListeners>>,
    next_id: Cell<u64>,
    /// Stands in for the DOM's `Selection.modify` (no JS field).
    selection_movement: RefCell<Option<Rc<dyn SelectionMovement>>>,
    /// `_config`.
    config: EditorConfig,
    /// `_htmlConversions`: the import conversions by lower-case DOM node
    /// name, in registration order.
    #[cfg(feature = "html")]
    pub(crate) html_conversions: HashMap<String, Vec<crate::lexical_dom::DomImporter>>,
    /// The `exportDOM` overrides of `html.export`, by node type.
    #[cfg(feature = "html")]
    pub(crate) html_export: HashMap<&'static str, crate::lexical_dom::ExportOverride>,
    /// `_config.dom`, which `DOMRenderExtension` replaces when its overrides
    /// change (here, not in [`EditorConfig`], which nodes borrow).
    #[cfg(feature = "html")]
    pub(crate) dom: RefCell<crate::lexical_dom::EditorDomRenderConfig>,
    /// `_config.namespace`.
    namespace: String,
    /// `_parentEditor`.
    parent_editor: Option<Editor>,
    /// `_key`.
    key: String,
    /// `_editable`.
    editable: Cell<bool>,
    /// `_headless`: the editor never reconciles (`@lexical/headless`, or a
    /// headless parent editor).
    headless: Cell<bool>,
    /// What `@lexical/headless` puts before `setRootElement` (where JS
    /// replaces the method on the instance): it refuses a root.
    pub(crate) set_root_element_guard: RefCell<Option<SetRootElementGuard>>,
    /// `_onWarn`.
    on_warn: WarningHandler,
    /// `_slotsUsed`: whether a slot was ever set in this editor, or in a
    /// state it was given. Latched for the editor's lifetime.
    pub(crate) slots_used: Cell<bool>,
    /// `_rootElement`.
    root_element: RefCell<Option<RootElementRef>>,
    /// `INTERNAL_registerNodeStructure`'s registration (JS keeps it in a
    /// module-level `WeakMap` by editor).
    pub(crate) node_structure: RefCell<Option<Rc<NodeStructureRegistration>>>,
    /// `_listeners.mutation`: each listener with its class, in the order
    /// they were registered.
    mutation_listeners: RefCell<Vec<(u64, MutationListener, &'static str)>>,
    /// `_listeners.editable`.
    pub(crate) editable_listeners: Listeners<dyn Fn(bool) -> Option<Unregister>>,
    /// `_listeners.root`, each with what its last call returned.
    pub(crate) root_listeners: RefCell<Vec<RootListenerEntry>>,
    /// `_deferred`: the `$onUpdate` callbacks of the pending update.
    pub(crate) deferred: RefCell<Vec<Box<dyn FnOnce()>>>,
    /// `_compositionKey`: the node an input method is composing in.
    pub(crate) composition_key: RefCell<Option<NodeKey>>,
    /// `_compositionMergeHeld`: the composed node a merge was held back
    /// for (`DEFER_COMPOSITION_MERGE_TAG`).
    pub(crate) composition_merge_held: RefCell<Option<NodeKey>>,
    /// `_lastNotifiedSelection`: the selection `SELECTION_CHANGE_COMMAND`
    /// last announced (a clone), which the commit compares the pending
    /// selection with.
    pub(crate) last_notified_selection: RefCell<Option<crate::lexical_selection::Selection>>,
    /// `_inputState`, the part the model reads.
    pub(crate) input_state: RefCell<crate::events::InputState>,
    /// `_keyDownShortcuts`: the compiled key-down bindings.
    pub(crate) key_down_shortcuts: RefCell<Option<crate::events::KeyDownShortcuts>>,
    /// `RootNode.__cachedText`: the root the reconciler last cached its
    /// text content on, and that text. JS keeps it on the root node, which
    /// a clone resets; here the cache names the root by identity.
    pub(crate) root_text_cache: RefCell<Option<(Rc<dyn LexicalNode>, String)>>,
    /// `_keyToDOMMap`: what the reconciler keeps of each node it rendered.
    pub(crate) key_to_dom_map: RefCell<crate::reconciler::KeyToDomMap>,
    /// `_decorators`.
    pub(crate) decorators: RefCell<Decorators>,
    /// `_pendingDecorators`.
    pub(crate) pending_decorators: RefCell<Option<Decorators>>,
    /// `_listeners.decorator`.
    decorator_listeners: Listeners<DecoratorListenerFn>,
    /// The `LexicalBuilder` that built the editor (JS's
    /// `editor[builderSymbol]`).
    #[cfg(feature = "extension")]
    pub(crate) builder: RefCell<Option<Rc<dyn Any>>>,
}

/// What refuses a root element in [`Editor::set_root_element`], before it
/// changes anything.
pub(crate) type SetRootElementGuard =
    Rc<dyn Fn(Option<&RootElementRef>) -> Result<(), LexicalError>>;

/// `createEditor(config)`: registers the core nodes (root, text, line
/// break, tab, paragraph, artificial) and `args.nodes`, in `args.namespace`,
/// or the parent editor's when that is empty. (JS's `createEditor()` with
/// no config at all takes the active editor as the parent; Rust always
/// takes a config, as Swift does.)
///
/// A node replacement whose `with_klass` doesn't extend the class it
/// replaces panics, as JS's development build throws; one without
/// `with_klass` warns ([`with_warning_handler`](crate::with_warning_handler)).
pub fn create_editor(args: CreateEditorArgs) -> Editor {
    let parent_editor = args.parent_editor;
    // `config.namespace || (parentEditor !== null ?
    // parentEditor._config.namespace : createUID())`.
    let namespace = if !args.namespace.is_empty() {
        args.namespace
    } else if let Some(parent) = &parent_editor {
        parent.get_namespace().to_owned()
    } else {
        create_uid()
    };
    let core = [
        // The root is an `ElementNode`.
        NodeClass {
            is_element_or_decorator: || true,
            ..NodeClass::with_import_json("root", RootNode::import_json)
        },
        NodeClass::of::<TextNode>(),
        NodeClass::of::<LineBreakNode>(),
        NodeClass::of::<TabNode>(),
        NodeClass::of::<ParagraphNode>(),
        NodeClass::of::<ArtificialNode>(),
    ];
    let classes: Vec<NodeClass> = core.into_iter().chain(args.nodes).collect();
    // `validateNodeStructure(klass)`, which JS runs in development, as the
    // vectors were recorded.
    for class in &classes {
        if let Some(replacement) = &class.replacement {
            match &replacement.with_klass {
                Some(with_klass) => assert!(
                    // `replaceWithKlass.prototype instanceof klass`.
                    (with_klass.super_types)().contains(&class.node_type),
                    "{} doesn't extend the {}",
                    with_klass.class_name,
                    class.class_name
                ),
                None => crate::warnings::console_warn(&format!(
                    "Override for {} specifies 'replace' without 'withKlass'. 'withKlass' will be required in a future version.",
                    class.class_name
                )),
            }
        }
        (class.validate_node_structure)();
    }
    let mut node_order: Vec<&'static str> = Vec::new();
    for class in &classes {
        if !node_order.contains(&class.node_type) {
            node_order.push(class.node_type);
        }
    }
    #[cfg(feature = "html")]
    let html_conversions =
        crate::lexical_dom::initialize_conversion_cache(&classes, &args.html.import);
    let nodes: HashMap<&'static str, NodeClass> = classes
        .into_iter()
        .map(|class| (class.node_type, class))
        .collect();
    let headless = parent_editor.as_ref().is_some_and(|p| p.0.headless.get());
    let on_warn = args.on_warn.unwrap_or_else(|| Rc::new(Err));
    let shared_node_states = nodes
        .iter()
        .map(|(&node_type, class)| {
            let shared = create_shared_node_state((class.state_configs)());
            (node_type, Rc::new(shared))
        })
        .collect();
    let editor = Editor(Rc::new(EditorInner {
        nodes,
        node_order,
        shared_node_states,
        editor_state: RefCell::new(create_empty_editor_state()),
        pending: RefCell::new(None),
        dirty: RefCell::new(Dirty::default()),
        tags: RefCell::new(OrderedSet::new()),
        updating: Cell::new(false),
        dispatching_commit_listeners: Cell::new(false),
        updates: RefCell::new(VecDeque::new()),
        cascade_count: Cell::new(0),
        transforms: RefCell::new(HashMap::new()),
        update_listeners: RefCell::new(Vec::new()),
        text_content_listeners: RefCell::new(Vec::new()),
        commands: RefCell::new(HashMap::new()),
        next_id: Cell::new(1),
        selection_movement: RefCell::new(None),
        config: EditorConfig { theme: args.theme },
        #[cfg(feature = "html")]
        html_conversions,
        #[cfg(feature = "html")]
        html_export: args.html.export.into_iter().collect(),
        #[cfg(feature = "html")]
        dom: RefCell::new(args.dom.unwrap_or_default()),
        namespace,
        parent_editor,
        key: create_uid(),
        editable: Cell::new(args.editable.unwrap_or(true)),
        headless: Cell::new(headless),
        set_root_element_guard: RefCell::new(None),
        on_warn,
        slots_used: Cell::new(false),
        root_element: RefCell::new(None),
        node_structure: RefCell::new(None),
        mutation_listeners: RefCell::new(Vec::new()),
        editable_listeners: RefCell::new(Vec::new()),
        root_listeners: RefCell::new(Vec::new()),
        deferred: RefCell::new(Vec::new()),
        composition_key: RefCell::new(None),
        composition_merge_held: RefCell::new(None),
        last_notified_selection: RefCell::new(None),
        input_state: RefCell::new(crate::events::InputState::default()),
        key_down_shortcuts: RefCell::new(None),
        root_text_cache: RefCell::new(None),
        key_to_dom_map: RefCell::default(),
        decorators: RefCell::new(Decorators::default()),
        pending_decorators: RefCell::new(None),
        decorator_listeners: RefCell::new(Vec::new()),
        #[cfg(feature = "extension")]
        builder: RefCell::new(None),
    }));
    if let Some(initial_editor_state) = args.editor_state {
        *editor.0.pending.borrow_mut() = Some(initial_editor_state);
        editor.0.dirty.borrow_mut().dirty_type = DirtyType::FullReconcile;
    }
    let _unregister = crate::events::register_default_command_handlers(&editor);
    editor
}

impl Editor {
    /// `editor._config`.
    pub fn config(&self) -> &EditorConfig {
        &self.0.config
    }

    /// `LexicalEditor.version`: the port's own, as Swift's is
    /// `lexical-swift@<upstream>`, for the upstream release in
    /// [`UPSTREAM_VERSION`](crate::UPSTREAM_VERSION).
    pub const VERSION: &'static str = "lexical-rust@0.53.0";

    /// `isComposing()`: whether an input method is composing
    /// (`_compositionKey` is set).
    pub fn is_composing(&self) -> bool {
        self.0.composition_key.borrow().is_some()
    }

    /// `_compositionKey`.
    pub(crate) fn composition_key(&self) -> Option<NodeKey> {
        self.0.composition_key.borrow().clone()
    }

    /// `_config.namespace`: the namespace the editor was created with.
    pub fn get_namespace(&self) -> &str {
        &self.0.namespace
    }

    /// `_parentEditor`: the editor this one is nested in
    /// ([`CreateEditorArgs::parent_editor`]), if any.
    pub fn parent_editor(&self) -> Option<&Editor> {
        self.0.parent_editor.as_ref()
    }

    /// `getKey()`: the editor's own random key (`createUID()`), which a
    /// drag from it is marked with (`$writeDragSourceToDataTransfer`).
    pub fn get_key(&self) -> &str {
        &self.0.key
    }

    /// `blur()`: the root element gives up the focus and its selection
    /// ([`RootElement::blur`]).
    pub fn blur(&self) {
        if let Some(root) = self.get_root_element() {
            root.0.blur();
        }
    }

    /// `focus(callbackFn, options)`: with a root element, an update (tagged
    /// [`FOCUS_TAG`]) that marks the selection dirty, so the view applies
    /// it again, or selects the end (or, with
    /// [`EditorFocusOptions::RootStart`], the start) of the root when there
    /// is none; `callback` runs once it commits. Without a root element it
    /// does nothing. (JS also turns `autocapitalize` off on the DOM root
    /// around the update, which only iOS's keyboard reads.)
    pub fn focus(
        &self,
        callback: Option<Box<dyn FnOnce()>>,
        options: EditorFocusOptions,
    ) -> Result<(), LexicalError> {
        if self.get_root_element().is_none() {
            return Ok(());
        }
        update_editor_sync(
            self,
            move || {
                let selection = crate::get_selection();
                let root = crate::get_root();
                if let Some(selection) = selection {
                    // Marking the selection dirty will force the selection
                    // back to it.
                    if !selection.is_dirty() {
                        crate::set_selection(Some(selection.clone_selection()));
                    }
                } else if root.get_children_size() != 0 {
                    if options == EditorFocusOptions::RootStart {
                        root.select_start()?;
                    } else {
                        root.select_end()?;
                    }
                }
                add_update_tag(FOCUS_TAG);
                crate::on_update(move || {
                    if let Some(callback) = callback {
                        callback();
                    }
                });
                Ok(())
            },
            None,
        )
    }

    /// `_headless`: whether the editor never reconciles, as
    /// [`create_headless_editor`](crate::create_headless_editor) makes it
    /// (and a nested editor of a headless one is).
    pub fn is_headless(&self) -> bool {
        self.0.headless.get()
    }

    pub(crate) fn set_headless(&self, guard: SetRootElementGuard) {
        self.0.headless.set(true);
        *self.0.set_root_element_guard.borrow_mut() = Some(guard);
    }

    /// `_onWarn(error)`: what the call that warned returns.
    pub(crate) fn on_warn(&self, warning: LexicalError) -> Result<(), LexicalError> {
        (self.0.on_warn)(warning)
    }

    /// `isEditable()`.
    pub fn is_editable(&self) -> bool {
        self.0.editable.get()
    }

    /// `setEditable(editable)`: when that changes the editor's mode, the
    /// editable listeners hear of it.
    pub fn set_editable(&self, editable: bool) {
        if self.0.editable.get() != editable {
            self.0.editable.set(editable);
            with_updating(self, true, || {
                trigger_listeners(&self.0.editable_listeners, |listener| listener(editable));
            });
        }
    }

    /// `registerEditableListener(listener)`: `listener` hears each change of
    /// [`is_editable`](Self::is_editable).
    pub fn register_editable_listener<R: ListenerCleanup>(
        &self,
        listener: impl Fn(bool) -> R + 'static,
    ) -> Unregister {
        register_listener(
            self,
            |inner| &inner.editable_listeners,
            Rc::new(move |editable| listener(editable).into_cleanup())
                as Rc<dyn Fn(bool) -> Option<Unregister>>,
        )
    }

    /// `getDecorators()`: what each decorator decorated to
    /// ([`LexicalNode::decorate`]), as the reconciler last collected it.
    /// It is empty until the editor has a root element.
    pub fn get_decorators(&self) -> Decorators {
        self.0.decorators.borrow().clone()
    }

    /// `registerDecoratorListener(listener)`: `listener` hears each commit
    /// that changed [`get_decorators`](Self::get_decorators), with the new
    /// decorators.
    pub fn register_decorator_listener<R: ListenerCleanup>(
        &self,
        listener: impl Fn(&Decorators) -> R + 'static,
    ) -> Unregister {
        register_listener(
            self,
            |inner| &inner.decorator_listeners,
            Rc::new(move |decorators: &Decorators| listener(decorators).into_cleanup())
                as Rc<DecoratorListenerFn>,
        )
    }

    /// `triggerListeners('decorator', ...)`'s loop.
    pub(crate) fn trigger_decorator_listeners(&self, decorators: &Decorators) {
        trigger_listeners(&self.0.decorator_listeners, |listener| listener(decorators));
    }

    /// `getRootElement()`.
    pub fn get_root_element(&self) -> Option<RootElementRef> {
        self.0.root_element.borrow().clone()
    }

    /// `setRootElement(nextRootElement)`, its model half: the editor is
    /// reset (its state is empty again) and the pending state, or the
    /// current one, is committed with `history-merge`, the queued updates
    /// and tags kept. With a root, every node is reconciled, so mutation
    /// listeners hear of each as created; then the root listeners run.
    /// Styling and events are the app's view's job.
    pub fn set_root_element(
        &self,
        next_root_element: Option<RootElementRef>,
    ) -> Result<(), LexicalError> {
        let guard = self.0.set_root_element_guard.borrow().clone();
        if let Some(guard) = guard {
            guard(next_root_element.as_ref())?;
        }
        let prev_root_element = self.get_root_element();
        if next_root_element == prev_root_element {
            return Ok(());
        }
        let pending_editor_state = self.pending().unwrap_or_else(|| self.get_editor_state());
        *self.0.root_element.borrow_mut() = next_root_element.clone();
        // `resetEditor(this, prev, next, pendingEditorState, {preserveUpdateQueue: true})`.
        *self.0.editor_state.borrow_mut() = create_empty_editor_state();
        *self.0.pending.borrow_mut() = Some(pending_editor_state);
        *self.0.composition_key.borrow_mut() = None;
        *self.0.dirty.borrow_mut() = Dirty::default();
        crate::reconciler::reset_key_to_dom_map(self, next_root_element.is_some());
        if next_root_element.is_some() {
            self.0.dirty.borrow_mut().dirty_type = DirtyType::FullReconcile;
        }
        self.add_tags(&[HISTORY_MERGE_TAG.to_owned()]);
        let committed = commit_pending_updates(self);
        // `triggerListeners('root', this, false, next, prev)`.
        let listeners: Vec<_> = self
            .0
            .root_listeners
            .borrow()
            .iter()
            .map(|(_, l, cleanup)| (l.clone(), cleanup.clone()))
            .collect();
        with_updating(self, false, || {
            for (listener, cleanup) in listeners {
                trigger_root_listener(
                    &listener,
                    &cleanup,
                    next_root_element.as_ref(),
                    prev_root_element.as_ref(),
                );
            }
        });
        committed
    }

    /// `registerRootListener(listener)`: `listener` is called at once with
    /// the root element (and `None`), then with the next and previous root
    /// each time it changes, and with `None` and the root when it is
    /// unregistered. What it returns runs before its next call.
    pub fn register_root_listener(
        &self,
        listener: impl Fn(Option<&RootElementRef>, Option<&RootElementRef>) -> Option<Unregister>
        + 'static,
    ) -> Unregister {
        let id = self.next_id();
        let listener: RootListener = Rc::new(listener);
        let root = self.get_root_element();
        let cleanup = Rc::new(RefCell::new(listener(root.as_ref(), None)));
        self.0
            .root_listeners
            .borrow_mut()
            .push((id, listener.clone(), cleanup.clone()));
        let weak = self.weak();
        Box::new(move || {
            let Some(inner) = weak.upgrade() else { return };
            let root = inner.root_element.borrow().clone();
            trigger_root_listener(&listener, &cleanup, None, root.as_ref());
            inner
                .root_listeners
                .borrow_mut()
                .retain(|(i, _, _)| *i != id);
            let last = cleanup.borrow_mut().take();
            if let Some(last) = last {
                last();
            }
        })
    }

    /// `registerMutationListener(klass, listener, options)`, by the node
    /// type: after each commit that the reconciler ran in (the editor has a
    /// root element), `listener` gets the class's nodes it created,
    /// updated or destroyed. Unless `skip_initialization`, it is called at
    /// once with the class's nodes in the editor's state as created. Fails
    /// for a type the editor doesn't register.
    pub fn register_mutation_listener(
        &self,
        node_type: &str,
        listener: impl Fn(&NodeMutations, &MutationListenerPayload) + 'static,
        options: MutationListenerOptions,
    ) -> Result<Unregister, LexicalError> {
        let Some((&node_type, _)) = self.0.nodes.get_key_value(node_type) else {
            return Err(LexicalError::new(format!(
                "Node {node_type} has not been registered. Ensure node has been passed to createEditor."
            )));
        };
        // `resolveRegisteredNodeAfterReplacements`: a replaced class's
        // listener hears of the class that replaces it.
        let node_type = self.resolve_registered_node_after_replacements(node_type)?;
        let id = self.next_id();
        let listener: MutationListener = Rc::new(listener);
        self.0
            .mutation_listeners
            .borrow_mut()
            .push((id, listener.clone(), node_type));
        if !options.skip_initialization {
            // `initializeMutationListener(listener, klass)`.
            let prev_editor_state = self.get_editor_state();
            let keys: NodeMutations = {
                let node_map = prev_editor_state.0.node_map.borrow();
                prev_editor_state
                    .keys_in_creation_order()
                    .into_iter()
                    .filter(|key| node_map[key].get_type() == node_type)
                    .map(|key| (key, NodeMutation::Created))
                    .collect()
            };
            if !keys.is_empty() {
                listener(
                    &keys,
                    &MutationListenerPayload {
                        dirty_leaves: Vec::new(),
                        prev_editor_state,
                        update_tags: vec!["registerMutationListener".to_owned()],
                    },
                );
            }
        }
        let weak = self.weak();
        Ok(Box::new(move || {
            if let Some(inner) = weak.upgrade() {
                inner
                    .mutation_listeners
                    .borrow_mut()
                    .retain(|(i, _, _)| *i != id);
            }
        }))
    }

    /// The node types mutation listeners listen to.
    pub(crate) fn has_mutation_listeners(&self) -> bool {
        !self.0.mutation_listeners.borrow().is_empty()
    }

    pub(crate) fn mutation_listeners(&self) -> Vec<(MutationListener, &'static str)> {
        self.0
            .mutation_listeners
            .borrow()
            .iter()
            .map(|(_, l, t)| (l.clone(), *t))
            .collect()
    }

    pub(crate) fn registered_nodes(&self) -> &HashMap<&'static str, NodeClass> {
        &self.0.nodes
    }

    /// `editor._nodes.get(type)`: the class the editor registers under the
    /// type.
    pub fn get_registered_node(&self, node_type: &str) -> Option<NodeClass> {
        self.0.nodes.get(node_type).cloned()
    }

    /// `resolveRegisteredNodeAfterReplacements(registeredNode)`, by type:
    /// the type of the class a registered class's replacements end at
    /// (following `withKlass`). Fails for a `withKlass` that isn't
    /// registered.
    pub fn resolve_registered_node_after_replacements(
        &self,
        node_type: &str,
    ) -> Result<&'static str, LexicalError> {
        let Some((mut node_type, mut class)) = self.0.nodes.get_key_value(node_type) else {
            return Err(LexicalError::new(format!(
                "Node {node_type} has not been registered. Ensure node has been passed to createEditor."
            )));
        };
        while let Some(next) = class.replace_with_klass() {
            let Some((t, c)) = self.0.nodes.get_key_value(next) else {
                return Err(LexicalError::new(format!(
                    "Node {next} has not been registered. Ensure node has been passed to createEditor."
                )));
            };
            (node_type, class) = (t, c);
        }
        Ok(node_type)
    }

    /// `hasNode(klass)`, by the node type: whether the editor registers it.
    pub fn has_node(&self, node_type: &str) -> bool {
        self.0.nodes.contains_key(node_type)
    }

    /// `hasNodes(klasses)`, by the node types: whether the editor registers
    /// all of them.
    pub fn has_nodes(&self, node_types: &[&str]) -> bool {
        node_types.iter().all(|t| self.has_node(t))
    }

    /// The registered node classes in the order JS's `_nodes` holds them.
    pub(crate) fn registered_nodes_in_order(&self) -> Vec<NodeClass> {
        self.0
            .node_order
            .iter()
            .map(|node_type| self.0.nodes[node_type].clone())
            .collect()
    }

    /// The `sharedNodeState` of a registered node type.
    pub(crate) fn shared_node_state(&self, node_type: &str) -> Option<Rc<SharedNodeState>> {
        self.0.shared_node_states.get(node_type).cloned()
    }

    pub(crate) fn ptr_eq(&self, other: &Editor) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn next_id(&self) -> u64 {
        self.0.next_id.replace(self.0.next_id.get() + 1)
    }

    pub(crate) fn weak(&self) -> Weak<EditorInner> {
        Rc::downgrade(&self.0)
    }

    /// Sets what stands in for the DOM's `Selection.modify` in this editor:
    /// where a caret lands when `modify`, `deleteCharacter`, `deleteWord` or
    /// `deleteLine` move it within text, which only text layout knows.
    /// Without one (the default) the editor behaves as JS does with no root
    /// element: those moves fail with `window object not found`.
    pub fn set_selection_movement(&self, movement: Option<Rc<dyn SelectionMovement>>) {
        *self.0.selection_movement.borrow_mut() = movement;
    }

    /// The movement [`set_selection_movement`](Self::set_selection_movement)
    /// set.
    pub(crate) fn selection_movement(&self) -> Option<Rc<dyn SelectionMovement>> {
        self.0.selection_movement.borrow().clone()
    }

    /// `getEditorState()`: the committed state.
    pub fn get_editor_state(&self) -> EditorState {
        self.0.editor_state.borrow().clone()
    }

    pub(crate) fn pending(&self) -> Option<EditorState> {
        self.0.pending.borrow().clone()
    }

    /// `toJSON()`: `{editorState}`, this editor's committed state in the
    /// form the export around it is writing
    /// ([`is_compact_export`](crate::is_compact_export)), which is how a
    /// nested editor stays in the form of the document containing it.
    pub fn to_json(&self) -> Value {
        let mut json = crate::json::Object::new();
        json.insert(
            "editorState",
            self.get_editor_state()
                .to_json(crate::serialized_export::is_compact_export()),
        );
        Value::Object(json)
    }

    /// `parseEditorState(json)` from the JSON text.
    pub fn parse_editor_state(&self, json: &str) -> Result<EditorState, LexicalError> {
        let value = json::parse(json).map_err(|e| LexicalError::new(e.to_string()))?;
        self.parse_editor_state_json(&value)
    }

    /// `parseEditorState(serializedEditorState)` from parsed JSON.
    pub fn parse_editor_state_json(&self, json: &Value) -> Result<EditorState, LexicalError> {
        parse_editor_state(json, self, None)
    }

    /// `parseEditorState(serializedEditorState, updateFn)`: runs `update_fn`
    /// on the parsed state before it is made read-only, as an update of
    /// that state (`$getRoot()` is its root); nothing it changes is marked
    /// dirty in this editor, nor in an update running here.
    pub fn parse_editor_state_json_with(
        &self,
        json: &Value,
        update_fn: impl FnOnce() -> Result<(), LexicalError>,
    ) -> Result<EditorState, LexicalError> {
        parse_editor_state(json, self, Some(Box::new(update_fn)))
    }

    /// `update(updateFn, options)`. Inside another update of this editor (or
    /// a listener of its commit) the update is queued and runs after it, as
    /// in JS, which is why `update_fn` is `'static`.
    ///
    /// A non-discrete update is committed by the next [`read`](Self::read)
    /// or discrete update, where JS commits it in a microtask. An update
    /// that fails is discarded, the editor keeps its previous state, and the
    /// error is returned (JS reports it to `onError`).
    pub fn update(
        &self,
        update_fn: impl FnOnce() -> Result<(), LexicalError> + 'static,
        options: UpdateOptions,
    ) -> Result<(), LexicalError> {
        if self.0.updating.get() {
            let id = self.next_id();
            self.0.updates.borrow_mut().push_back(QueuedUpdate {
                id,
                update_fn: Box::new(update_fn),
                options,
            });
            Ok(())
        } else {
            // `scheduleCascadeReset`: JS resets the cascade budget in a
            // macrotask, once control returns to the event loop. An update
            // begun outside any update or commit of this editor is the
            // host's next turn, so separate actions don't accumulate
            // toward the limit; a runaway cascade continues from commits.
            if !crate::updates::is_committing_pending_updates() {
                self.0.cascade_count.set(0);
            }
            begin_update(self, update_fn, &options)
        }
    }

    /// `setEditorState(editorState, {tag})`: makes `editor_state` the
    /// editor's state, through an update, so a state from
    /// [`parse_editor_state`](Self::parse_editor_state) has its text
    /// normalized and its transforms run, as in JS. The editor keeps its
    /// state and this returns the error when the update fails, such as for
    /// a state with nothing but the root (an invariant in JS's DEV build).
    pub fn set_editor_state(
        &self,
        editor_state: &EditorState,
        tag: Option<&str>,
    ) -> Result<(), LexicalError> {
        let is_empty_editor_state = editor_state.is_empty();
        // A writable state, so transforms can run on it.
        let writable_editor_state = if editor_state.0.read_only.get() {
            let writable = clone_editor_state(editor_state);
            *writable.0.selection.borrow_mut() =
                editor_state.selection().map(|s| s.clone_selection());
            writable
        } else {
            editor_state.clone()
        };
        let tags: Vec<String> = tag.map(str::to_owned).into_iter().collect();
        if let Some(pending) = self.pending()
            && !pending.is_empty()
        {
            self.add_tags(&tags);
            commit_pending_updates(self)?;
        }
        *self.0.pending.borrow_mut() = Some(writable_editor_state.clone());
        {
            let mut dirty = self.0.dirty.borrow_mut();
            dirty.dirty_type = DirtyType::FullReconcile;
            dirty.elements.set("root", false);
        }
        *self.0.composition_key.borrow_mut() = None;
        self.0
            .slots_used
            .set(self.0.slots_used.get() || editor_state.0.slots_used.get());
        let parsed = editor_state.0.parsed.get();
        let options = UpdateOptions {
            discrete: true,
            ..UpdateOptions::default()
        };
        let editor = self.clone();
        update_editor_sync(
            self,
            move || {
                editor.add_tags(&tags);
                invariant!(
                    !is_empty_editor_state,
                    "setEditorState: the editor state is empty. Ensure the editor state's root node never becomes empty."
                );
                if parsed {
                    // Every node is dirty, so normalization and transforms
                    // run on a state that arrived from outside.
                    let node_map = writable_editor_state.0.node_map.borrow();
                    let mut dirty = editor.0.dirty.borrow_mut();
                    for key in writable_editor_state.keys_in_creation_order() {
                        if node_map[&key].element().is_some() {
                            dirty.elements.set(&key, true);
                        } else {
                            dirty.leaves.add(&key);
                        }
                    }
                }
                Ok(())
            },
            (!self.0.updating.get()).then_some(&options),
        )
    }

    /// `read(callbackFn)`: commits any pending update, then runs `f` with
    /// the editor's state active, read-only. Fails when committing runs a
    /// queued update that fails.
    pub fn read<R>(&self, f: impl FnOnce() -> R) -> Result<R, LexicalError> {
        self.read_with_mode(EditorReadMode::ForceCommit, f)
    }

    /// `read(mode, callbackFn)`: [`read`](Self::read) for
    /// `EditorReadMode::ForceCommit`; `Latest` reads the committed state
    /// without committing first, and `Pending` the state of the update in
    /// progress or queued (the committed one without one), without
    /// committing it.
    pub fn read_with_mode<R>(
        &self,
        mode: EditorReadMode,
        f: impl FnOnce() -> R,
    ) -> Result<R, LexicalError> {
        if mode == EditorReadMode::ForceCommit {
            if !self.0.dispatching_commit_listeners.get() {
                commit_pending_updates(self)?;
            } else if self.pending().is_some() {
                crate::warnings::console_warn(
                    "editor.read() was called while the editor was notifying listeners of a commit, and a listener had started another update. That update was not committed early, so that listeners observe commits in order, and the read observed the state being committed. Inside listeners, read with editor.getEditorState().read(fn, {editor}) or editor.read('latest', fn) instead.",
                );
            }
        }
        let editor_state = match mode {
            EditorReadMode::Pending => self.pending().unwrap_or_else(|| self.get_editor_state()),
            _ => self.get_editor_state(),
        };
        Ok(read_editor_state(Some(self), &editor_state, f))
    }

    /// `registerNodeTransform(klass, listener)`, by the node type. Nodes of
    /// that type already in the editor are marked dirty, so the transform
    /// runs on them with the next update. A class replaced by a
    /// `with_klass` ([`LexicalNodeReplacement`]) gets it on that class too.
    pub fn register_node_transform(
        &self,
        node_type: &'static str,
        transform: impl Fn(&NodeRef) -> Result<(), LexicalError> + 'static,
    ) -> Unregister {
        let transform: Transform = Rc::new(transform);
        let mut types = vec![node_type];
        if let Some(with_klass) = self
            .0
            .nodes
            .get(node_type)
            .and_then(NodeClass::replace_with_klass)
            && self.0.nodes.contains_key(with_klass)
        {
            types.push(with_klass);
        }
        let unregisters: Vec<Unregister> = types
            .iter()
            .map(|&t| {
                let transform = transform.clone();
                self.add_node_transform(t, move |node| transform(node))
            })
            .collect();
        mark_nodes_with_types_as_dirty(self, &types);
        Box::new(move || {
            for unregister in unregisters {
                unregister();
            }
        })
    }

    /// `registeredNode.transforms.add(transform)`: what
    /// [`register_node_transform`](Self::register_node_transform) does
    /// before marking the type's nodes dirty, for callers that add to a
    /// class's transforms directly, as `NormalizeInlineElementsExtension`
    /// does.
    pub(crate) fn add_node_transform(
        &self,
        node_type: &'static str,
        transform: impl Fn(&NodeRef) -> Result<(), LexicalError> + 'static,
    ) -> Unregister {
        let id = self.next_id();
        self.0
            .transforms
            .borrow_mut()
            .entry(node_type)
            .or_default()
            .push((id, Rc::new(transform)));
        let weak = self.weak();
        Box::new(move || {
            if let Some(inner) = weak.upgrade()
                && let Some(list) = inner.transforms.borrow_mut().get_mut(node_type)
            {
                list.retain(|(i, _)| *i != id);
            }
        })
    }

    pub(crate) fn transforms_of(&self, node_type: &str) -> Vec<Transform> {
        self.0
            .transforms
            .borrow()
            .get(node_type)
            .map(|list| list.iter().map(|(_, t)| t.clone()).collect())
            .unwrap_or_default()
    }

    /// `registerUpdateListener(listener)`.
    pub fn register_update_listener<R: ListenerCleanup>(
        &self,
        listener: impl Fn(&UpdateListenerPayload) -> R + 'static,
    ) -> Unregister {
        register_listener(
            self,
            |inner| &inner.update_listeners,
            Rc::new(move |payload: &UpdateListenerPayload| listener(payload).into_cleanup())
                as Rc<UpdateListenerFn>,
        )
    }

    /// `registerTextContentListener(listener)`.
    pub fn register_text_content_listener<R: ListenerCleanup>(
        &self,
        listener: impl Fn(&str) -> R + 'static,
    ) -> Unregister {
        register_listener(
            self,
            |inner| &inner.text_content_listeners,
            Rc::new(move |text: &str| listener(text).into_cleanup()) as Rc<TextContentListenerFn>,
        )
    }

    /// `triggerListeners('update', ...)`'s loop.
    pub(crate) fn trigger_update_listeners(&self, payload: &UpdateListenerPayload) {
        trigger_listeners(&self.0.update_listeners, |listener| listener(payload));
    }

    /// Whether a text content listener is registered.
    pub(crate) fn has_text_content_listeners(&self) -> bool {
        !self.0.text_content_listeners.borrow().is_empty()
    }

    /// `triggerListeners('textcontent', ...)`'s loop.
    pub(crate) fn trigger_text_content_listeners(&self, text: &str) {
        trigger_listeners(&self.0.text_content_listeners, |listener| listener(text));
    }

    /// `registerCommand(command, listener, priority)`, `priority` being one
    /// of the `COMMAND_PRIORITY_*` constants: 0 to 4 add the listener to the
    /// end of that priority's queue, and the `COMMAND_PRIORITY_BEFORE_*`
    /// ones (-8 to -4) to its beginning (`normalizePriority`'s `priority &
    /// 7`).
    pub fn register_command<P: 'static>(
        &self,
        command: &LexicalCommand<P>,
        listener: impl Fn(&P, &Editor) -> Result<bool, LexicalError> + 'static,
        priority: i8,
    ) -> Unregister {
        let normalized = priority & 7;
        let before = normalized != priority;
        let priority = usize::from(normalized.min(COMMAND_PRIORITY_CRITICAL) as u8);
        let id = self.next_id();
        let listener: CommandListener<P> = Rc::new(listener);
        let command_id = command.id();
        {
            let mut commands = self.0.commands.borrow_mut();
            let list = &mut commands.entry(command_id).or_default()[priority];
            let entry = (id, Rc::new(listener) as Rc<dyn Any>);
            if before {
                list.insert(0, entry);
            } else {
                list.push(entry);
            }
        }
        let weak = self.weak();
        Box::new(move || {
            if let Some(inner) = weak.upgrade() {
                let mut commands = inner.commands.borrow_mut();
                if let Some(lists) = commands.get_mut(&command_id) {
                    lists[priority].retain(|(i, _)| *i != id);
                    // The command leaves the map with its last listener.
                    if lists.iter().all(Vec::is_empty) {
                        commands.remove(&command_id);
                    }
                }
            }
        })
    }

    /// `editor._commands.has(command)`.
    #[cfg(test)]
    pub(crate) fn has_command<P>(&self, command: &LexicalCommand<P>) -> bool {
        self.0.commands.borrow().contains_key(&command.id())
    }

    pub(crate) fn command_listeners<P: 'static>(
        &self,
        command: &LexicalCommand<P>,
        priority: usize,
    ) -> Vec<CommandListener<P>> {
        self.0
            .commands
            .borrow()
            .get(&command.id())
            .map(|lists| {
                lists[priority]
                    .iter()
                    .filter_map(|(_, l)| l.downcast_ref::<CommandListener<P>>().cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// `dispatchCommand(command, payload)`: runs the command's listeners, by
    /// priority (within each, this editor's, then its parent editors'),
    /// until one handles it, and returns whether one did.
    pub fn dispatch_command<P: 'static>(
        &self,
        command: &LexicalCommand<P>,
        payload: P,
    ) -> Result<bool, LexicalError> {
        trigger_command_listeners(self, command, &Rc::new(payload), self)
    }

    // The dirty records.

    pub(crate) fn dirty_type(&self) -> DirtyType {
        self.0.dirty.borrow().dirty_type
    }

    /// Marks a node dirty: an element as intentionally dirty, any other node
    /// as a dirty leaf.
    pub(crate) fn mark_dirty(&self, key: &str, is_element: bool) {
        let mut dirty = self.0.dirty.borrow_mut();
        // Don't downgrade a full reconcile.
        if dirty.dirty_type == DirtyType::None {
            dirty.dirty_type = DirtyType::HasDirtyNodes;
        }
        if is_element {
            dirty.elements.set(key, true);
        } else {
            dirty.leaves.add(key);
        }
    }

    pub(crate) fn add_tags(&self, tags: &[String]) {
        let mut set = self.0.tags.borrow_mut();
        for tag in tags {
            set.add(tag);
        }
    }
}

/// `$hasUpdateTag(tag)`: whether the running update has the tag (one of
/// its options' tags, or one [`add_update_tag`] added). Panics outside an
/// update or a read of an editor, as JS's `getActiveEditor()` throws.
pub fn has_update_tag(tag: &str) -> bool {
    get_editor().0.tags.borrow().contains(tag)
}

/// `$addUpdateTag(tag)`: adds a tag to the running update, which update and
/// mutation listeners and [`has_update_tag`] see. Panics in a read, or
/// outside an update, as JS's `errorOnReadOnly` and `getActiveEditor()`
/// throw.
pub fn add_update_tag(tag: &str) {
    crate::updates::error_on_read_only();
    get_editor().0.tags.borrow_mut().add(tag);
}

/// `$getEditor()`: the editor of the running update or read (or the one
/// `parseEditorState` parses for). Panics outside them, as JS's
/// `getActiveEditor()` throws.
pub fn get_editor() -> Editor {
    crate::updates::get_registry_editor().unwrap_or_else(|| {
        panic!("Unable to find an active editor. This method can only be used synchronously during the callback of editor.update() or editor.read().")
    })
}

/// Runs `f` with the editor's `_updating` set to `updating`, as
/// `triggerListeners` does with `isCurrentlyEnqueuingUpdates`.
fn with_updating<R>(editor: &Editor, updating: bool, f: impl FnOnce() -> R) -> R {
    struct Restore<'a>(&'a Editor, bool);
    impl Drop for Restore<'_> {
        fn drop(&mut self) {
            self.0.0.updating.set(self.1);
        }
    }
    let _restore = Restore(editor, editor.0.updating.replace(updating));
    f()
}

/// `registerListener(listenerMap, listener)`: `unregisterListener` removes
/// the listener and runs what its last call returned.
fn register_listener<F: ?Sized + 'static>(
    editor: &Editor,
    listeners: fn(&EditorInner) -> &Listeners<F>,
    listener: Rc<F>,
) -> Unregister {
    let id = editor.next_id();
    listeners(&editor.0)
        .borrow_mut()
        .push((id, listener, Rc::default()));
    let weak = editor.weak();
    Box::new(move || {
        let Some(inner) = weak.upgrade() else { return };
        let removed = {
            let mut list = listeners(&inner).borrow_mut();
            list.iter()
                .position(|(i, _, _)| *i == id)
                .map(|at| list.remove(at))
        };
        let last = removed.and_then(|(_, _, cleanup)| cleanup.borrow_mut().take());
        if let Some(last) = last {
            last();
        }
    })
}

/// `triggerListeners(type, ...)`'s loop over the listeners registered when
/// it starts: each one's last clean-up runs, then the listener; what it
/// returns is kept for its next call, or runs at once if the listener was
/// unregistered meanwhile.
fn trigger_listeners<F: ?Sized>(listeners: &Listeners<F>, call: impl Fn(&F) -> Option<Unregister>) {
    let snapshot: Vec<_> = listeners
        .borrow()
        .iter()
        .map(|(id, l, cleanup)| (*id, l.clone(), cleanup.clone()))
        .collect();
    for (id, listener, cleanup) in snapshot {
        let last = cleanup.borrow_mut().take();
        if let Some(last) = last {
            last();
        }
        let next = call(&listener);
        if listeners.borrow().iter().any(|(i, _, _)| *i == id) {
            *cleanup.borrow_mut() = next;
        } else if let Some(next) = next {
            next();
        }
    }
}

/// `triggerListener(listenerMap, listener, args)` of a root listener: what
/// it returned last runs, then it runs, and what it returns is kept.
fn trigger_root_listener(
    listener: &RootListener,
    cleanup: &Rc<RefCell<Option<Unregister>>>,
    next: Option<&RootElementRef>,
    prev: Option<&RootElementRef>,
) {
    let last = cleanup.borrow_mut().take();
    if let Some(last) = last {
        last();
    }
    let next_cleanup = listener(next, prev);
    *cleanup.borrow_mut() = next_cleanup;
}

/// `markNodesWithTypesAsDirty(editor, types)`: an update that marks every
/// node of those types in the committed state dirty, in the order JS's
/// node map holds them.
pub(crate) fn mark_nodes_with_types_as_dirty(editor: &Editor, types: &[&str]) {
    let editor_state = editor.get_editor_state();
    let keys: Vec<String> = {
        let node_map = editor_state.0.node_map.borrow();
        editor_state
            .keys_in_creation_order()
            .into_iter()
            .filter(|key| types.contains(&node_map[key].get_type()))
            .collect()
    };
    if keys.is_empty() {
        return;
    }
    let options = UpdateOptions {
        tags: if editor.pending().is_none() {
            vec![HISTORY_MERGE_TAG.to_owned()]
        } else {
            Vec::new()
        },
        ..UpdateOptions::default()
    };
    // Marking existing nodes dirty can't fail.
    let _ = editor.update(
        move || {
            for key in &keys {
                if let Some(node) = crate::get_node_by_key(key) {
                    node.mark_dirty();
                }
            }
            Ok(())
        },
        options,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{create_paragraph_node, create_text_node, get_root};

    fn discrete() -> UpdateOptions {
        UpdateOptions {
            discrete: true,
            ..UpdateOptions::default()
        }
    }

    #[test]
    fn has_nodes_and_get_editor() {
        let editor = create_editor(CreateEditorArgs::default());
        assert!(editor.has_node("text") && editor.has_node("paragraph"));
        assert!(!editor.has_node("heading"));
        assert!(editor.has_nodes(&[]) && editor.has_nodes(&["text", "root"]));
        assert!(!editor.has_nodes(&["text", "heading"]));
        let same = Rc::new(Cell::new(false));
        let (seen, active) = (same.clone(), editor.clone());
        editor
            .update(
                move || {
                    seen.set(get_editor().ptr_eq(&active));
                    Ok(())
                },
                discrete(),
            )
            .unwrap();
        assert!(same.get());
    }

    #[test]
    fn create_editor_starts_with_the_given_state() {
        let source = create_editor(CreateEditorArgs::default());
        source
            .update(
                || {
                    let paragraph = create_paragraph_node();
                    paragraph.append(&[create_text_node("hello")])?;
                    get_root().append(&[paragraph])
                },
                discrete(),
            )
            .unwrap();
        let editor = create_editor(CreateEditorArgs {
            editor_state: Some(source.get_editor_state()),
            editable: Some(false),
            ..CreateEditorArgs::default()
        });
        assert!(!editor.is_editable());
        assert_eq!(
            editor.read(|| get_root().get_text_content()).unwrap(),
            "hello"
        );
    }

    #[test]
    fn focus_without_a_root_element_does_nothing() {
        let editor = create_editor(CreateEditorArgs::default());
        let called = Rc::new(Cell::new(false));
        let flag = called.clone();
        editor
            .focus(
                Some(Box::new(move || flag.set(true))),
                EditorFocusOptions::default(),
            )
            .unwrap();
        assert!(!called.get());
        assert!(editor.read(crate::get_selection).unwrap().is_none());
    }

    #[test]
    fn on_warn_hears_an_endless_update_cascade() {
        let warnings = Rc::new(RefCell::new(Vec::<String>::new()));
        let heard = warnings.clone();
        let editor = create_editor(CreateEditorArgs {
            on_warn: Some(Rc::new(move |warning| {
                heard.borrow_mut().push(warning.message().to_owned());
                Ok(())
            })),
            ..CreateEditorArgs::default()
        });
        let updates = Rc::new(Cell::new(0));
        let (count, target) = (updates.clone(), editor.weak());
        let _unregister = editor.register_update_listener(move |_| {
            let Some(editor) = target.upgrade().map(Editor) else {
                return;
            };
            if count.replace(count.get() + 1) < 1000 {
                let _ = editor.update(|| get_root().append(&[create_paragraph_node()]), discrete());
            }
        });
        editor
            .update(|| get_root().append(&[create_paragraph_node()]), discrete())
            .unwrap();
        let warnings = warnings.borrow();
        assert_eq!(warnings.len(), 1, "{warnings:?} after {}", updates.get());
        assert!(warnings[0].starts_with("One or more update listeners are endlessly enqueueing"));
        assert!(updates.get() < 1000);
    }
}
