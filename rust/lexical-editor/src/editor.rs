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
/// static `importJSON`).
#[derive(Clone, Copy)]
pub struct NodeClass {
    pub(crate) node_type: &'static str,
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

fn is_inline_element_of<T: LexicalNode + Default>() -> bool {
    let node = T::default();
    node.element().is_some() && node.is_inline()
}

/// The `importJSON` a class gets when it declares none
/// (`synthesizeImportJSON`): build the node, then apply the JSON.
fn import_default<T: LexicalNode + Default>(json: &Object) -> Result<NodeRef, LexicalError> {
    let mut node = T::default();
    node.update_from_json(json);
    Ok(set_node_key(Box::new(node)))
}

impl NodeClass {
    /// The class of `T`, which imports a node as `T::default()` with the
    /// JSON applied.
    pub fn of<T: LexicalNode + Default>() -> Self {
        Self {
            node_type: T::default().get_type(),
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
        }
    }

    pub fn node_type(&self) -> &'static str {
        self.node_type
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

/// `CreateEditorArgs`: the node classes to register besides the core ones,
/// the editor's `namespace` (when empty, its parent editor's, or a random
/// one, `createUID()`), the theme, the parent editor, and (with the `html`
/// feature) the HTML import and export overrides and the render config.
#[derive(Clone, Default)]
pub struct CreateEditorArgs {
    pub nodes: Vec<NodeClass>,
    pub namespace: String,
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

/// `EditorUpdateOptions`. `onUpdate` and `event` come with their users.
#[derive(Clone, Debug, Default)]
pub struct UpdateOptions {
    /// Commit at the end of the update instead of at the next read.
    pub discrete: bool,
    /// Update tags (`tag`).
    pub tags: Vec<String>,
    pub skip_transforms: bool,
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
}

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

pub const COMMAND_PRIORITY_EDITOR: u8 = 0;
pub const COMMAND_PRIORITY_LOW: u8 = 1;
pub const COMMAND_PRIORITY_NORMAL: u8 = 2;
pub const COMMAND_PRIORITY_HIGH: u8 = 3;
pub const COMMAND_PRIORITY_CRITICAL: u8 = 4;

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

type Listeners<F> = RefCell<Vec<(u64, Rc<F>)>>;
type UpdateListener = Rc<dyn Fn(&UpdateListenerPayload)>;
type TextContentListener = Rc<dyn Fn(&str)>;
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
    update_listeners: Listeners<dyn Fn(&UpdateListenerPayload)>,
    text_content_listeners: Listeners<dyn Fn(&str)>,
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
    editable_listeners: Listeners<dyn Fn(bool)>,
    /// `_listeners.root`, each with what its last call returned.
    root_listeners: RefCell<Vec<RootListenerEntry>>,
    /// `_deferred`: the `$onUpdate` callbacks of the pending update.
    pub(crate) deferred: RefCell<Vec<Box<dyn FnOnce()>>>,
    /// `_compositionKey`: the node an input method is composing in.
    pub(crate) composition_key: RefCell<Option<NodeKey>>,
    /// `_compositionMergeHeld`: the composed node a merge was held back
    /// for (`DEFER_COMPOSITION_MERGE_TAG`).
    pub(crate) composition_merge_held: RefCell<Option<NodeKey>>,
    /// `_inputState`, the part the model reads.
    pub(crate) input_state: RefCell<crate::events::InputState>,
    /// `_keyDownShortcuts`: the compiled key-down bindings.
    pub(crate) key_down_shortcuts: RefCell<Option<crate::events::KeyDownShortcuts>>,
    /// `RootNode.__cachedText`: the root the reconciler last cached its
    /// text content on, and that text. JS keeps it on the root node, which
    /// a clone resets; here the cache names the root by identity.
    pub(crate) root_text_cache: RefCell<Option<(Rc<dyn LexicalNode>, String)>>,
    /// The `LexicalBuilder` that built the editor (JS's
    /// `editor[builderSymbol]`).
    #[cfg(feature = "extension")]
    pub(crate) builder: RefCell<Option<Rc<dyn Any>>>,
}

/// `createEditor(config)`: registers the core nodes (root, text, line
/// break, tab, paragraph, artificial) and `args.nodes`, in `args.namespace`,
/// or the parent editor's when that is empty. (JS's `createEditor()` with
/// no config at all takes the active editor as the parent; Rust always
/// takes a config, as Swift does.)
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
        editable: Cell::new(true),
        slots_used: Cell::new(false),
        root_element: RefCell::new(None),
        node_structure: RefCell::new(None),
        mutation_listeners: RefCell::new(Vec::new()),
        editable_listeners: RefCell::new(Vec::new()),
        root_listeners: RefCell::new(Vec::new()),
        deferred: RefCell::new(Vec::new()),
        composition_key: RefCell::new(None),
        composition_merge_held: RefCell::new(None),
        input_state: RefCell::new(crate::events::InputState::default()),
        key_down_shortcuts: RefCell::new(None),
        root_text_cache: RefCell::new(None),
        #[cfg(feature = "extension")]
        builder: RefCell::new(None),
    }));
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

    /// `isEditable()`.
    pub fn is_editable(&self) -> bool {
        self.0.editable.get()
    }

    /// `setEditable(editable)`: when that changes the editor's mode, the
    /// editable listeners hear of it.
    pub fn set_editable(&self, editable: bool) {
        if self.0.editable.get() != editable {
            self.0.editable.set(editable);
            let listeners: Vec<_> = self
                .0
                .editable_listeners
                .borrow()
                .iter()
                .map(|(_, l)| l.clone())
                .collect();
            with_updating(self, true, || {
                for listener in listeners {
                    listener(editable);
                }
            });
        }
    }

    /// `registerEditableListener(listener)`: `listener` hears each change of
    /// [`is_editable`](Self::is_editable).
    pub fn register_editable_listener(&self, listener: impl Fn(bool) + 'static) -> Unregister {
        let id = self.next_id();
        self.0
            .editable_listeners
            .borrow_mut()
            .push((id, Rc::new(listener)));
        let weak = self.weak();
        Box::new(move || {
            if let Some(inner) = weak.upgrade() {
                inner
                    .editable_listeners
                    .borrow_mut()
                    .retain(|(i, _)| *i != id);
            }
        })
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
        self.0.nodes.get(node_type).copied()
    }

    /// The registered node classes in the order JS's `_nodes` holds them.
    pub(crate) fn registered_nodes_in_order(&self) -> Vec<NodeClass> {
        self.0
            .node_order
            .iter()
            .map(|node_type| self.0.nodes[node_type])
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

    /// `parseEditorState(json)` from the JSON text.
    pub fn parse_editor_state(&self, json: &str) -> Result<EditorState, LexicalError> {
        let value = json::parse(json).map_err(|e| LexicalError::new(e.to_string()))?;
        self.parse_editor_state_json(&value)
    }

    /// `parseEditorState(serializedEditorState)` from parsed JSON.
    pub fn parse_editor_state_json(&self, json: &Value) -> Result<EditorState, LexicalError> {
        parse_editor_state(json, self)
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
        if !self.0.dispatching_commit_listeners.get() {
            commit_pending_updates(self)?;
        }
        Ok(read_editor_state(Some(self), &self.get_editor_state(), f))
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
        if mode == EditorReadMode::ForceCommit && !self.0.dispatching_commit_listeners.get() {
            commit_pending_updates(self)?;
        }
        let editor_state = match mode {
            EditorReadMode::Pending => self.pending().unwrap_or_else(|| self.get_editor_state()),
            _ => self.get_editor_state(),
        };
        Ok(read_editor_state(Some(self), &editor_state, f))
    }

    /// `registerNodeTransform(klass, listener)`, by the node type. Nodes of
    /// that type already in the editor are marked dirty, so the transform
    /// runs on them with the next update.
    pub fn register_node_transform(
        &self,
        node_type: &'static str,
        transform: impl Fn(&NodeRef) -> Result<(), LexicalError> + 'static,
    ) -> Unregister {
        let unregister = self.add_node_transform(node_type, transform);
        mark_nodes_with_types_as_dirty(self, &[node_type]);
        unregister
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
    pub fn register_update_listener(
        &self,
        listener: impl Fn(&UpdateListenerPayload) + 'static,
    ) -> Unregister {
        let id = self.next_id();
        self.0
            .update_listeners
            .borrow_mut()
            .push((id, Rc::new(listener)));
        let weak = self.weak();
        Box::new(move || {
            if let Some(inner) = weak.upgrade() {
                inner
                    .update_listeners
                    .borrow_mut()
                    .retain(|(i, _)| *i != id);
            }
        })
    }

    /// `registerTextContentListener(listener)`.
    pub fn register_text_content_listener(&self, listener: impl Fn(&str) + 'static) -> Unregister {
        let id = self.next_id();
        self.0
            .text_content_listeners
            .borrow_mut()
            .push((id, Rc::new(listener)));
        let weak = self.weak();
        Box::new(move || {
            if let Some(inner) = weak.upgrade() {
                inner
                    .text_content_listeners
                    .borrow_mut()
                    .retain(|(i, _)| *i != id);
            }
        })
    }

    pub(crate) fn update_listeners(&self) -> Vec<UpdateListener> {
        self.0
            .update_listeners
            .borrow()
            .iter()
            .map(|(_, l)| l.clone())
            .collect()
    }

    pub(crate) fn text_content_listeners(&self) -> Vec<TextContentListener> {
        self.0
            .text_content_listeners
            .borrow()
            .iter()
            .map(|(_, l)| l.clone())
            .collect()
    }

    /// `registerCommand(command, listener, priority)`, `priority` being one
    /// of the `COMMAND_PRIORITY_*` constants (0 to 4).
    pub fn register_command<P: 'static>(
        &self,
        command: &LexicalCommand<P>,
        listener: impl Fn(&P, &Editor) -> Result<bool, LexicalError> + 'static,
        priority: u8,
    ) -> Unregister {
        let priority = usize::from(priority.min(COMMAND_PRIORITY_CRITICAL));
        let id = self.next_id();
        let listener: CommandListener<P> = Rc::new(listener);
        let command_id = command.id();
        self.0.commands.borrow_mut().entry(command_id).or_default()[priority]
            .push((id, Rc::new(listener) as Rc<dyn Any>));
        let weak = self.weak();
        Box::new(move || {
            if let Some(inner) = weak.upgrade()
                && let Some(lists) = inner.commands.borrow_mut().get_mut(&command_id)
            {
                lists[priority].retain(|(i, _)| *i != id);
            }
        })
    }

    /// `registerCommand(command, listener, COMMAND_PRIORITY_BEFORE_*)`: as
    /// [`register_command`](Self::register_command), but at the start of
    /// `priority`'s queue (JS's `COMMAND_PRIORITY_BEFORE_EDITOR` is the
    /// start of the editor queue).
    #[cfg(any(feature = "auto-embed", feature = "list"))]
    pub(crate) fn register_command_before<P: 'static>(
        &self,
        command: &LexicalCommand<P>,
        listener: impl Fn(&P, &Editor) -> Result<bool, LexicalError> + 'static,
        priority: u8,
    ) -> Unregister {
        let priority = usize::from(priority.min(COMMAND_PRIORITY_CRITICAL));
        let id = self.next_id();
        let listener: CommandListener<P> = Rc::new(listener);
        let command_id = command.id();
        self.0.commands.borrow_mut().entry(command_id).or_default()[priority]
            .insert(0, (id, Rc::new(listener) as Rc<dyn Any>));
        let weak = self.weak();
        Box::new(move || {
            if let Some(inner) = weak.upgrade()
                && let Some(lists) = inner.commands.borrow_mut().get_mut(&command_id)
            {
                lists[priority].retain(|(i, _)| *i != id);
            }
        })
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

/// `$hasUpdateTag(tag)`: whether the running update has the tag. Panics
/// outside an update or a read of an editor, as JS's `getActiveEditor()`
/// throws.
pub fn has_update_tag(tag: &str) -> bool {
    crate::updates::get_active_editor()
        .expect("Unable to find an active editor. This method can only be used synchronously during the callback of editor.update() or editor.read().")
        .0
        .tags
        .borrow()
        .contains(tag)
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
