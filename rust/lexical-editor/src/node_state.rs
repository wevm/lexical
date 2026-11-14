//! `LexicalNodeState.ts`: NodeState, the ad-hoc data an app keeps on any
//! node beside the properties of its class. A [`StateConfig`] made with
//! [`create_state`] names a key and how its value parses from JSON, writes
//! back and compares; [`get_state`] and [`set_state`] read and write it on a
//! node, and the node's JSON carries the values that aren't their default
//! under `"$"` (or at the top level, for a state its class declares flat).
//! Keys no config has read stay as their JSON, and are written back as they
//! came.
//!
//! As in JS, a node version shares its NodeState with the version it was
//! cloned from until one of them writes it, which copies it (parsing the
//! JSON of the keys a config has read on its node type). JS tells the
//! version that owns a NodeState by object identity; here a node version is
//! its key and the editor state it was made writable in.
//!
//! Not ported: the `SerializationSchema` builders as `parse` functions (and
//! with them `canonicalValue`'s `arraySetValue` form, `derived` and
//! `schema`), `$getStateChange`'s use in `updateDOM`, which comes with the
//! DOM, and the `Object.prototype` check on flat keys (rust/PARITY.md).

use crate::LexicalError;
use crate::error::invariant;
use crate::json::{Object, Value, array_index};
use crate::lexical_utils::{get_writable, mutate};
use crate::node::{LexicalNode, NodeKey, NodeRef};
use crate::updates::{error_on_read_only, get_active_editor_state, get_registry_editor};
use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::rc::Rc;

/// A value a state can hold without saying how it serializes and compares:
/// one that JSON writes as it is (JS's default `unparse`), compared with
/// `Object.is` (JS's default `isEqual`).
pub trait StateValue: Clone + 'static {
    /// The value as JSON.
    fn to_json(&self) -> Value;
    /// `Object.is(self, other)`.
    fn object_is(&self, other: &Self) -> bool;
}

impl StateValue for f64 {
    fn to_json(&self) -> Value {
        Value::Number(*self)
    }
    fn object_is(&self, other: &Self) -> bool {
        (self.is_nan() && other.is_nan()) || self.to_bits() == other.to_bits()
    }
}

impl StateValue for bool {
    fn to_json(&self) -> Value {
        Value::Bool(*self)
    }
    fn object_is(&self, other: &Self) -> bool {
        self == other
    }
}

impl StateValue for String {
    fn to_json(&self) -> Value {
        Value::String(self.clone())
    }
    fn object_is(&self, other: &Self) -> bool {
        self == other
    }
}

/// `None` is `null`.
impl<T: StateValue> StateValue for Option<T> {
    fn to_json(&self) -> Value {
        self.as_ref().map_or(Value::Null, StateValue::to_json)
    }
    fn object_is(&self, other: &Self) -> bool {
        match (self, other) {
            (None, None) => true,
            (Some(a), Some(b)) => a.object_is(b),
            _ => false,
        }
    }
}

/// A state's `isEqual`: whether two of its values are the same value.
pub type StateIsEqual<V> = Rc<dyn Fn(&V, &V) -> bool>;

/// `StateValueConfig`: how a state's value parses from JSON (`parse`, which
/// gives the default for `undefined`), writes back (`unparse`) and compares
/// (`isEqual`), and whether `$copyNode` resets it (`resetOnCopyNode`).
pub struct StateValueConfig<V> {
    pub parse: Rc<dyn Fn(&Value) -> V>,
    pub unparse: Rc<dyn Fn(&V) -> Value>,
    pub is_equal: StateIsEqual<V>,
    pub reset_on_copy_node: bool,
}

impl<V> Clone for StateValueConfig<V> {
    fn clone(&self) -> Self {
        Self {
            parse: self.parse.clone(),
            unparse: self.unparse.clone(),
            is_equal: self.is_equal.clone(),
            reset_on_copy_node: self.reset_on_copy_node,
        }
    }
}

impl<V: StateValue> StateValueConfig<V> {
    /// `{parse}`, with JS's defaults for the rest: the value as JSON,
    /// `Object.is`, and kept by a copy.
    pub fn new(parse: impl Fn(&Value) -> V + 'static) -> Self {
        Self {
            parse: Rc::new(parse),
            unparse: Rc::new(StateValue::to_json),
            is_equal: Rc::new(StateValue::object_is),
            reset_on_copy_node: false,
        }
    }
}

struct StateConfigInner<V> {
    key: String,
    config: StateValueConfig<V>,
    /// `defaultValue`, `parse(undefined)`, computed once: every node without
    /// a value of its own reads this one.
    default_value: Rc<dyn Any>,
}

/// `StateConfig`: what [`create_state`] returns, for [`get_state`] and
/// [`set_state`]. Clones are the same config: nodes tell configs apart by
/// identity, as JS does.
pub struct StateConfig<V>(Rc<StateConfigInner<V>>);

impl<V> Clone for StateConfig<V> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<V> fmt::Debug for StateConfig<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StateConfig({:?})", self.0.key)
    }
}

impl<V: Clone + 'static> StateConfig<V> {
    /// `key`: the key the state serializes under.
    pub fn key(&self) -> &str {
        &self.0.key
    }

    /// `parse(value)`; `Value::Undefined` gives the default.
    pub fn parse(&self, value: &Value) -> V {
        (self.0.config.parse)(value)
    }

    /// `unparse(value)`.
    pub fn unparse(&self, value: &V) -> Value {
        (self.0.config.unparse)(value)
    }

    /// `isEqual(a, b)`.
    pub fn is_equal(&self, a: &V, b: &V) -> bool {
        (self.0.config.is_equal)(a, b)
    }

    /// `defaultValue`.
    pub fn default_value(&self) -> V {
        downcast::<V>(&self.0.default_value)
    }

    /// `resetOnCopyNode`.
    pub fn reset_on_copy_node(&self) -> bool {
        self.0.config.reset_on_copy_node
    }

    /// This config without its value type (`AnyStateConfig`).
    pub fn erased(&self) -> AnyStateConfig {
        AnyStateConfig(self.0.clone())
    }
}

fn downcast<V: Clone + 'static>(value: &Rc<dyn Any>) -> V {
    value
        .downcast_ref::<V>()
        .expect("a state value of its config's type")
        .clone()
}

/// What a node's NodeState needs of a config whose value type it doesn't
/// know.
trait ErasedStateConfig {
    fn key(&self) -> &str;
    fn parse_any(&self, value: &Value) -> Rc<dyn Any>;
    fn unparse_any(&self, value: &dyn Any) -> Value;
    fn is_equal_any(&self, a: &dyn Any, b: &dyn Any) -> bool;
    fn default_any(&self) -> Rc<dyn Any>;
    fn reset_on_copy_node(&self) -> bool;
}

impl<V: 'static> ErasedStateConfig for StateConfigInner<V> {
    fn key(&self) -> &str {
        &self.key
    }
    fn parse_any(&self, value: &Value) -> Rc<dyn Any> {
        Rc::new((self.config.parse)(value))
    }
    fn unparse_any(&self, value: &dyn Any) -> Value {
        (self.config.unparse)(
            value
                .downcast_ref::<V>()
                .expect("a value of the config's type"),
        )
    }
    fn is_equal_any(&self, a: &dyn Any, b: &dyn Any) -> bool {
        match (a.downcast_ref::<V>(), b.downcast_ref::<V>()) {
            (Some(a), Some(b)) => (self.config.is_equal)(a, b),
            _ => false,
        }
    }
    fn default_any(&self) -> Rc<dyn Any> {
        self.default_value.clone()
    }
    fn reset_on_copy_node(&self) -> bool {
        self.config.reset_on_copy_node
    }
}

/// `AnyStateConfig`: a [`StateConfig`] of any value type.
#[derive(Clone)]
pub struct AnyStateConfig(Rc<dyn ErasedStateConfig>);

impl AnyStateConfig {
    /// The config's key.
    pub fn key(&self) -> &str {
        self.0.key()
    }

    /// Whether `self` and `other` are the same config (`===`).
    pub fn is(&self, other: &AnyStateConfig) -> bool {
        std::ptr::addr_eq(Rc::as_ptr(&self.0), Rc::as_ptr(&other.0))
    }
}

impl fmt::Debug for AnyStateConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AnyStateConfig({:?})", self.key())
    }
}

/// `createState(key, {parse})`: a state with JS's default `unparse` and
/// `isEqual` ([`StateValue`]).
pub fn create_state<V: StateValue>(
    key: &str,
    parse: impl Fn(&Value) -> V + 'static,
) -> StateConfig<V> {
    create_state_with(key, StateValueConfig::new(parse))
}

/// `createState(key, valueConfig)`.
pub fn create_state_with<V: 'static>(key: &str, config: StateValueConfig<V>) -> StateConfig<V> {
    let default_value: Rc<dyn Any> = Rc::new((config.parse)(&Value::Undefined));
    StateConfig(Rc::new(StateConfigInner {
        key: key.to_owned(),
        config,
        default_value,
    }))
}

/// `RequiredNodeStateConfig`: a state a node class declares in its
/// `$config`'s `stateConfigs` ([`LexicalNode::state_configs`]), which its
/// type knows before any node reads it; a `flat` one serializes at the top
/// level of the node's JSON instead of under `"$"`.
#[derive(Clone, Debug)]
pub struct RequiredNodeStateConfig {
    pub(crate) state_config: AnyStateConfig,
    pub(crate) flat: bool,
}

impl RequiredNodeStateConfig {
    /// `stateConfig` or `{stateConfig, flat}`.
    pub fn new<V: Clone + 'static>(state_config: &StateConfig<V>, flat: bool) -> Self {
        Self {
            state_config: state_config.erased(),
            flat,
        }
    }
}

/// `SharedNodeState`: the configs a node type knows in an editor, by key
/// (`sharedConfigMap`), and the keys it serializes flat (`flatKeys`), shared
/// by every node of the type.
#[derive(Debug, Default)]
pub(crate) struct SharedNodeState {
    pub(crate) shared_config_map: RefCell<HashMap<String, AnyStateConfig>>,
    flat_keys: Vec<String>,
}

/// `createSharedNodeState(nodeConfig)`.
pub(crate) fn create_shared_node_state(configs: Vec<RequiredNodeStateConfig>) -> SharedNodeState {
    let shared = SharedNodeState::default();
    let mut flat_keys = Vec::new();
    for required in configs {
        let key = required.state_config.key().to_owned();
        if required.flat && !flat_keys.contains(&key) {
            flat_keys.push(key.clone());
        }
        shared
            .shared_config_map
            .borrow_mut()
            .insert(key, required.state_config);
    }
    SharedNodeState {
        flat_keys,
        ..shared
    }
}

/// `UNSAFE_STATE_KEYS`.
const UNSAFE_STATE_KEYS: [&str; 3] = ["__proto__", "constructor", "prototype"];

/// An `UnknownStateRecord`: JSON values by key, in a JS object's order. The
/// values are shared, as JS shares the objects, so two records compare
/// their objects and arrays by identity (`===`).
#[derive(Clone, Debug, Default)]
pub(crate) struct UnknownState(Vec<(String, Rc<Value>)>);

impl UnknownState {
    pub(crate) fn get(&self, key: &str) -> Option<&Rc<Value>> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub(crate) fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// `record[key] = value`, keeping a JS object's key order.
    fn insert(&mut self, key: &str, value: Rc<Value>) {
        if let Some(entry) = self.0.iter_mut().find(|(k, _)| k == key) {
            entry.1 = value;
            return;
        }
        let at = match array_index(key) {
            Some(index) => self
                .0
                .iter()
                .position(|(k, _)| array_index(k).is_none_or(|i| i > index))
                .unwrap_or(self.0.len()),
            None => self.0.len(),
        };
        self.0.insert(at, (key.to_owned(), value));
    }

    fn remove(&mut self, key: &str) {
        self.0.retain(|(k, _)| k != key);
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&str, &Rc<Value>)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v))
    }
}

/// `a === b` for two JSON values: objects and arrays by identity.
pub(crate) fn json_strict_equals(a: Option<&Rc<Value>>, b: Option<&Rc<Value>>) -> bool {
    let undefined = |v: Option<&Rc<Value>>| v.is_none_or(|v| v.is_undefined());
    match (a, b) {
        _ if undefined(a) && undefined(b) => true,
        (Some(a), Some(b)) => match (&**a, &**b) {
            (Value::Object(_), Value::Object(_)) | (Value::Array(_), Value::Array(_)) => {
                Rc::ptr_eq(a, b)
            }
            (Value::Number(x), Value::Number(y)) => x == y,
            (Value::Object(_) | Value::Array(_), _) | (_, Value::Object(_) | Value::Array(_)) => {
                false
            }
            (x, y) => x == y,
        },
        _ => false,
    }
}

/// A node version, as a NodeState knows the one it belongs to (JS's
/// `state.node`): its key, and the id of the editor state it was made
/// writable (or created) in.
pub(crate) type NodeVersion = (NodeKey, u64);

/// The version of `node` (this version of it).
pub(crate) fn version_of(node: &dyn LexicalNode) -> NodeVersion {
    let base = node.base();
    (base.key.clone(), base.written_in)
}

/// `NodeState`: the state values of a node version.
pub(crate) struct NodeState {
    /// `node`: the version this state belongs to; another one copies it
    /// before writing.
    node: NodeVersion,
    /// `knownState`: parsed values by config, in the order they came.
    known_state: Vec<(AnyStateConfig, Rc<dyn Any>)>,
    /// `unknownState`: values still as JSON.
    pub(crate) unknown_state: Option<UnknownState>,
    pub(crate) shared_node_state: Rc<SharedNodeState>,
    /// `size`: the keys of both, counted once.
    pub(crate) size: usize,
}

/// A node's NodeState, shared by the versions that haven't written it.
pub(crate) type NodeStateRef = Rc<RefCell<NodeState>>;

impl fmt::Debug for NodeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeState")
            .field("node", &self.node)
            .field(
                "known_state",
                &self
                    .known_state
                    .iter()
                    .map(|(c, _)| c.key())
                    .collect::<Vec<_>>(),
            )
            .field("unknown_state", &self.unknown_state)
            .field("size", &self.size)
            .finish()
    }
}

/// `computeSize(sharedConfigMap, unknownState, knownState)`.
fn compute_size(
    shared_config_map: &HashMap<String, AnyStateConfig>,
    unknown_state: Option<&UnknownState>,
    known_state: &[(AnyStateConfig, Rc<dyn Any>)],
) -> usize {
    let mut size = known_state.len();
    for (k, _) in unknown_state.into_iter().flat_map(UnknownState::iter) {
        let shared_config = shared_config_map.get(k);
        if shared_config.is_none_or(|c| !known_state.iter().any(|(k, _)| k.is(c))) {
            size += 1;
        }
    }
    size
}

impl NodeState {
    fn new(
        node: NodeVersion,
        shared_node_state: Rc<SharedNodeState>,
        unknown_state: Option<UnknownState>,
        known_state: Vec<(AnyStateConfig, Rc<dyn Any>)>,
        size: Option<usize>,
    ) -> Self {
        let computed_size = compute_size(
            &shared_node_state.shared_config_map.borrow(),
            unknown_state.as_ref(),
            &known_state,
        );
        // JS checks these in DEV builds.
        debug_assert!(size.is_none_or(|s| s == computed_size));
        Self {
            node,
            known_state,
            unknown_state,
            shared_node_state,
            size: computed_size,
        }
    }

    /// The known value of `config`, if any.
    fn known(&self, config: &AnyStateConfig) -> Option<&Rc<dyn Any>> {
        self.known_state
            .iter()
            .find(|(c, _)| c.is(config))
            .map(|(_, v)| v)
    }

    fn has_known(&self, config: &AnyStateConfig) -> bool {
        self.known(config).is_some()
    }

    /// `knownState.set(config, value)`.
    fn set_known(&mut self, config: &AnyStateConfig, value: Rc<dyn Any>) {
        match self.known_state.iter_mut().find(|(c, _)| c.is(config)) {
            Some(entry) => entry.1 = value,
            None => self.known_state.push((config.clone(), value)),
        }
    }

    /// The keys of the known values, in their order.
    pub(crate) fn known_entries(&self) -> Vec<(AnyStateConfig, Rc<dyn Any>)> {
        self.known_state.clone()
    }

    /// `getValue(stateConfig)`: the known value, or the one parsed from the
    /// JSON, which it keeps (without changing the order `toJSON` writes),
    /// or the default.
    pub(crate) fn get_value(&mut self, config: &AnyStateConfig) -> Rc<dyn Any> {
        if let Some(known) = self.known(config) {
            return known.clone();
        }
        self.shared_node_state
            .shared_config_map
            .borrow_mut()
            .insert(config.key().to_owned(), config.clone());
        let mut parsed = config.0.default_any();
        if let Some(json_value) = self
            .unknown_state
            .as_ref()
            .and_then(|u| u.get(config.key()))
            .cloned()
        {
            if !json_value.is_undefined() {
                parsed = config.0.parse_any(&json_value);
            }
            self.set_known(config, parsed.clone());
        }
        parsed
    }

    /// `toJSON()`: the values that aren't their default, the flat keys of
    /// the node's type at the top level and the rest under `"$"`.
    pub(crate) fn to_json(&self) -> Object {
        let mut state: Object = self
            .unknown_state
            .iter()
            .flat_map(UnknownState::iter)
            .map(|(k, v)| (k, (**v).clone()))
            .collect();
        let mut flat_state = Object::new();
        for (config, v) in &self.known_state {
            if config.0.is_equal_any(&**v, &*config.0.default_any()) {
                state.remove(config.key());
            } else {
                state.insert(config.key(), config.0.unparse_any(&**v));
            }
        }
        for key in &self.shared_node_state.flat_keys {
            if let Some(value) = state.remove(key) {
                flat_state.insert(key.as_str(), value);
            }
        }
        if !state.is_empty() {
            flat_state.insert(crate::constants::NODE_STATE_KEY, Value::Object(state));
        }
        flat_state
    }

    /// `getWritable(node)`: `this` when it belongs to `node`, else a copy
    /// for it, with the JSON of the keys its type knows parsed.
    pub(crate) fn get_writable(this: &NodeStateRef, node: NodeVersion) -> NodeStateRef {
        let state = this.borrow();
        if state.node == node {
            return this.clone();
        }
        let mut next_known_state = state.known_state.clone();
        let next_unknown_state = parse_and_prune_next_unknown_state(
            &state.shared_node_state.shared_config_map.borrow(),
            &mut next_known_state,
            state.unknown_state.as_ref(),
        );
        Rc::new(RefCell::new(NodeState::new(
            node,
            state.shared_node_state.clone(),
            next_unknown_state,
            next_known_state,
            Some(state.size),
        )))
    }

    /// `resetOnCopyNode()`.
    fn reset_on_copy_node(&mut self) {
        for (config, value) in &mut self.known_state {
            if config.0.reset_on_copy_node() {
                *value = config.0.default_any();
            }
        }
    }

    /// `updateFromKnown(stateConfig, value)`.
    pub(crate) fn update_from_known(&mut self, config: &AnyStateConfig, value: Rc<dyn Any>) {
        let key = config.key();
        self.shared_node_state
            .shared_config_map
            .borrow_mut()
            .insert(key.to_owned(), config.clone());
        let in_unknown = self
            .unknown_state
            .as_ref()
            .is_some_and(|u| u.contains_key(key));
        if in_unknown {
            // The known value replaces the JSON copy. The key is already
            // counted in size.
            let unknown = self.unknown_state.as_mut().expect("checked");
            unknown.remove(key);
            if unknown.0.is_empty() {
                self.unknown_state = None;
            }
        } else if !self.has_known(config) {
            self.size += 1;
        }
        self.set_known(config, value);
    }

    /// `updateFromUnknown(k, v)`: parsed when the node's type knows the key.
    pub(crate) fn update_from_unknown(&mut self, k: &str, v: Rc<Value>) {
        if UNSAFE_STATE_KEYS.contains(&k) {
            return;
        }
        let config = self
            .shared_node_state
            .shared_config_map
            .borrow()
            .get(k)
            .cloned();
        match config {
            Some(config) => {
                let parsed = config.0.parse_any(&v);
                self.update_from_known(&config, parsed);
            }
            None => {
                let unknown = self.unknown_state.get_or_insert_default();
                if !unknown.contains_key(k) {
                    self.size += 1;
                }
                unknown.insert(k, v);
            }
        }
    }

    /// `updateFromJSON(unknownState)`: every known value back to its
    /// default, then the entries of `unknown_state` (`Object.entries`).
    fn update_from_json(&mut self, unknown_state: &Value) {
        for (config, value) in &mut self.known_state {
            *value = config.0.default_any();
        }
        self.size = self.known_state.len();
        self.unknown_state = None;
        if is_truthy(unknown_state) {
            for (k, v) in object_entries(unknown_state) {
                self.update_from_unknown(&k, v);
            }
        }
    }
}

/// Whether a JSON value is truthy in JS.
fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Undefined | Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => *n != 0.0 && !n.is_nan(),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// `Object.entries(value)` of a JSON value: an object's properties, an
/// array's or a string's (UTF-16 code units) by index, nothing for the rest.
fn object_entries(value: &Value) -> Vec<(String, Rc<Value>)> {
    match value {
        Value::Object(object) => object
            .iter()
            .map(|(k, v)| (k.to_owned(), Rc::new(v.clone())))
            .collect(),
        Value::Array(array) => array
            .iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), Rc::new(v.clone())))
            .collect(),
        Value::String(s) => s
            .encode_utf16()
            .enumerate()
            .map(|(i, unit)| {
                (
                    i.to_string(),
                    Rc::new(Value::String(String::from_utf16_lossy(&[unit]))),
                )
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// `parseAndPruneNextUnknownState(sharedConfigMap, nextKnownState,
/// unknownState)`.
fn parse_and_prune_next_unknown_state(
    shared_config_map: &HashMap<String, AnyStateConfig>,
    next_known_state: &mut Vec<(AnyStateConfig, Rc<dyn Any>)>,
    unknown_state: Option<&UnknownState>,
) -> Option<UnknownState> {
    let mut next_unknown_state: Option<UnknownState> = None;
    for (k, v) in unknown_state.into_iter().flat_map(UnknownState::iter) {
        if UNSAFE_STATE_KEYS.contains(&k) {
            continue;
        }
        match shared_config_map.get(k) {
            Some(config) => {
                if !next_known_state.iter().any(|(c, _)| c.is(config)) {
                    next_known_state.push((config.clone(), config.0.parse_any(v)));
                }
            }
            None => next_unknown_state
                .get_or_insert_default()
                .insert(k, v.clone()),
        }
    }
    next_unknown_state
}

/// `$checkCollision(node, stateConfig, state)` (a DEV check in JS, which
/// the vectors come from): another config with the same key already used
/// on the node's type.
fn check_collision(
    node: &dyn LexicalNode,
    config: &AnyStateConfig,
    state: &NodeState,
) -> Result<(), LexicalError> {
    let collision = state
        .shared_node_state
        .shared_config_map
        .borrow()
        .get(config.key())
        .cloned();
    invariant!(
        collision.is_none_or(|c| c.is(config)),
        "$setState: State key collision {} detected in {} node with type {} and key {}. Only one StateConfig with a given key should be used on a node.",
        Value::from(config.key()),
        node.get_type(),
        node.get_type(),
        node.base().key
    );
    Ok(())
}

/// `$getSharedNodeState(node)`: the node's, or its type's in the active
/// editor.
fn get_shared_node_state(node: &dyn LexicalNode) -> Result<Rc<SharedNodeState>, LexicalError> {
    if let Some(state) = &node.base().state {
        return Ok(state.borrow().shared_node_state.clone());
    }
    let editor = get_registry_editor()
        .ok_or_else(|| LexicalError::new("Unable to find an active editor."))?;
    editor.shared_node_state(node.get_type()).ok_or_else(|| {
        LexicalError::new(format!(
            "getRegisteredNode: Type {} not found",
            node.get_type()
        ))
    })
}

/// `$getWritableNodeState(node)`: makes the node writable and gives it a
/// NodeState of its own.
pub(crate) fn get_writable_node_state(node: &NodeRef) -> Result<NodeStateRef, LexicalError> {
    get_writable(node.get_key());
    let state = {
        let writable = node.get_latest();
        let version = version_of(&*writable);
        match &writable.base().state {
            Some(state) => NodeState::get_writable(state, version),
            None => Rc::new(RefCell::new(NodeState::new(
                version,
                get_shared_node_state(&*writable)?,
                None,
                Vec::new(),
                None,
            ))),
        }
    };
    mutate(node.get_key(), |n| n.base_mut().state = Some(state.clone()));
    Ok(state)
}

/// `$getState(node, stateConfig, NODE_STATE_DIRECT)` without its value type.
fn get_state_any(
    node: &dyn LexicalNode,
    config: &AnyStateConfig,
) -> Result<Rc<dyn Any>, LexicalError> {
    match &node.base().state {
        Some(state) => {
            check_collision(node, config, &state.borrow())?;
            Ok(state.borrow_mut().get_value(config))
        }
        None => Ok(config.0.default_any()),
    }
}

/// `$getState(node, stateConfig)`: the value of the state on the latest
/// version of the node, or its default. Fails when another config with the
/// same key was used on the node's type.
pub fn get_state<V: Clone + 'static>(
    node: &NodeRef,
    config: &StateConfig<V>,
) -> Result<V, LexicalError> {
    get_state_direct(&*node.get_latest(), config)
}

/// `$getState(node, stateConfig, NODE_STATE_DIRECT)`: the value on this
/// version of the node, which needs no editor state.
pub fn get_state_direct<V: Clone + 'static>(
    node: &dyn LexicalNode,
    config: &StateConfig<V>,
) -> Result<V, LexicalError> {
    Ok(downcast::<V>(&get_state_any(node, &config.erased())?))
}

/// `$getStateChange(node, prevNode, stateConfig)`: `None` when the two
/// versions hold equal values, else `(value, prevValue)`.
pub fn get_state_change<V: Clone + 'static>(
    node: &dyn LexicalNode,
    prev_node: &dyn LexicalNode,
    config: &StateConfig<V>,
) -> Result<Option<(V, V)>, LexicalError> {
    let value = get_state_direct(node, config)?;
    let prev_value = get_state_direct(prev_node, config)?;
    Ok((!config.is_equal(&value, &prev_value)).then_some((value, prev_value)))
}

/// The writing half of `$setState`.
fn write_state(
    node: &NodeRef,
    config: &AnyStateConfig,
    value: Rc<dyn Any>,
) -> Result<NodeRef, LexicalError> {
    get_writable(node.get_key());
    let state = get_writable_node_state(node)?;
    check_collision(&*node.get_latest(), config, &state.borrow())?;
    state.borrow_mut().update_from_known(config, value);
    Ok(node.clone())
}

/// `$setState(node, stateConfig, valueOrUpdater)` with an updater, without
/// its value type: nothing is written when the value is equal to the one
/// the node has.
fn set_state_any_with(
    node: &NodeRef,
    config: &AnyStateConfig,
    updater: impl FnOnce(&Rc<dyn Any>) -> Rc<dyn Any>,
) -> Result<NodeRef, LexicalError> {
    error_on_read_only();
    let prev_value = get_state_any(&*node.get_latest(), config)?;
    let value = updater(&prev_value);
    if config.0.is_equal_any(&*prev_value, &*value) {
        return Ok(node.clone());
    }
    write_state(node, config, value)
}

/// `$setState(node, stateConfig, value)`: sets the state on the node (the
/// node is written even when the value is the one it has). Fails when
/// another config with the same key was used on the node's type.
pub fn set_state<V: Clone + 'static>(
    node: &NodeRef,
    config: &StateConfig<V>,
    value: V,
) -> Result<NodeRef, LexicalError> {
    error_on_read_only();
    write_state(node, &config.erased(), Rc::new(value))
}

/// `$setState(node, stateConfig, updater)`: sets the state to what
/// `updater` makes of the node's value, unless that is equal to it, when
/// the node is left as it is.
pub fn set_state_with<V: Clone + 'static>(
    node: &NodeRef,
    config: &StateConfig<V>,
    updater: impl FnOnce(&V) -> V,
) -> Result<NodeRef, LexicalError> {
    set_state_any_with(node, &config.erased(), |prev| {
        Rc::new(updater(
            prev.downcast_ref::<V>()
                .expect("a value of the config's type"),
        ))
    })
}

/// `$updateStateFromJSON(node, serializedNode)`: the node's NodeState from
/// the `"$"` of its JSON, when it has one or the node has a state.
pub(crate) fn update_state_from_json(node: &NodeRef, json: &Object) -> Result<(), LexicalError> {
    let unknown_state = json
        .get(crate::constants::NODE_STATE_KEY)
        .cloned()
        .unwrap_or(Value::Undefined);
    if node.get_latest().base().state.is_some() || is_truthy(&unknown_state) {
        get_writable_node_state(node)?
            .borrow_mut()
            .update_from_json(&unknown_state);
    }
    Ok(())
}

/// `$applyFlatStates(node, serializedNode, flatStates)`: each flat state the
/// node's class declares, from the top level of its JSON, where it is.
pub(crate) fn apply_flat_states(node: &NodeRef, json: &Object) -> Result<(), LexicalError> {
    let flat_states: Vec<AnyStateConfig> = node
        .get_latest()
        .state_configs()
        .into_iter()
        .filter(|required| required.flat)
        .map(|required| required.state_config)
        .collect();
    for config in flat_states {
        if let Some(raw) = json.get(config.key()).filter(|v| !v.is_undefined()) {
            let parsed = config.0.parse_any(raw);
            set_state_any_with(node, &config, |_| parsed)?;
        }
    }
    Ok(())
}

/// `exportJSON(compact)`: the node's own JSON, then its NodeState.
pub(crate) fn export_json_with_state(node: &dyn LexicalNode, compact: bool) -> Object {
    let mut json = node.export_json(compact);
    if let Some(state) = &node.base().state {
        for (key, value) in state.borrow().to_json().iter() {
            json.insert(key, value.clone());
        }
    }
    json
}

/// `$cloneNodeState(from, to)`: for `to`, a new node, a copy of the
/// NodeState of the version `from` when it belongs to it, else the same.
pub(crate) fn clone_node_state(from: &dyn LexicalNode, to_key: &str) -> Option<NodeStateRef> {
    let state = from.base().state.as_ref()?;
    if state.borrow().node == version_of(from) {
        let to = (to_key.to_owned(), get_active_editor_state().0.id);
        Some(NodeState::get_writable(state, to))
    } else {
        Some(state.clone())
    }
}

/// What `$copyNode` and `afterCloneFrom` give a copy of a node with another
/// key: a copy of the NodeState, with the values of the configs that ask
/// for it reset (`resetOnCopyNodeFrom`).
pub(crate) fn copy_node_state(state: &NodeStateRef, to_key: &str) -> NodeStateRef {
    let to = (to_key.to_owned(), get_active_editor_state().0.id);
    let copy = NodeState::get_writable(state, to);
    copy.borrow_mut().reset_on_copy_node();
    copy
}

/// `hasUnequalMapEntry(keys, sourceState, otherState)`.
fn has_unequal_map_entry(
    keys: &mut HashSet<String>,
    source_state: &NodeStateRef,
    other_state: Option<&NodeStateRef>,
) -> bool {
    let known = source_state.borrow().known_entries();
    for (config, value) in known {
        if !keys.insert(config.key().to_owned()) {
            continue;
        }
        let other_value = match other_state {
            Some(other) => other.borrow_mut().get_value(&config),
            None => config.0.default_any(),
        };
        if !Rc::ptr_eq(&other_value, &value) && !config.0.is_equal_any(&*other_value, &*value) {
            return true;
        }
    }
    false
}

/// `hasUnequalRecordEntry(keys, sourceState, otherState)`.
fn has_unequal_record_entry(
    keys: &mut HashSet<String>,
    source_state: &NodeStateRef,
    other_state: Option<&NodeStateRef>,
) -> bool {
    let source = source_state.borrow();
    let other = other_state.map(|o| o.borrow());
    let other_unknown = other.as_ref().and_then(|o| o.unknown_state.as_ref());
    for (key, value) in source.unknown_state.iter().flat_map(UnknownState::iter) {
        if !keys.insert(key.to_owned()) {
            continue;
        }
        if !json_strict_equals(Some(value), other_unknown.and_then(|u| u.get(key))) {
            return true;
        }
    }
    false
}

/// `nodeStatesAreEquivalent(a, b)`: whether two text nodes with these
/// states may merge.
pub(crate) fn node_states_are_equivalent(
    a: Option<&NodeStateRef>,
    b: Option<&NodeStateRef>,
) -> bool {
    match (a, b) {
        (None, None) => return true,
        (Some(a), Some(b)) if Rc::ptr_eq(a, b) => return true,
        _ => {}
    }
    let mut keys = HashSet::new();
    !(a.is_some_and(|a| has_unequal_map_entry(&mut keys, a, b))
        || b.is_some_and(|b| has_unequal_map_entry(&mut keys, b, a))
        || a.is_some_and(|a| has_unequal_record_entry(&mut keys, a, b))
        || b.is_some_and(|b| has_unequal_record_entry(&mut keys, b, a)))
}

/// `$carryNodeState(from, to)`: gives `to`, a node that takes the place of
/// `from` with the same content, the NodeState of `from`: known values as
/// they are (as JSON when the type of `to` has another config for the
/// key), JSON as it is, defaults left out, and the keys `to` holds kept.
pub(crate) fn carry_node_state(from: &NodeRef, to: &NodeRef) -> Result<(), LexicalError> {
    let Some(from_state) = from.get_latest().base().state.clone() else {
        return Ok(());
    };
    if from_state.borrow().size == 0 {
        return Ok(());
    }
    let mut done = HashSet::new();
    if let Some(to_state) = to.get_latest().base().state.clone() {
        let to_state = to_state.borrow();
        for (config, _) in &to_state.known_state {
            done.insert(config.key().to_owned());
        }
        for (key, _) in to_state.unknown_state.iter().flat_map(UnknownState::iter) {
            done.insert(key.to_owned());
        }
    }
    let state = get_writable_node_state(to)?;
    let (known, unknown) = {
        let from_state = from_state.borrow();
        (from_state.known_entries(), from_state.unknown_state.clone())
    };
    for (config, value) in known {
        if !done.insert(config.key().to_owned()) {
            continue;
        }
        if config.0.is_equal_any(&*value, &*config.0.default_any()) {
            continue;
        }
        let mut state = state.borrow_mut();
        let own = state
            .shared_node_state
            .shared_config_map
            .borrow()
            .get(config.key())
            .cloned();
        if own.is_none_or(|own| own.is(&config)) {
            state.update_from_known(&config, value);
        } else {
            let json = Rc::new(config.0.unparse_any(&*value));
            state.update_from_unknown(config.key(), json);
        }
    }
    for (key, value) in unknown.iter().flat_map(UnknownState::iter) {
        if !done.contains(key) {
            state.borrow_mut().update_from_unknown(key, value.clone());
        }
    }
    Ok(())
}

/// The value of a known config for a JSON read, as `unparse` writes it
/// (what `registerLexicalTextEntity`'s `$carryNodeState` needs).
#[cfg(feature = "text")]
pub(crate) fn unparse(config: &AnyStateConfig, value: &Rc<dyn Any>) -> Value {
    config.0.unparse_any(&**value)
}

/// Whether `value` is the default of `config`.
#[cfg(feature = "text")]
pub(crate) fn is_default(config: &AnyStateConfig, value: &Rc<dyn Any>) -> bool {
    config.0.is_equal_any(&**value, &*config.0.default_any())
}
