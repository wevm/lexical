//! `LexicalEditor.ts`: creating an editor, its registered nodes, and the
//! registration of transforms, listeners and commands.

use crate::editor_state::{clone_editor_state, create_empty_editor_state};
use crate::error::invariant;
use crate::json::{self, Object, Value};
use crate::lexical_utils::set_node_key;
use crate::node::{LexicalNode, NodeKey, NodeRef};
use crate::nodes::{
    line_break::LineBreakNode, paragraph::ParagraphNode, root::RootNode, tab::TabNode,
    text::TextNode,
};
use crate::ordered::{OrderedMap, OrderedSet};
use crate::selection_deletion::SelectionMovement;
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
        }
    }

    /// A class with its own `importJSON`.
    pub fn with_import_json(
        node_type: &'static str,
        import_json: fn(&Object) -> Result<NodeRef, LexicalError>,
    ) -> Self {
        Self {
            node_type,
            import_json,
        }
    }

    pub fn node_type(&self) -> &'static str {
        self.node_type
    }
}

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

/// `CreateEditorArgs`: the node classes to register besides the core ones.
#[derive(Clone, Default)]
pub struct CreateEditorArgs {
    pub nodes: Vec<NodeClass>,
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

type Listeners<F> = RefCell<Vec<(u64, Rc<F>)>>;
type UpdateListener = Rc<dyn Fn(&UpdateListenerPayload)>;
type TextContentListener = Rc<dyn Fn(&str)>;
/// A command's listeners by priority, each a `CommandListener<P>` of the
/// command's payload type.
type CommandListeners = [Vec<(u64, Rc<dyn Any>)>; 5];

pub(crate) struct EditorInner {
    nodes: HashMap<&'static str, NodeClass>,
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
}

/// `createEditor(config)`: registers the core nodes (root, text, line
/// break, tab, paragraph) and `args.nodes`.
pub fn create_editor(args: CreateEditorArgs) -> Editor {
    let core = [
        NodeClass::with_import_json("root", RootNode::import_json),
        NodeClass::of::<TextNode>(),
        NodeClass::of::<LineBreakNode>(),
        NodeClass::of::<TabNode>(),
        NodeClass::of::<ParagraphNode>(),
    ];
    let nodes = core
        .into_iter()
        .chain(args.nodes)
        .map(|class| (class.node_type, class))
        .collect();
    Editor(Rc::new(EditorInner {
        nodes,
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
    }))
}

impl Editor {
    pub(crate) fn registered_nodes(&self) -> &HashMap<&'static str, NodeClass> {
        &self.0.nodes
    }

    pub(crate) fn ptr_eq(&self, other: &Editor) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn next_id(&self) -> u64 {
        self.0.next_id.replace(self.0.next_id.get() + 1)
    }

    fn weak(&self) -> Weak<EditorInner> {
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

    /// `registerNodeTransform(klass, listener)`, by the node type. Nodes of
    /// that type already in the editor are marked dirty, so the transform
    /// runs on them with the next update.
    pub fn register_node_transform(
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
        mark_nodes_with_types_as_dirty(self, &[node_type]);
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
    /// priority, until one handles it, and returns whether one did.
    pub fn dispatch_command<P: 'static>(
        &self,
        command: &LexicalCommand<P>,
        payload: P,
    ) -> Result<bool, LexicalError> {
        trigger_command_listeners(self, command, &payload)
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

/// `markNodesWithTypesAsDirty(editor, types)`: an update that marks every
/// node of those types in the committed state dirty, in the order JS's
/// node map holds them.
fn mark_nodes_with_types_as_dirty(editor: &Editor, types: &[&str]) {
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
