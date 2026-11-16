//! `BlockIdExtension.ts` (`@experimental`): stable block ids, stored as
//! NodeState ([`block_id_state`]), so that code outside the editor can
//! address a block by id across sessions, replicas and serializations. The
//! positional ids are in `positional_block_id.rs` and the handles in
//! `block_handles.rs`.
//!
//! Every identified block gets an id in the update that creates it. A
//! document stored without ids gets positional ids, which every reader and
//! replica derives alike; blocks created by edits get generated ids
//! ([`generate_block_id`]). Ids are unique: a copy yields to the block it
//! was copied from. While enabled, [`get_block_by_id`] looks blocks up in an
//! index maintained from the dirty nodes of each commit.
//!
//! Not ported: JS's `__DEV__` warning for a repair update that left blocks
//! without a unique id (Rust has no warning channel), and the per-state
//! cache of handle lookups, which only saves repeated lookups of the same
//! frozen state.

pub use super::block_handles::MIN_BLOCK_HANDLE_LENGTH;
use super::block_handles::{
    HANDLE_KEY_LENGTH, HandleTable, build_handle_table, handle_in_table, handle_key,
    ids_with_prefix,
};
use super::get_extension_dependency::get_extension_output;
use super::positional_block_id::{
    ID_ALPHABET, ID_LENGTH, derive_duplicate_id, derive_positional_id, document_key,
};
use super::signals::{Signal, effect, signal};
use crate::editor_state::EditorState;
use crate::extension_core::{
    ExtensionBuildState, ExtensionConfigBase, ExtensionInitState, ExtensionRegisterState,
    InitialEditorConfig, LexicalExtension, declare_peer_dependency, define_extension,
};
use crate::json::Value;
use crate::node::{LexicalNode, NodeKey};
use crate::node_state::{StateConfig, StateValueConfig, create_state_with, get_state, set_state};
use crate::ordered::{OrderedMap, OrderedSet};
use crate::slot::{
    get_slot, get_slot_host, get_slot_name_within_host, get_slot_names, slots_field,
};
use crate::updates::{get_active_editor, internal_get_active_editor_state, read_editor_state};
use crate::{
    COLLABORATION_TAG, Editor, HISTORIC_TAG, HISTORY_MERGE_TAG, LexicalError, NodeRef, Unregister,
    UpdateListenerPayload, UpdateOptions, get_node_by_key, get_root, has_update_tag,
};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};

/// The most candidates an ambiguous [`BlockHandleResult`] lists, so that a
/// handle matching many blocks (of ids that are not random) stays short.
const MAX_HANDLE_CANDIDATES: usize = 10;

/// Attempts to find a valid, unused id before giving up, which only a broken
/// `generate_id` can exhaust.
const MAX_ID_ATTEMPTS: usize = 32;

/// Tag of the follow-up update that repairs ids after a commit applied with
/// `skip_transforms` or tagged `COLLABORATION_TAG` or `HISTORIC_TAG`.
const BLOCK_ID_REPAIR_TAG: &str = "block-id-repair";

/// `BLOCK_ID_PATTERN` (`/^[A-Za-z0-9_-]{1,64}$/`): the ids accepted from
/// JSON, HTML, another replica, or `generate_id`.
fn matches_block_id_pattern(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}

/// `parseBlockId(value)`: a string that is a valid id, else no id.
fn parse_block_id(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|id| matches_block_id_pattern(id))
        .map(str::to_owned)
}

thread_local! {
    static BLOCK_ID_STATE: StateConfig<Option<String>> = create_state_with(
        "blockId",
        StateValueConfig {
            parse: Rc::new(parse_block_id),
            unparse: Rc::new(|id: &Option<String>| {
                id.as_ref().map_or(Value::Undefined, |id| Value::String(id.clone()))
            }),
            is_equal: Rc::new(|a: &Option<String>, b: &Option<String>| a == b),
            reset_on_copy_node: true,
            derived: false,
            schema: None,
        },
    );
}

/// `blockIdState` (`@experimental`): the NodeState that stores a block's
/// id, serialized in node JSON as `"$": {"blockId": "..."}`.
///
/// It is ordinary NodeState, so it survives clones and undo, round-trips
/// through `exportJSON`/`importJSON`, and is carried by anything that
/// copies NodeState. It is reset by `$copyNode`, so a node split off
/// another one starts without an id instead of with a duplicate.
///
/// A valid id is 1 to 64 characters of the base64url alphabet (`A`–`Z`,
/// `a`–`z`, `0`–`9`, `-`, `_`). Anything else reads as no id, so the block
/// gets a new one as if it had none.
pub fn block_id_state() -> StateConfig<Option<String>> {
    BLOCK_ID_STATE.with(Clone::clone)
}

/// The id stored on a version of a node, re-parsed: a value set directly
/// with `set_state` reads as no id when it is invalid.
fn id_of_version(node: &dyn LexicalNode) -> Result<Option<String>, LexicalError> {
    Ok(
        crate::node_state::get_state_direct(node, &block_id_state())?
            .filter(|id| matches_block_id_pattern(id)),
    )
}

/// `$getBlockId(node)` (`@experimental`): the id stored on `node`, or
/// `None` if it has none.
///
/// This reads the NodeState directly, so it also returns an id left on a
/// node that is not an identified block; such ids are not indexed and
/// [`get_block_by_id`] does not find them.
pub fn get_block_id(node: &NodeRef) -> Result<Option<String>, LexicalError> {
    Ok(get_state(node, &block_id_state())?.filter(|id| matches_block_id_pattern(id)))
}

fn set_block_id(node: &NodeRef, id: Option<String>) -> Result<(), LexicalError> {
    set_state(node, &block_id_state(), id).map(|_| ())
}

/// `$getExtensionOutput(BlockIdExtension)`.
fn output() -> Result<Rc<BlockIdOutput>, LexicalError> {
    get_extension_output(&block_id_extension())?
        .ok_or_else(|| LexicalError::new("BlockIdExtension has no output"))
}

/// `$getBlockById(id)` (`@experimental`): the identified block with the
/// given id in the active editor state, or `None`. Requires
/// [`block_id_extension`].
///
/// The lookup goes through an index kept up to date from the dirty nodes of
/// each commit and is verified against the active state, so it never
/// returns a stale node. A block created, replaced or re-keyed earlier in
/// the same update is found once that update has committed. The match is
/// exact: a prefix of an id finds nothing.
pub fn get_block_by_id(id: &str) -> Result<Option<NodeRef>, LexicalError> {
    (output()?.get_block_by_id)(id)
}

/// Why a handle is [`BlockHandleResult::Invalid`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockHandleInvalidReason {
    /// `'too-short'`: fewer than [`MIN_BLOCK_HANDLE_LENGTH`] characters.
    TooShort,
    /// `'malformed'`: not 1 to 64 characters of the base64url alphabet.
    Malformed,
}

impl BlockHandleInvalidReason {
    /// The JS `reason`.
    pub fn as_str(self) -> &'static str {
        match self {
            BlockHandleInvalidReason::TooShort => "too-short",
            BlockHandleInvalidReason::Malformed => "malformed",
        }
    }
}

/// `BlockHandleResult` (`@experimental`): what [`resolve_block_handle`]
/// found for a handle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockHandleResult {
    /// The one identified block that the handle names.
    Found { node: NodeRef },
    /// It names no identified block.
    NotFound { handle: String },
    /// It names several: `candidates` holds their handles (current ones, or
    /// with `read` the read's), at most 10.
    Ambiguous {
        handle: String,
        candidates: Vec<String>,
    },
    /// Neither a handle nor an id.
    Invalid {
        handle: String,
        reason: BlockHandleInvalidReason,
    },
}

impl BlockHandleResult {
    /// The JS `status`: `'found'`, `'not-found'`, `'ambiguous'` or
    /// `'invalid'`.
    pub fn status(&self) -> &'static str {
        match self {
            BlockHandleResult::Found { .. } => "found",
            BlockHandleResult::NotFound { .. } => "not-found",
            BlockHandleResult::Ambiguous { .. } => "ambiguous",
            BlockHandleResult::Invalid { .. } => "invalid",
        }
    }

    /// The block, when found.
    pub fn node(&self) -> Option<&NodeRef> {
        match self {
            BlockHandleResult::Found { node } => Some(node),
            _ => None,
        }
    }
}

fn not_found(handle: &str) -> BlockHandleResult {
    BlockHandleResult::NotFound {
        handle: handle.to_owned(),
    }
}

fn invalid(handle: &str, reason: BlockHandleInvalidReason) -> BlockHandleResult {
    BlockHandleResult::Invalid {
        handle: handle.to_owned(),
        reason,
    }
}

/// `$getBlockHandle(node)` (`@experimental`): the handle of an identified
/// block, a short, opaque name for it in the current document: the shortest
/// prefix, at least [`MIN_BLOCK_HANDLE_LENGTH`] characters long, of a
/// 64-bit hash of the block's id that no other identified block's hash
/// starts with. `None` for a node that has no id or that
/// [`get_block_by_id`] would not find by its id. Requires
/// [`block_id_extension`].
///
/// Handles are computed all at once from the blocks of the last committed
/// state: from the index while the extension is enabled (cached until the
/// ids change), and from a scan of that state while it is disabled (cached
/// until the next commit).
pub fn get_block_handle(node: &NodeRef) -> Result<Option<String>, LexicalError> {
    (output()?.get_block_handle)(node)
}

/// `$getBlockHandles()` (`@experimental`): the handle of every identified
/// block, by id (JS's `Map`, as its entries in order). Requires
/// [`block_id_extension`].
pub fn get_block_handles() -> Result<Vec<(String, String)>, LexicalError> {
    (output()?.get_block_handles)()
}

/// `$resolveBlockHandle(handle, options)` (`@experimental`): the identified
/// block that a handle from [`get_block_handle`] names, for input that
/// cannot be trusted: a mistyped, outdated or invalid handle is reported in
/// a [`BlockHandleResult`] instead of failing. A string longer than 11
/// characters is taken as a whole id. Pass the handles the model was shown
/// as [`BlockHandleOptions::read`] whenever the document can change between
/// the read and the edit. Requires [`block_id_extension`].
pub fn resolve_block_handle(
    handle: &str,
    options: Option<&BlockHandleOptions>,
) -> Result<BlockHandleResult, LexicalError> {
    (output()?.resolve_block_handle)(handle, options)
}

/// `BlockHandleOptions` (`@experimental`): options of
/// [`resolve_block_handle`].
#[derive(Clone, Debug, Default)]
pub struct BlockHandleOptions {
    /// The handles the model was shown, as [`get_block_handles`] returned
    /// them at the read (id → handle). With it, a handle names the block it
    /// named in that read, and is found only while that block still exists.
    pub read: Option<Vec<(String, String)>>,
}

/// `$isRootChildBlock(node)` (`@experimental`): an `is_identified_block`
/// that identifies every element and decorator child of the root, and
/// nothing nested.
pub fn is_root_child_block(node: &NodeRef) -> bool {
    (node.is_element() || node.is_decorator()) && node.get_parent().is_some_and(|p| p.is_root())
}

/// `$defaultIsIdentifiedBlock(node)` (`@experimental`): the default
/// `is_identified_block`:
///
/// - every element and decorator child of the root;
/// - every nested block that holds text: a non-inline element whose class
///   declares `children: 'inline'` in `$config()` (or a list that includes
///   `'inline'`, [`LexicalNode::declares_inline_children`]);
/// - every nested non-inline decorator.
///
/// Nested wrappers get no id: shadow roots and nested elements whose class
/// declares no `children`. A slot is nested like a shadow root.
pub fn default_is_identified_block(node: &NodeRef) -> bool {
    if !node.is_element() && !node.is_decorator() {
        return false;
    }
    let parent = node.get_parent();
    if parent.as_ref().is_some_and(NodeRef::is_root) {
        return true;
    }
    if (parent.is_none() && get_slot_host(node).is_none()) || node.is_inline() {
        return false;
    }
    if !node.is_element() {
        // A block decorator.
        return true;
    }
    let latest = node.get_latest();
    !latest.is_shadow_root() && latest.declares_inline_children()
}

/// `generateBlockId()` (`@experimental`): the default
/// [`BlockIdConfig::generate_id`]: 21 characters of the base64url alphabet
/// (126 random bits). JS draws them from `crypto.getRandomValues`; without
/// dependencies, Rust hashes a counter and the clock with the standard
/// library's randomly keyed `RandomState`, which is not a cryptographic
/// generator.
pub fn generate_block_id() -> String {
    use std::hash::BuildHasher;
    thread_local! {
        static COUNTER: Cell<u64> = const { Cell::new(0) };
    }
    let count = COUNTER.with(|c| c.replace(c.get().wrapping_add(1)));
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let state = std::collections::hash_map::RandomState::new();
    let mut bits = [
        state.hash_one((count, nanos, 0u8)),
        state.hash_one((count, nanos, 1u8)),
        state.hash_one((count, nanos, 2u8)),
    ];
    let mut id = String::with_capacity(ID_LENGTH);
    for i in 0..ID_LENGTH {
        let word = &mut bits[i / 10];
        id.push(ID_ALPHABET[(*word & 63) as usize] as char);
        *word >>= 6;
    }
    id
}

/// [`BlockIdConfig::is_identified_block`]: which nodes get an id.
#[derive(Clone)]
pub struct IsIdentifiedBlock(pub Rc<dyn Fn(&NodeRef) -> bool>);

/// [`BlockIdConfig::generate_id`]: makes a new id.
#[derive(Clone)]
pub struct GenerateId(pub Rc<dyn Fn() -> String>);

/// `BlockIdConfig` (`@experimental`).
#[derive(Clone)]
pub struct BlockIdConfig {
    /// Set to `true` to stop assigning, repairing, and indexing ids. Ids
    /// already stored on nodes are kept. While disabled, [`get_block_by_id`]
    /// falls back to a scan of the document.
    pub disabled: bool,
    /// `$isIdentifiedBlock`: which nodes get an id. Evaluated when a node is
    /// marked dirty, so it should depend only on the node and its ancestors.
    /// Every reader of a document stored without ids must use the same
    /// predicate, since positional ids follow from it. Default:
    /// [`default_is_identified_block`].
    pub is_identified_block: IsIdentifiedBlock,
    /// Makes a new id: 1 to 64 characters of the base64url alphabet. A value
    /// that is invalid or already used in this editor is discarded and
    /// `generate_id` is called again; 32 such values in a row fail. Default:
    /// [`generate_block_id`].
    pub generate_id: GenerateId,
    /// Whether the follow-up update that names the blocks an update applied
    /// with `skip_transforms`, or tagged `COLLABORATION_TAG` or
    /// `HISTORIC_TAG`, left without a valid id or with a repeated one runs
    /// the editor's transforms (`true`, the default), or only sets their ids
    /// (`false`).
    pub repair_transforms: bool,
}

impl Default for BlockIdConfig {
    fn default() -> Self {
        BlockIdConfig {
            disabled: false,
            is_identified_block: IsIdentifiedBlock(Rc::new(default_is_identified_block)),
            generate_id: GenerateId(Rc::new(generate_block_id)),
            repair_transforms: true,
        }
    }
}

/// `Partial<BlockIdConfig>`.
#[derive(Clone, Default)]
pub struct BlockIdConfigOverrides {
    pub disabled: Option<bool>,
    pub is_identified_block: Option<IsIdentifiedBlock>,
    pub generate_id: Option<GenerateId>,
    pub repair_transforms: Option<bool>,
}

impl ExtensionConfigBase for BlockIdConfig {
    type Partial = BlockIdConfigOverrides;

    fn shallow_merge(&self, overrides: &BlockIdConfigOverrides) -> Self {
        BlockIdConfig {
            disabled: overrides.disabled.unwrap_or(self.disabled),
            is_identified_block: overrides
                .is_identified_block
                .clone()
                .unwrap_or_else(|| self.is_identified_block.clone()),
            generate_id: overrides
                .generate_id
                .clone()
                .unwrap_or_else(|| self.generate_id.clone()),
            repair_transforms: overrides
                .repair_transforms
                .unwrap_or(self.repair_transforms),
        }
    }
}

/// The type of [`BlockIdOutput::get_block_by_id`].
pub type GetBlockByIdFn = Rc<dyn Fn(&str) -> Result<Option<NodeRef>, LexicalError>>;
/// The type of [`BlockIdOutput::get_block_handle`].
pub type GetBlockHandleFn = Rc<dyn Fn(&NodeRef) -> Result<Option<String>, LexicalError>>;
/// The type of [`BlockIdOutput::get_block_handles`].
pub type GetBlockHandlesFn = Rc<dyn Fn() -> Result<Vec<(String, String)>, LexicalError>>;
/// The type of [`BlockIdOutput::resolve_block_handle`].
pub type ResolveBlockHandleFn =
    Rc<dyn Fn(&str, Option<&BlockHandleOptions>) -> Result<BlockHandleResult, LexicalError>>;

/// `BlockIdOutput` (`@experimental`): the output of [`block_id_extension`].
#[derive(Clone)]
pub struct BlockIdOutput {
    /// See [`BlockIdConfig::disabled`].
    pub disabled: Signal<bool>,
    /// `$getBlockById`: the same as [`get_block_by_id`] for this editor.
    pub get_block_by_id: GetBlockByIdFn,
    /// `$getBlockHandle`: the same as [`get_block_handle`] for this editor.
    pub get_block_handle: GetBlockHandleFn,
    /// `$getBlockHandles`: the same as [`get_block_handles`] for this
    /// editor.
    pub get_block_handles: GetBlockHandlesFn,
    /// `$resolveBlockHandle`: the same as [`resolve_block_handle`] for this
    /// editor.
    pub resolve_block_handle: ResolveBlockHandleFn,
    /// `$isIdentifiedBlock`: the configured
    /// [`BlockIdConfig::is_identified_block`].
    pub is_identified_block: IsIdentifiedBlock,
}

/// The keys of the blocks that carry an id: one, or a set only while a
/// duplicate awaits repair.
enum HolderKeys {
    One(NodeKey),
    Many(OrderedSet),
}

/// An index of the identified blocks of the committed editor state: id →
/// the key of the block that carries it, and key → id.
#[derive(Default)]
struct BlockIdIndex {
    keys_of_id: OrderedMap<HolderKeys>,
    id_of_key: OrderedMap<String>,
    /// Changes whenever the set of indexed ids may have changed.
    version: u64,
}

impl BlockIdIndex {
    fn clear(&mut self) {
        self.keys_of_id = OrderedMap::new();
        self.id_of_key = OrderedMap::new();
        self.version += 1;
    }

    fn holders(&self, id: &str) -> Vec<NodeKey> {
        match self.keys_of_id.get(id) {
            None => Vec::new(),
            Some(HolderKeys::One(key)) => vec![key.clone()],
            Some(HolderKeys::Many(keys)) => keys.iter().map(str::to_owned).collect(),
        }
    }

    fn is_repeated(&self, id: &str) -> bool {
        matches!(self.keys_of_id.get(id), Some(HolderKeys::Many(_)))
    }

    fn set(&mut self, key: &str, id: &str) {
        if self.id_of_key.get(key).map(String::as_str) == Some(id) {
            return;
        }
        self.remove(key);
        self.version += 1;
        self.id_of_key.set(key, id.to_owned());
        match self.keys_of_id.get_mut(id) {
            None => self.keys_of_id.set(id, HolderKeys::One(key.to_owned())),
            Some(HolderKeys::One(other)) => {
                let mut keys = OrderedSet::new();
                keys.add(other);
                keys.add(key);
                self.keys_of_id.set(id, HolderKeys::Many(keys));
            }
            Some(HolderKeys::Many(keys)) => keys.add(key),
        }
    }

    fn remove(&mut self, key: &str) {
        let Some(id) = self.id_of_key.delete(key) else {
            return;
        };
        self.version += 1;
        let last = match self.keys_of_id.get_mut(&id) {
            Some(HolderKeys::Many(keys)) => {
                keys.delete(key);
                if keys.len() == 1 {
                    keys.iter().next().map(str::to_owned)
                } else {
                    return;
                }
            }
            _ => None,
        };
        match last {
            Some(last) => self.keys_of_id.set(&id, HolderKeys::One(last)),
            None => {
                self.keys_of_id.delete(&id);
            }
        }
    }
}

/// The handles of the blocks [`get_block_by_id`] sees, and how to find the
/// live block of an id among them: through the index (`scan: None`), or
/// among the holders a scan of a committed state found.
struct HandleView {
    table: HandleTable,
    scan: Option<OrderedMap<Vec<NodeKey>>>,
    /// `frozenLookup`'s memo: the block of each id looked up, per frozen
    /// state it was asked about (JS's `WeakMap`, keyed weakly here too).
    frozen: FrozenLookups,
}

type FrozenLookups = RefCell<
    Vec<(
        Weak<crate::editor_state::EditorStateInner>,
        HashMap<String, Option<NodeRef>>,
    )>,
>;

/// A generated id given to a block created in the same update, and the id
/// it gave up for it, if it was a copy that yielded to the block it was
/// copied from.
struct ProvisionalId {
    id: String,
    yielded: Option<String>,
}

/// `BlockIdRuntime`: per-editor state, the init result of
/// [`block_id_extension`] (opaque, `unknown` in JS).
pub struct BlockIdRuntime {
    config: BlockIdConfig,
    index: RefCell<BlockIdIndex>,
    /// Whether `index` is being maintained (the extension is enabled).
    active: Cell<bool>,
    /// Whether a committed state with content has been seen since the
    /// extension was registered. Until then, updates load content, and get
    /// positional ids.
    has_content: Cell<bool>,
    /// The ids of the blocks that `placed_state` leaves unnamed.
    placed: RefCell<Rc<OrderedMap<String>>>,
    placed_state: RefCell<Option<EditorState>>,
    /// What earlier transform passes of the update that is building
    /// `provisional_state` decided.
    provisional: RefCell<OrderedMap<ProvisionalId>>,
    /// The blocks named by any pass of that update.
    provisional_seen: RefCell<OrderedSet>,
    provisional_state: RefCell<Option<EditorState>>,
    /// The handles of the indexed ids, for the index version they were built
    /// at.
    handles: RefCell<Option<(Rc<HandleView>, u64)>>,
    /// The scan of the last committed state for handles while disabled.
    scan: RefCell<Option<(Rc<HandleView>, EditorState)>>,
}

impl BlockIdRuntime {
    fn new(config: BlockIdConfig) -> Self {
        BlockIdRuntime {
            config,
            index: RefCell::new(BlockIdIndex::default()),
            active: Cell::new(false),
            has_content: Cell::new(false),
            placed: RefCell::new(Rc::new(OrderedMap::new())),
            placed_state: RefCell::new(None),
            provisional: RefCell::new(OrderedMap::new()),
            provisional_seen: RefCell::new(OrderedSet::new()),
            provisional_state: RefCell::new(None),
            handles: RefCell::new(None),
            scan: RefCell::new(None),
        }
    }

    fn is_identified_block(&self, node: &NodeRef) -> bool {
        (self.config.is_identified_block.0)(node)
    }

    /// `runtime.$getBlockById`.
    fn get_block_by_id(&self, id: &str) -> Result<Option<NodeRef>, LexicalError> {
        let mut found: Option<NodeRef> = None;
        if !self.active.get() {
            // Not indexed while disabled: the first holder in document order.
            walk((), &mut |node, _| {
                if found.is_none()
                    && self.is_identified_block(node)
                    && get_block_id(node)?.as_deref() == Some(id)
                {
                    found = Some(node.clone());
                }
                Ok(())
            })?;
            return Ok(found);
        }
        // Several holders only until the repair update: the one it keeps.
        for node in live_holders(self, id, &[])? {
            if found
                .as_ref()
                .is_none_or(|f| compare_document_order(&node, f) < 0)
            {
                found = Some(node);
            }
        }
        Ok(found)
    }
}

/// Visits every node below the root except text, in document order (a
/// host's slot values, in canonical slot order, before its children). Each
/// visit returns the context passed to the visits of the node's contents.
fn walk<T: Clone>(
    context: T,
    visit: &mut dyn FnMut(&NodeRef, &T) -> Result<T, LexicalError>,
) -> Result<(), LexicalError> {
    let mut stack: Vec<(NodeRef, T)> = Vec::new();
    let push_contents = |stack: &mut Vec<(NodeRef, T)>, node: &NodeRef, inner: T| {
        // Pushed in reverse, so that they pop in document order.
        if node.is_element() {
            let mut child = node.get_last_child();
            while let Some(current) = child {
                if !current.is_text() {
                    stack.push((current.clone(), inner.clone()));
                }
                child = current.get_previous_sibling();
            }
        }
        for name in get_slot_names(node).iter().rev() {
            if let Some(slot) = get_slot(node, name) {
                stack.push((slot, inner.clone()));
            }
        }
    };
    push_contents(&mut stack, &get_root(), context);
    while let Some((node, outer)) = stack.pop() {
        let inner = visit(&node, &outer)?;
        push_contents(&mut stack, &node, inner);
    }
    Ok(())
}

/// An identified block's place among its parent frame's blocks, and a
/// counter for the blocks inside it. The root's frame has no parent.
struct Frame {
    parent: Option<Rc<Frame>>,
    index: usize,
    next: Cell<usize>,
}

fn place_of(frame: &Frame) -> String {
    match &frame.parent {
        Some(parent) if parent.parent.is_some() => format!("{}.{}", place_of(parent), frame.index),
        _ => frame.index.to_string(),
    }
}

/// Positional ids for the blocks without an id that `is_target` selects,
/// the same on every read and every replica: SHA-256 of the document key and
/// the block's place, its path among identified blocks. A taken id is
/// retried with `#n` appended to the place. This walks and hashes the whole
/// document.
fn place_blocks(
    runtime: &BlockIdRuntime,
    is_target: &dyn Fn(&NodeRef) -> bool,
) -> Result<OrderedMap<String>, LexicalError> {
    let mut taken: HashSet<String> = HashSet::new();
    let mut targets: Vec<(NodeKey, Rc<Frame>)> = Vec::new();
    let root_frame = Rc::new(Frame {
        parent: None,
        index: 0,
        next: Cell::new(0),
    });
    walk(root_frame, &mut |node, frame| {
        if !runtime.is_identified_block(node) {
            return Ok(frame.clone());
        }
        let inner = Rc::new(Frame {
            parent: Some(frame.clone()),
            index: frame.next.get(),
            next: Cell::new(0),
        });
        frame.next.set(frame.next.get() + 1);
        if let Some(id) = get_block_id(node)? {
            taken.insert(id);
        } else if is_target(node) {
            targets.push((node.get_key().to_owned(), inner.clone()));
        }
        Ok(inner)
    })?;
    let mut placed = OrderedMap::new();
    if targets.is_empty() {
        return Ok(placed);
    }
    let key = document_key();
    for (node_key, frame) in targets {
        let path = place_of(&frame);
        let mut id = derive_positional_id(&key, &path);
        let mut attempt = 1;
        while taken.contains(&id) {
            id = derive_positional_id(&key, &format!("{path}#{attempt}"));
            attempt += 1;
        }
        taken.insert(id.clone());
        placed.set(&node_key, id);
    }
    Ok(placed)
}

/// Names the holders of a repeated id that every replica must name alike:
/// the earliest in document order keeps `id`, and the k-th later one gets
/// `derive_duplicate_id(id, k)`, retried with `#n` appended to `k` while
/// taken. Costs O(holders), whatever the document's size.
fn rename_later_holders(
    runtime: &BlockIdRuntime,
    id: &str,
    holders: &[NodeRef],
    used: &mut OrderedSet,
) -> Result<Vec<(NodeKey, String)>, LexicalError> {
    let mut holders = holders.to_vec();
    holders.sort_by(|a, b| compare_document_order(a, b).cmp(&0));
    let mut renamed = Vec::new();
    for (k, holder) in holders.iter().enumerate().skip(1) {
        let mut next = derive_duplicate_id(id, &k.to_string());
        let mut n = 1;
        while is_id_taken(runtime, &next, used, None)? {
            next = derive_duplicate_id(id, &format!("{k}#{n}"));
            n += 1;
        }
        used.add(&next);
        renamed.push((holder.get_key().to_owned(), next));
    }
    Ok(renamed)
}

/// The ids that the blocks `keys` of the active state should have, for
/// those it leaves unnamed: positional ids for blocks without an id,
/// derived ids for the later holders of a repeated id.
fn name_unnamed(
    runtime: &BlockIdRuntime,
    keys: &[NodeKey],
) -> Result<OrderedMap<String>, LexicalError> {
    let mut unnamed = OrderedSet::new();
    let mut repeated = OrderedSet::new();
    for key in keys {
        if let Some(node) = get_node_by_key(key)
            && is_live_identified(runtime, &node)
        {
            match get_block_id(&node)? {
                Some(id) => repeated.add(&id),
                None => unnamed.add(key),
            }
        }
    }
    let mut named = if unnamed.is_empty() {
        OrderedMap::new()
    } else {
        place_blocks(runtime, &|node| unnamed.contains(node.get_key()))?
    };
    let mut used = OrderedSet::new();
    for (_, id) in named.iter() {
        used.add(id);
    }
    // In code unit order, so that a retry after a collision is decided
    // alike everywhere.
    let mut ids: Vec<&str> = repeated.iter().collect();
    ids.sort_by(|a, b| crate::utf16::cmp(a, b));
    for id in ids {
        let holders = live_holders(runtime, id, &[])?;
        if holders.len() > 1 {
            for (key, next) in rename_later_holders(runtime, id, &holders, &mut used)? {
                named.set(&key, next);
            }
        }
    }
    Ok(named)
}

/// An attached node that the predicate identifies.
fn is_live_identified(runtime: &BlockIdRuntime, node: &NodeRef) -> bool {
    node.is_attached() && runtime.is_identified_block(node)
}

/// Attached identified blocks of the active state that carry `id`.
fn live_holders(
    runtime: &BlockIdRuntime,
    id: &str,
    extra_keys: &[NodeKey],
) -> Result<Vec<NodeRef>, LexicalError> {
    let mut seen: HashSet<NodeKey> = HashSet::new();
    let mut result = Vec::new();
    let indexed = runtime.index.borrow().holders(id);
    for key in indexed.iter().chain(extra_keys) {
        if !seen.insert(key.clone()) {
            continue;
        }
        if let Some(node) = get_node_by_key(key)
            && is_live_identified(runtime, &node)
            && get_block_id(&node)?.as_deref() == Some(id)
        {
            result.push(node);
        }
    }
    Ok(result)
}

/// Whether `id` is in `used` or carried by an identified block of the
/// active state other than the block `except`.
fn is_id_taken(
    runtime: &BlockIdRuntime,
    id: &str,
    used: &OrderedSet,
    except: Option<&str>,
) -> Result<bool, LexicalError> {
    Ok(used.contains(id)
        || live_holders(runtime, id, &[])?
            .iter()
            .any(|holder| Some(holder.get_key()) != except))
}

/// The parent of `node`, or the host of the slot that it occupies.
fn get_container(node: &NodeRef) -> Option<NodeRef> {
    node.get_parent().or_else(|| get_slot_host(node))
}

/// Document order of two nodes of the active state (see [`walk`]), the
/// tie-break that every replica must resolve alike: node keys cannot be
/// used for it, because each editor numbers the nodes that it receives in
/// its own order. Costs O(depth) plus a walk over the siblings between the
/// two branches.
fn compare_document_order(a: &NodeRef, b: &NodeRef) -> i64 {
    if a == b {
        return 0;
    }
    // Container key → the node below it on the way up from `a`.
    let mut below: HashMap<NodeKey, NodeRef> = HashMap::new();
    let mut child = a.clone();
    let mut container = get_container(a);
    while let Some(current) = container {
        below.insert(current.get_key().to_owned(), child);
        container = get_container(&current);
        child = current;
    }
    if below.contains_key(b.get_key()) {
        // `b` is an ancestor of `a`.
        return 1;
    }
    let mut b_child = b.clone();
    let mut container = get_container(b);
    while let Some(current) = container {
        if current == *a {
            // `a` is an ancestor of `b`.
            return -1;
        }
        let Some(a_child) = below.get(current.get_key()) else {
            container = get_container(&current);
            b_child = current;
            continue;
        };
        // `current` is the closest common one; compare the two branches.
        let a_slot = get_slot_name_within_host(a_child);
        let b_slot = get_slot_name_within_host(&b_child);
        if a_slot.is_some() || b_slot.is_some() {
            let (Some(a_slot), Some(b_slot)) = (a_slot.as_ref(), b_slot.as_ref()) else {
                // Slots come before children.
                return if a_slot.is_none() { 1 } else { -1 };
            };
            let names = get_slot_names(&current);
            let position = |name: &String| {
                names
                    .iter()
                    .position(|n| n == name)
                    .map_or(-1, |i| i as i64)
            };
            return position(a_slot) - position(b_slot);
        }
        let mut na = Some(a_child.clone());
        let mut nb = Some(b_child.clone());
        while let (Some(x), Some(y)) = (&na, &nb) {
            if x == &b_child {
                return -1;
            } else if y == a_child {
                return 1;
            }
            let (next_a, next_b) = (x.get_next_sibling(), y.get_next_sibling());
            na = next_a;
            nb = next_b;
        }
        // The walk from `a` reached the end first: `b` comes before it.
        return if na.is_none() { 1 } else { -1 };
    }
    // Not in the same tree (cannot happen for attached nodes).
    compare_keys(a, b)
}

/// Creation order. Node keys come from a counter that only increases within
/// one process, so this orders nodes that one local update created; it is
/// used only for decisions that one editor makes and other replicas then
/// receive.
fn compare_keys(a: &NodeRef, b: &NodeRef) -> i64 {
    let number = |node: &NodeRef| node.get_key().parse::<i64>().unwrap_or(0);
    number(a) - number(b)
}

fn make_fresh_id(runtime: &BlockIdRuntime, used: &mut OrderedSet) -> Result<String, LexicalError> {
    for _ in 0..MAX_ID_ATTEMPTS {
        let id = (runtime.config.generate_id.0)();
        if matches_block_id_pattern(&id) && !is_id_taken(runtime, &id, used, None)? {
            used.add(&id);
            return Ok(id);
        }
    }
    Err(LexicalError::new(format!(
        "BlockIdExtension: generateId returned {MAX_ID_ATTEMPTS} ids in a row that were invalid or already in use"
    )))
}

/// A version of a node in a state's node map.
fn node_in(state: &EditorState, key: &str) -> Option<Rc<dyn LexicalNode>> {
    state.0.node_map.borrow().get(key).cloned()
}

/// Whether the pending update is a load: the first content since the
/// extension was registered, or a state that replaced the previous one
/// wholesale (setEditorState).
fn is_load(editor: &Editor, runtime: &BlockIdRuntime) -> bool {
    let prev_first = node_in(&editor.get_editor_state(), "root")
        .and_then(|root| root.element().and_then(|e| e.first.clone()));
    !runtime.has_content.get() || prev_first.is_some_and(|first| get_node_by_key(&first).is_none())
}

/// What one run of [`assign_block_ids`] decided so far.
struct Assignment<'a> {
    runtime: &'a BlockIdRuntime,
    prev_state: EditorState,
    placed_before: Rc<OrderedMap<String>>,
    used: OrderedSet,
    /// New key → key of the previous-state block it continues.
    lineage: OrderedMap<NodeKey>,
}

impl Assignment<'_> {
    fn prev_node(&self, key: &str) -> Option<Rc<dyn LexicalNode>> {
        node_in(&self.prev_state, key)
    }

    /// The id a block had in the previous state, counting the one its
    /// readers derived when that state left it unnamed.
    fn prev_id_of_key(&self, key: &str) -> Result<Option<String>, LexicalError> {
        if let Some(placed) = self.placed_before.get(key) {
            return Ok(Some(placed.clone()));
        }
        self.prev_node(key)
            .map_or(Ok(None), |node| id_of_version(&*node))
    }

    /// Whether the block was identified with `id` in the previous state. An
    /// id left on a block that was not identified is not reserved.
    fn held_before(&self, key: &str, id: &str) -> bool {
        self.placed_before.get(key).map(String::as_str) == Some(id)
            || self
                .runtime
                .index
                .borrow()
                .id_of_key
                .get(key)
                .map(String::as_str)
                == Some(id)
    }

    fn inherit(&mut self, node: &NodeRef, id: &str, from_key: &str) -> Result<(), LexicalError> {
        self.lineage.set(node.get_key(), from_key.to_owned());
        self.runtime.provisional.borrow_mut().delete(node.get_key());
        self.used.add(id);
        if get_block_id(node)?.as_deref() != Some(id) {
            set_block_id(node, Some(id.to_owned()))?;
        }
        Ok(())
    }

    fn is_taken(&self, id: &str, except: Option<&str>) -> Result<bool, LexicalError> {
        is_id_taken(self.runtime, id, &self.used, except)
    }
}

/// Removed in this update: still in the pending node map, detached. A key
/// missing altogether belongs to a state replaced wholesale.
fn is_removed(key: Option<&str>) -> bool {
    key.and_then(get_node_by_key)
        .is_some_and(|node| !node.is_attached())
}

/// Whether an earlier pass of this update generated the block's id because
/// it had none, so lineage found now may still replace it. (A copy that
/// yielded its id is not: it is decided as a holder of that id.)
fn is_provisional(runtime: &BlockIdRuntime, node: &NodeRef) -> Result<bool, LexicalError> {
    let entry = runtime
        .provisional
        .borrow()
        .get(node.get_key())
        .map(|entry| (entry.yielded.is_none(), entry.id.clone()));
    Ok(match entry {
        Some((true, id)) => get_block_id(node)?.as_deref() == Some(id.as_str()),
        _ => false,
    })
}

/// The RootNode transform, which Lexical applies after every other
/// transform of the same pass. `keys` holds the identified blocks marked
/// dirty in this update. The rules are those of the documentation ("How ids
/// follow edits", "Uniqueness", "Documents stored without ids"):
///
/// 0. A block that the previous state left unnamed takes the id every
///    reader of that state derived for it.
/// 1. A new block continues a block of the previous state (and takes its
///    id): one that took the children of a removed block, or all the
///    children of a block now empty (Enter at the start, which re-keys the
///    emptied block), or one that sits where a removed block sat (or in its
///    slot). For the children, a copy that yields its id counts as a block
///    without one.
/// 2. Blocks without an id get one: positional in a load, generated
///    otherwise.
/// 3. Of the blocks that share an id, the one that was identified with it
///    in the previous state keeps it. Several such blocks are named by
///    document order and derived ids, alike on every replica. Otherwise a
///    block that continues a removed holder of the id keeps it, else the
///    first one created; the others get generated ids, in creation order.
fn assign_block_ids(
    editor: &Editor,
    runtime: &BlockIdRuntime,
    keys: Vec<NodeKey>,
) -> Result<(), LexicalError> {
    if keys.is_empty() {
        return Ok(());
    }
    if has_update_tag(COLLABORATION_TAG) || has_update_tag(HISTORIC_TAG) {
        // Changes made during updates tagged COLLABORATION_TAG or
        // HISTORIC_TAG are treated as replayed and are not propagated by
        // whoever applied them: the follow-up repair update names these
        // blocks.
        return Ok(());
    }
    let blocks: Vec<NodeRef> = keys
        .iter()
        .filter_map(|key| get_node_by_key(key))
        .filter(|node| is_live_identified(runtime, node))
        .collect();
    if blocks.is_empty() {
        return Ok(());
    }
    let mut block_keys = OrderedSet::new();
    for node in &blocks {
        block_keys.add(node.get_key());
    }
    let pending_state = editor.pending();
    let same_update = match (&*runtime.provisional_state.borrow(), &pending_state) {
        (Some(a), Some(b)) => a.ptr_eq(b),
        (None, None) => true,
        _ => false,
    };
    if !same_update {
        *runtime.provisional.borrow_mut() = OrderedMap::new();
        *runtime.provisional_seen.borrow_mut() = OrderedSet::new();
        *runtime.provisional_state.borrow_mut() = pending_state;
    }
    for key in block_keys.iter() {
        runtime.provisional_seen.borrow_mut().add(key);
    }
    // A copy that yielded its id in an earlier pass takes it back when no
    // block holds it any more (the original was merged into it since); the
    // rules below then decide between several such copies.
    for node in &blocks {
        let entry = runtime
            .provisional
            .borrow()
            .get(node.get_key())
            .and_then(|entry| Some((entry.id.clone(), entry.yielded.clone()?)));
        if let Some((id, yielded)) = entry
            && get_block_id(node)?.as_deref() == Some(id.as_str())
        {
            let seen: Vec<NodeKey> = runtime
                .provisional_seen
                .borrow()
                .iter()
                .map(str::to_owned)
                .collect();
            if live_holders(runtime, &yielded, &seen)?.is_empty() {
                runtime.provisional.borrow_mut().delete(node.get_key());
                set_block_id(node, Some(yielded))?;
            }
        }
    }
    let prev_state = editor.get_editor_state();
    let placed_before = if runtime
        .placed_state
        .borrow()
        .as_ref()
        .is_some_and(|s| s.ptr_eq(&prev_state))
    {
        runtime.placed.borrow().clone()
    } else {
        Rc::new(OrderedMap::new())
    };
    let mut a = Assignment {
        runtime,
        prev_state,
        placed_before,
        used: OrderedSet::new(),
        lineage: OrderedMap::new(),
    };
    let load = is_load(editor, runtime);

    // 0. Blocks the previous state left unnamed.
    if !a.placed_before.is_empty() {
        for node in &blocks {
            let key = node.get_key();
            if let Some(placed) = a.placed_before.get(key).cloned() {
                let id = get_block_id(node)?;
                let prev_id = a.prev_node(key).map_or(Ok(None), |n| id_of_version(&*n))?;
                if (id.is_none() || id == prev_id) && !a.is_taken(&placed, Some(key))? {
                    a.inherit(node, &placed, key)?;
                }
            }
        }
    }

    // Whether a new block takes lineage by children as if it had no id: its
    // id `id` was generated by an earlier pass of this update, or it is a
    // copy that yields `id` in step 3 to a block that was identified with
    // it before the update and still holds it.
    let is_id_less = |a: &Assignment, node: &NodeRef, id: &str| -> Result<bool, LexicalError> {
        if runtime
            .provisional
            .borrow()
            .get(node.get_key())
            .is_some_and(|entry| entry.id == id)
        {
            return Ok(true);
        }
        Ok(live_holders(runtime, id, &[])?
            .iter()
            .any(|holder| holder != node && a.held_before(holder.get_key(), id)))
    };

    // 1a. Lineage by children. The first child decides, unless it is new:
    // then the first child that comes from a removed block does.
    let new_blocks: Vec<NodeRef> = blocks
        .iter()
        .filter(|node| a.prev_node(node.get_key()).is_none())
        .cloned()
        .collect();
    for node in &new_blocks {
        if !node.is_element() {
            continue;
        }
        let first_child = node.get_first_child();
        let mut source_key: Option<NodeKey> = None;
        let mut child = first_child.clone();
        let mut is_first = true;
        while let Some(current) = child {
            if source_key.is_some() {
                break;
            }
            if let Some(parent_key) = a
                .prev_node(current.get_key())
                .and_then(|prev_child| prev_child.base().parent.clone())
                && (is_first || is_removed(Some(&parent_key)))
            {
                source_key = Some(parent_key);
            }
            is_first = false;
            child = current.get_next_sibling();
        }
        let Some(source_key) = source_key else {
            continue;
        };
        let own_id = get_block_id(node)?;
        let Some(id) = a.prev_id_of_key(&source_key)? else {
            continue;
        };
        if let Some(own_id) = &own_id
            && *own_id != id
            && !is_id_less(&a, node, own_id)?
        {
            continue;
        }
        if is_removed(Some(&source_key)) {
            // Replaced, e.g. by setBlocksType, or merged into this block.
            if !a.is_taken(&id, Some(&source_key))? {
                a.inherit(node, &id, &source_key)?;
            }
            continue;
        }
        let source = get_node_by_key(&source_key);
        let prev_source_first = a
            .prev_node(&source_key)
            .and_then(|n| n.element().map(|e| e.first.clone()));
        if let (Some(first_child), Some(source), Some(prev_first)) =
            (&first_child, &source, prev_source_first)
            && source.is_element()
            && source.is_attached()
            && source.get_children_size() == 0
            && prev_first.as_deref() == Some(first_child.get_key())
            && get_block_id(source)?.as_deref() == Some(id.as_str())
            && !a.used.contains(&id)
        {
            // All of the content moved on (Enter at the start): the id
            // follows it.
            let fresh = make_fresh_id(runtime, &mut a.used)?;
            set_block_id(source, Some(fresh))?;
            a.inherit(node, &id, &source_key)?;
        }
    }

    // 1b. Lineage by position, for the remaining new blocks without an id.
    let mut resolved = OrderedSet::new();
    for start in &new_blocks {
        let mut stack: Vec<NodeRef> = vec![start.clone()];
        while let Some(node) = stack.last().cloned() {
            let key = node.get_key().to_owned();
            if resolved.contains(&key) {
                stack.pop();
                continue;
            }
            let prev_sibling = node.get_previous_sibling();
            if let Some(prev_sibling) = &prev_sibling
                && a.prev_node(prev_sibling.get_key()).is_none()
                && block_keys.contains(prev_sibling.get_key())
                && !resolved.contains(prev_sibling.get_key())
            {
                // A run of replaced blocks is matched against the removed
                // run in order: resolve the new block before this one first.
                stack.push(prev_sibling.clone());
                continue;
            }
            stack.pop();
            resolved.add(&key);
            if a.lineage.contains(&key)
                || (get_block_id(&node)?.is_some() && !is_provisional(runtime, &node)?)
            {
                continue;
            }
            let removed_key: Option<NodeKey> = if let Some(host) = get_slot_host(&node) {
                let prev_host = a.prev_node(host.get_key());
                match (prev_host, get_slot_name_within_host(&node)) {
                    (Some(prev_host), Some(name)) => {
                        slots_field(&*prev_host).and_then(|slots| slots.get(&name).cloned())
                    }
                    _ => None,
                }
            } else if let Some(prev_sibling) = &prev_sibling {
                let anchor_key = if a.prev_node(prev_sibling.get_key()).is_some() {
                    Some(prev_sibling.get_key().to_owned())
                } else {
                    a.lineage.get(prev_sibling.get_key()).cloned()
                };
                anchor_key
                    .and_then(|k| a.prev_node(&k))
                    .and_then(|anchor| anchor.base().next.clone())
            } else {
                node.get_parent()
                    .and_then(|parent| a.prev_node(parent.get_key()))
                    .and_then(|prev_parent| prev_parent.element().and_then(|e| e.first.clone()))
            };
            if let Some(removed_key) = removed_key
                && is_removed(Some(&removed_key))
                && let Some(id) = a.prev_id_of_key(&removed_key)?
                && !a.is_taken(&id, Some(&removed_key))?
            {
                a.inherit(&node, &id, &removed_key)?;
            }
        }
    }

    // 2 and 3.
    let mut candidates_by_id: OrderedMap<Vec<NodeKey>> = OrderedMap::new();
    let mut need_fresh: Vec<NodeRef> = Vec::new();
    for node in &blocks {
        match get_block_id(node)? {
            Some(id) => match candidates_by_id.get_mut(&id) {
                Some(list) => list.push(node.get_key().to_owned()),
                None => candidates_by_id.set(&id, vec![node.get_key().to_owned()]),
            },
            None => need_fresh.push(node.clone()),
        }
    }
    let mut repeated: OrderedMap<Vec<NodeRef>> = OrderedMap::new();
    let mut yielded_of: OrderedMap<String> = OrderedMap::new();
    for (id, keys_with_id) in candidates_by_id.iter() {
        let holders = live_holders(runtime, id, keys_with_id)?;
        if holders.len() > 1 {
            repeated.set(id, holders);
        }
    }
    // In code unit order, like name_unnamed.
    let mut repeated_ids: Vec<String> = repeated.keys().map(str::to_owned).collect();
    repeated_ids.sort_by(|x, y| crate::utf16::cmp(x, y));
    for id in &repeated_ids {
        let holders = repeated.get(id).expect("a repeated id").clone();
        a.used.add(id);
        let existing: Vec<NodeRef> = holders
            .iter()
            .filter(|holder| a.held_before(holder.get_key(), id))
            .cloned()
            .collect();
        let shared = if existing.len() > 1 {
            Some(existing.clone())
        } else if load {
            Some(holders.clone())
        } else {
            None
        };
        let keepers: Vec<NodeRef> = if let Some(shared) = shared {
            for (key, next) in rename_later_holders(runtime, id, &shared, &mut a.used)? {
                let node = get_node_by_key(&key).expect("a holder of the active state");
                set_block_id(&node, Some(next))?;
            }
            shared
        } else if existing.len() == 1 {
            existing
        } else {
            // Every holder got the id in this local update.
            let mut continuing = Vec::new();
            for node in &holders {
                if let Some(from) = a.lineage.get(node.get_key()).cloned()
                    && is_removed(Some(&from))
                    && a.prev_id_of_key(&from)?.as_deref() == Some(id.as_str())
                {
                    continuing.push(node.clone());
                }
            }
            let pool = if continuing.is_empty() {
                &holders
            } else {
                &continuing
            };
            let first = pool[1..].iter().fold(&pool[0], |best, node| {
                if compare_keys(node, best) < 0 {
                    node
                } else {
                    best
                }
            });
            vec![first.clone()]
        };
        for node in &holders {
            if !keepers.contains(node) {
                need_fresh.push(node.clone());
                yielded_of.set(node.get_key(), id.clone());
            }
        }
    }
    let mut random = need_fresh.clone();
    if load && !need_fresh.is_empty() {
        // Every reader that loads the same content names its blocks alike.
        let mut fresh = OrderedSet::new();
        for node in &need_fresh {
            fresh.add(node.get_key());
        }
        let placed = place_blocks(runtime, &|node| fresh.contains(node.get_key()))?;
        random = Vec::new();
        for node in &need_fresh {
            let key = node.get_key();
            match placed.get(key) {
                Some(id) if !a.is_taken(id, Some(key))? => {
                    a.used.add(id);
                    set_block_id(node, Some(id.clone()))?;
                }
                _ => random.push(node.clone()),
            }
        }
    }
    random.sort_by(|x, y| compare_keys(x, y).cmp(&0));
    for node in &random {
        let id = make_fresh_id(runtime, &mut a.used)?;
        set_block_id(node, Some(id.clone()))?;
        if a.prev_node(node.get_key()).is_none() {
            runtime.provisional.borrow_mut().set(
                node.get_key(),
                ProvisionalId {
                    id,
                    yielded: yielded_of.get(node.get_key()).cloned(),
                },
            );
        }
    }
    Ok(())
}

/// Brings the index up to date with a committed state: incrementally from a
/// commit's dirty nodes, or rebuilt when `changes` is `None` or does not say
/// which nodes changed. Returns the keys of identified blocks that lack an
/// id or share one, which need a repair update.
fn sync_index(
    runtime: &BlockIdRuntime,
    editor_state: &EditorState,
    changes: Option<&UpdateListenerPayload>,
) -> Result<Vec<NodeKey>, LexicalError> {
    let mut needs_repair: Vec<NodeKey> = Vec::new();
    let mut touched_ids = OrderedSet::new();
    let mut visit = |node: &NodeRef| -> Result<(), LexicalError> {
        let key = node.get_key();
        if is_live_identified(runtime, node) {
            match get_block_id(node)? {
                Some(id) => {
                    runtime.index.borrow_mut().set(key, &id);
                    touched_ids.add(&id);
                }
                None => {
                    runtime.index.borrow_mut().remove(key);
                    needs_repair.push(key.to_owned());
                }
            }
        } else {
            runtime.index.borrow_mut().remove(key);
        }
        Ok(())
    };

    let incremental = changes.filter(|changes| {
        // Only the root is dirty: the state was replaced with
        // setEditorState (an undo or redo, for instance), which Lexical
        // reconciles in full too.
        !(changes.dirty_leaves.is_empty()
            && changes.dirty_elements.len() == 1
            && changes.dirty_elements[0].0 == "root")
    });
    if let Some(changes) = incremental {
        let prev_state = &changes.prev_editor_state;
        let remove_subtree = |key: &str| {
            let mut stack: Vec<NodeKey> = vec![key.to_owned()];
            while let Some(k) = stack.pop() {
                if node_in(editor_state, &k).is_some() {
                    // Moved elsewhere; it is dirty itself if it matters.
                    continue;
                }
                runtime.index.borrow_mut().remove(&k);
                let Some(prev_node) = node_in(prev_state, &k) else {
                    continue;
                };
                if let Some(element) = prev_node.element() {
                    let mut child = element.first.clone();
                    while let Some(current) = child {
                        child = node_in(prev_state, &current).and_then(|n| n.base().next.clone());
                        stack.push(current);
                    }
                }
                if let Some(slots) = slots_field(&*prev_node) {
                    stack.extend(slots.values().cloned());
                }
            }
        };
        let mut visit_key = |key: &str| -> Result<(), LexicalError> {
            match node_in(editor_state, key) {
                Some(node) => {
                    if node.text().is_none() {
                        visit(&NodeRef::new(key))?;
                    }
                }
                None => remove_subtree(key),
            }
            Ok(())
        };
        for (key, intentional) in &changes.dirty_elements {
            // An element marked dirty only because a descendant changed keeps
            // its id, position and identity, unless it was also created or
            // written in this update: Lexical downgrades the flag when a
            // later transform pass touches only a descendant, but never
            // clones for such a mark.
            let same_version = match (node_in(editor_state, key), node_in(prev_state, key)) {
                (Some(x), Some(y)) => std::ptr::addr_eq(Rc::as_ptr(&x), Rc::as_ptr(&y)),
                _ => false,
            };
            if *intentional || !same_version {
                visit_key(key)?;
            }
        }
        for key in &changes.dirty_leaves {
            visit_key(key)?;
        }
    } else {
        runtime.index.borrow_mut().clear();
        walk((), &mut |node, _| visit(node))?;
    }

    for id in touched_ids.iter() {
        if runtime.index.borrow().is_repeated(id) {
            let holders = live_holders(runtime, id, &[])?;
            if holders.len() > 1 {
                needs_repair.extend(holders.iter().map(|h| h.get_key().to_owned()));
            }
        }
    }
    Ok(needs_repair)
}

/// Notes the ids every reader gives the blocks a committed state leaves
/// unnamed, for the update that repairs it (or any other based on it).
fn sync(
    runtime: &BlockIdRuntime,
    editor_state: &EditorState,
    changes: Option<&UpdateListenerPayload>,
) -> Result<Vec<NodeKey>, LexicalError> {
    let repairs = sync_index(runtime, editor_state, changes)?;
    if !runtime.has_content.get() && !get_root().is_empty() {
        runtime.has_content.set(true);
    }
    *runtime.placed.borrow_mut() = Rc::new(if repairs.is_empty() {
        OrderedMap::new()
    } else {
        name_unnamed(runtime, &repairs)?
    });
    *runtime.placed_state.borrow_mut() = Some(editor_state.clone());
    Ok(repairs)
}

fn schedule_repair(
    editor: &Editor,
    runtime: &Rc<BlockIdRuntime>,
    keys: Vec<NodeKey>,
) -> Result<(), LexicalError> {
    let with_transforms = runtime.config.repair_transforms;
    let runtime = runtime.clone();
    editor.update(
        move || {
            let mut attached = Vec::new();
            for key in &keys {
                if let Some(node) = get_node_by_key(key)
                    && node.is_attached()
                {
                    if with_transforms {
                        node.mark_dirty();
                    } else if !attached.contains(key) {
                        attached.push(key.clone());
                    }
                }
            }
            if !with_transforms {
                // What the root transform would do with these blocks marked
                // dirty.
                let editor = get_active_editor().expect("an update has an editor");
                assign_block_ids(&editor, &runtime, attached)?;
            }
            Ok(())
        },
        // Discrete, so it commits on its own right after the commit that
        // needed it: batched into a later update tagged COLLABORATION_TAG,
        // its changes would be treated as replayed and not propagated.
        UpdateOptions {
            discrete: true,
            tags: vec![HISTORY_MERGE_TAG.to_owned(), BLOCK_ID_REPAIR_TAG.to_owned()],
            skip_transforms: !with_transforms,
            ..UpdateOptions::default()
        },
    )
}

fn upgrade(editor: &Weak<crate::editor::EditorInner>) -> Option<Editor> {
    editor.upgrade().map(Editor)
}

fn register_block_ids(
    editor: &Editor,
    runtime: &Rc<BlockIdRuntime>,
) -> Result<Unregister, LexicalError> {
    let candidates: Rc<RefCell<OrderedSet>> = Rc::new(RefCell::new(OrderedSet::new()));

    runtime.index.borrow_mut().clear();
    runtime.active.set(true);
    runtime.has_content.set(false);
    let current_state = editor.get_editor_state();
    let initial_repairs = read_editor_state(Some(editor), &current_state, || {
        sync(runtime, &current_state, None)
    })?;

    let mut cleanups: Vec<Unregister> = Vec::new();
    {
        let weak = editor.weak();
        let runtime = runtime.clone();
        cleanups.push(editor.register_update_listener(move |payload| {
            let Some(editor) = upgrade(&weak) else {
                return;
            };
            // A listener has no error channel: a NodeState key collision,
            // the only failure, fails the update that wrote the state too.
            let Ok(repairs) = read_editor_state(Some(&editor), &payload.editor_state, || {
                sync(&runtime, &payload.editor_state, Some(payload))
            }) else {
                return;
            };
            if !repairs.is_empty() {
                if !payload.tags.iter().any(|t| t == BLOCK_ID_REPAIR_TAG) {
                    // Queued: it runs after the listeners.
                    let _ = schedule_repair(&editor, &runtime, repairs);
                } else {
                    crate::warnings::console_warn(
                        "BlockIdExtension: a repair update left blocks without a unique id; is $isIdentifiedBlock stable?",
                    );
                }
            }
        }));
    }
    // Registering a transform marks every node of that type dirty, so the
    // first update assigns ids to a document that was opened without them.
    for class in editor.registered_nodes_in_order() {
        let node_type = class.node_type();
        if node_type == "root" || node_type == "linebreak" || (class.is_text)() {
            continue;
        }
        let candidates = candidates.clone();
        let runtime = runtime.clone();
        cleanups.push(editor.register_node_transform(node_type, move |node| {
            if runtime.is_identified_block(node) {
                candidates.borrow_mut().add(node.get_key());
            } else if get_block_id(node)?.is_some()
                && node_in(
                    &get_active_editor()
                        .expect("a transform has an editor")
                        .get_editor_state(),
                    node.get_key(),
                )
                .is_none()
                && !has_update_tag(COLLABORATION_TAG)
                && !has_update_tag(HISTORIC_TAG)
            {
                // A block that is not identified has no id: a copy created
                // where it is not one (a list pasted into a list item, which
                // nests it) does not keep the id of the block it was copied
                // from.
                set_block_id(node, None)?;
            }
            Ok(())
        }));
    }
    {
        let candidates = candidates.clone();
        let runtime = runtime.clone();
        cleanups.push(editor.register_node_transform("root", move |_| {
            let editor = get_active_editor().expect("a transform has an editor");
            // A load is named once a transform pass changes nothing in the
            // document before this transform (the root is not dirty again),
            // so that positional ids are derived from the document the load
            // ends with (equal adjacent lists merged, NodeStructureExtension's
            // repairs made), not from a state in between. A dirty node
            // outside the document does not run the root transform again, so
            // it must not defer naming.
            if editor.0.dirty.borrow().elements.contains("root")
                && !candidates.borrow().is_empty()
                && is_load(&editor, &runtime)
            {
                return Ok(());
            }
            let keys: Vec<NodeKey> = std::mem::take(&mut *candidates.borrow_mut())
                .iter()
                .map(str::to_owned)
                .collect();
            assign_block_ids(&editor, &runtime, keys)
        }));
    }
    if !initial_repairs.is_empty() {
        schedule_repair(editor, runtime, initial_repairs)?;
    }
    let runtime = runtime.clone();
    Ok(Box::new(move || {
        for cleanup in cleanups.into_iter().rev() {
            cleanup();
        }
        runtime.active.set(false);
        runtime.index.borrow_mut().clear();
        *runtime.placed.borrow_mut() = Rc::new(OrderedMap::new());
        *runtime.placed_state.borrow_mut() = None;
        *runtime.provisional.borrow_mut() = OrderedMap::new();
        *runtime.provisional_seen.borrow_mut() = OrderedSet::new();
        *runtime.provisional_state.borrow_mut() = None;
        *runtime.handles.borrow_mut() = None;
        *runtime.scan.borrow_mut() = None;
        *candidates.borrow_mut() = OrderedSet::new();
    }))
}

/// `$getEditor()`.
fn active_editor() -> Result<Editor, LexicalError> {
    get_active_editor().ok_or_else(|| {
        LexicalError::new(
            "Unable to find an active editor. This method can only be used synchronously during the callback of editor.update(), editor.read(), or editor.getEditorState().read(..., {editor}).",
        )
    })
}

fn get_handle_view(
    editor: &Editor,
    runtime: &BlockIdRuntime,
) -> Result<Rc<HandleView>, LexicalError> {
    if runtime.active.get() {
        // The ids of the committed state. Ids of blocks that the active
        // update removed are still in it: they can only make a handle
        // longer, and get_block_by_id, which verifies against the active
        // state, skips them.
        let version = runtime.index.borrow().version;
        if let Some((view, built_at)) = &*runtime.handles.borrow()
            && *built_at == version
        {
            return Ok(view.clone());
        }
        let table = build_handle_table(runtime.index.borrow().keys_of_id.keys());
        let view = Rc::new(HandleView {
            table,
            scan: None,
            frozen: RefCell::default(),
        });
        *runtime.handles.borrow_mut() = Some((view.clone(), version));
        return Ok(view);
    }
    // Not indexed while disabled: a scan of the last committed state, as the
    // index would be, cached until the next commit. Lookups are verified
    // against the active state, as the index's are.
    let committed = editor.get_editor_state();
    if let Some((view, state)) = &*runtime.scan.borrow()
        && state.ptr_eq(&committed)
    {
        return Ok(view.clone());
    }
    let keys_of_id = read_editor_state(Some(editor), &committed, || {
        let mut keys: OrderedMap<Vec<NodeKey>> = OrderedMap::new();
        walk((), &mut |node, _| {
            if runtime.is_identified_block(node)
                && let Some(id) = get_block_id(node)?
            {
                match keys.get_mut(&id) {
                    Some(list) => list.push(node.get_key().to_owned()),
                    None => keys.set(&id, vec![node.get_key().to_owned()]),
                }
            }
            Ok(())
        })?;
        Ok::<_, LexicalError>(keys)
    })?;
    let table = build_handle_table(keys_of_id.keys());
    let view = Rc::new(HandleView {
        table,
        scan: Some(keys_of_id),
        frozen: RefCell::default(),
    });
    *runtime.scan.borrow_mut() = Some((view.clone(), committed));
    Ok(view)
}

/// The live block of `id` among the blocks `view` sees, remembered per id
/// for each frozen state it is asked about (a read of a committed state;
/// JS's `frozenLookup`), so that reading the handle of every holder of a
/// repeated id costs the holders once, not once per holder. While the
/// active state is not frozen (an update), it looks every time.
fn lookup(
    runtime: &BlockIdRuntime,
    view: &HandleView,
    id: &str,
) -> Result<Option<NodeRef>, LexicalError> {
    let Some(state) = internal_get_active_editor_state().filter(|s| s.0.read_only.get()) else {
        return lookup_now(runtime, view, id);
    };
    {
        let mut frozen = view.frozen.borrow_mut();
        frozen.retain(|(state, _)| state.strong_count() > 0);
        if let Some(node) = frozen
            .iter()
            .find(|(s, _)| std::ptr::eq(s.as_ptr(), Rc::as_ptr(&state.0)))
            .and_then(|(_, found)| found.get(id))
        {
            return Ok(node.clone());
        }
    }
    let node = lookup_now(runtime, view, id)?;
    let mut frozen = view.frozen.borrow_mut();
    match frozen
        .iter_mut()
        .find(|(s, _)| std::ptr::eq(s.as_ptr(), Rc::as_ptr(&state.0)))
    {
        Some((_, found)) => {
            found.insert(id.to_owned(), node.clone());
        }
        None => frozen.push((
            Rc::downgrade(&state.0),
            HashMap::from([(id.to_owned(), node.clone())]),
        )),
    }
    Ok(node)
}

/// `$lookup(id)`, unremembered.
fn lookup_now(
    runtime: &BlockIdRuntime,
    view: &HandleView,
    id: &str,
) -> Result<Option<NodeRef>, LexicalError> {
    let Some(scan) = &view.scan else {
        return runtime.get_block_by_id(id);
    };
    // The first holder of the id in the active state, in document order, as
    // get_block_by_id finds it.
    let mut found: Option<NodeRef> = None;
    for key in scan.get(id).into_iter().flatten() {
        if let Some(node) = get_node_by_key(key)
            && is_live_identified(runtime, &node)
            && get_block_id(&node)?.as_deref() == Some(id)
            && found
                .as_ref()
                .is_none_or(|f| compare_document_order(&node, f) < 0)
        {
            found = Some(node);
        }
    }
    Ok(found)
}

fn get_handle_of(runtime: &BlockIdRuntime, node: &NodeRef) -> Result<Option<String>, LexicalError> {
    if !is_live_identified(runtime, node) {
        return Ok(None);
    }
    let Some(id) = get_block_id(node)? else {
        return Ok(None);
    };
    let view = get_handle_view(&active_editor()?, runtime)?;
    // Only a block that its id finds has a handle that finds it: not one
    // named after the last commit (its handle is not in the table), nor a
    // later holder of a repeated id, nor a node of another state.
    let Some(handle) = view.table.handles.get(&id) else {
        return Ok(None);
    };
    Ok((lookup(runtime, &view, &id)?.as_ref() == Some(node)).then(|| handle.clone()))
}

fn get_all_handles(runtime: &BlockIdRuntime) -> Result<Vec<(String, String)>, LexicalError> {
    let view = get_handle_view(&active_editor()?, runtime)?;
    let mut result = Vec::new();
    for (id, handle) in view.table.handles.iter() {
        if lookup(runtime, &view, id)?.is_some() {
            result.push((id.to_owned(), handle.clone()));
        }
    }
    Ok(result)
}

/// What [`resolve_in_read`] found: the id the handle names, or why it names
/// none.
enum ReadResolution {
    Id(String),
    Result(BlockHandleResult),
}

/// The block id that `handle` names among the ids of a read (see
/// [`BlockHandleOptions::read`]), or why it names none.
fn resolve_in_read(read: &[(String, String)], handle: &str) -> ReadResolution {
    let read_get = |id: &str| read.iter().find(|(k, _)| k == id).map(|(_, v)| v);
    if handle.len() > HANDLE_KEY_LENGTH {
        return if read_get(handle).is_some() {
            ReadResolution::Id(handle.to_owned())
        } else {
            ReadResolution::Result(not_found(handle))
        };
    }
    if handle.len() < MIN_BLOCK_HANDLE_LENGTH {
        return ReadResolution::Result(invalid(handle, BlockHandleInvalidReason::TooShort));
    }
    let mut matches: Vec<(String, &str)> = Vec::new();
    for (id, _) in read {
        let key = handle_key(id);
        if key.starts_with(handle) {
            matches.push((key, id));
        }
    }
    if matches.is_empty() {
        return ReadResolution::Result(not_found(handle));
    }
    if matches.len() > 1 {
        matches.sort_by(|(x, _), (y, _)| crate::utf16::cmp(x, y));
        let candidates = matches
            .iter()
            .take(MAX_HANDLE_CANDIDATES)
            .map(|(key, id)| read_get(id).cloned().unwrap_or_else(|| key.clone()))
            .collect();
        return ReadResolution::Result(BlockHandleResult::Ambiguous {
            handle: handle.to_owned(),
            candidates,
        });
    }
    ReadResolution::Id(matches[0].1.to_owned())
}

fn found_or_not(node: Option<NodeRef>, handle: &str) -> BlockHandleResult {
    match node {
        Some(node) => BlockHandleResult::Found { node },
        None => not_found(handle),
    }
}

fn resolve_handle(
    runtime: &BlockIdRuntime,
    handle: &str,
    options: Option<&BlockHandleOptions>,
) -> Result<BlockHandleResult, LexicalError> {
    // JS first refuses a handle that is not a string; a Rust handle is one.
    if !matches_block_id_pattern(handle) {
        return Ok(invalid(handle, BlockHandleInvalidReason::Malformed));
    }
    if let Some(read) = options.and_then(|o| o.read.as_ref()) {
        // What the handle named in the read, if that block still exists: a
        // block added since cannot take the handle, nor make it ambiguous.
        return Ok(match resolve_in_read(read, handle) {
            ReadResolution::Result(result) => result,
            ReadResolution::Id(id) => found_or_not(runtime.get_block_by_id(&id)?, handle),
        });
    }
    if handle.len() > HANDLE_KEY_LENGTH {
        // Longer than any handle: a whole id.
        return Ok(found_or_not(runtime.get_block_by_id(handle)?, handle));
    }
    if handle.len() < MIN_BLOCK_HANDLE_LENGTH {
        // Never a handle: say so, rather than "not found".
        return Ok(invalid(handle, BlockHandleInvalidReason::TooShort));
    }
    let view = get_handle_view(&active_editor()?, runtime)?;
    let mut found: Option<NodeRef> = None;
    let mut candidates: Vec<String> = Vec::new();
    for id in ids_with_prefix(&view.table, handle) {
        if let Some(node) = lookup(runtime, &view, &id)? {
            found = Some(node);
            if candidates.len() < MAX_HANDLE_CANDIDATES {
                candidates.push(handle_in_table(&view.table, &id));
            }
        }
    }
    if candidates.len() > 1 {
        return Ok(BlockHandleResult::Ambiguous {
            handle: handle.to_owned(),
            candidates,
        });
    }
    Ok(found_or_not(found, handle))
}

/// The type of [`block_id_extension`].
pub type BlockIdExtensionType = LexicalExtension<BlockIdConfig, BlockIdOutput, BlockIdRuntime>;

fn make() -> Rc<BlockIdExtensionType> {
    define_extension(LexicalExtension {
        // Where the editor also enforces declared node structure, register
        // after it, so that blocks are named on the structure its
        // normalization leaves.
        peer_dependencies: vec![declare_peer_dependency::<crate::json::Object>(
            "@lexical/NodeStructure",
            None,
        )],
        init: Some(Rc::new(
            |_: &mut InitialEditorConfig, config: &BlockIdConfig, _: &ExtensionInitState| {
                Ok(BlockIdRuntime::new(config.clone()))
            },
        )),
        build: Some(Rc::new(
            |_: &Editor, config: &BlockIdConfig, state: &ExtensionBuildState<BlockIdRuntime>| {
                let runtime = state.get_init_result()?;
                let (r1, r2, r3, r4) = (
                    runtime.clone(),
                    runtime.clone(),
                    runtime.clone(),
                    runtime.clone(),
                );
                Ok(BlockIdOutput {
                    disabled: signal(config.disabled),
                    get_block_by_id: Rc::new(move |id| r1.get_block_by_id(id)),
                    get_block_handle: Rc::new(move |node| get_handle_of(&r2, node)),
                    get_block_handles: Rc::new(move || get_all_handles(&r3)),
                    resolve_block_handle: Rc::new(move |handle, options| {
                        resolve_handle(&r4, handle, options)
                    }),
                    is_identified_block: config.is_identified_block.clone(),
                })
            },
        )),
        register: Some(Rc::new(
            |editor: &Editor,
             _: &BlockIdConfig,
             state: &ExtensionRegisterState<BlockIdRuntime, BlockIdOutput>| {
                let output = state.get_output().expect("BlockIdExtension has a build");
                let runtime = state.get_init_result()?;
                let editor = editor.weak();
                Ok(effect(move || {
                    if output.disabled.value() {
                        return None;
                    }
                    let editor = upgrade(&editor)?;
                    // The registration fails only on a NodeState key
                    // collision while it indexes the document.
                    register_block_ids(&editor, &runtime).ok()
                }))
            },
        )),
        ..LexicalExtension::new("@lexical/extension/BlockId", BlockIdConfig::default())
    })
}

thread_local! {
    static BLOCK_ID_EXTENSION: Rc<BlockIdExtensionType> = make();
}

/// `BlockIdExtension` (`@experimental`): gives blocks stable ids, stored as
/// NodeState ([`block_id_state`]), so that code outside the editor (an AI
/// agent, a server pipeline, a comment system) can address a block by id
/// across sessions, replicas, and serializations. See the module
/// documentation. Showing blocks to a model? Use handles
/// ([`get_block_handle`], [`resolve_block_handle`]).
///
/// The init result is the editor's [`BlockIdRuntime`], opaque outside this
/// module (`unknown` in JS). It peers on `@lexical/NodeStructure`, so that
/// where an editor has it, it registers first.
pub fn block_id_extension() -> Rc<BlockIdExtensionType> {
    BLOCK_ID_EXTENSION.with(Rc::clone)
}
