//! `extension-core/types.ts`: the types of an extension and of what the
//! builder hands its phases.
//!
//! JS types an extension by its `Config`, `Output` and `Init`, and the
//! builder works on any extension (`AnyLexicalExtension`). Here an extension
//! is a [`LexicalExtension<Config, Output, Init>`] behind an `Rc`, whose
//! identity is the extension's (JS compares extensions with `===`), and the
//! builder sees it as `dyn AnyLexicalExtension`. A config names the type of
//! its overrides (JS `Partial<Config>`) through [`ExtensionConfigBase`].

use crate::json::{Object, Value};
use crate::{Editor, EditorState, LexicalError, NodeClass, Unregister};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::Rc;

/// `ExtensionConfigBase`: what an extension's config is. JS configs are
/// objects, their overrides are `Partial<Config>`, and `shallowMergeConfig`
/// merges one as `{...config, ...overrides}`. A Rust config names its
/// overrides' type and how they apply: [`Object`], the JS form, merges as
/// JS does; `()` is the config of an extension with none (JS `{}`).
pub trait ExtensionConfigBase: Clone + 'static {
    /// `Partial<Config>`.
    type Partial: Clone + 'static;

    /// `{...config, ...overrides}`, or the config itself when no override
    /// differs from it (see [`shallow_merge_config`](crate::shallow_merge_config)).
    fn shallow_merge(&self, overrides: &Self::Partial) -> Self;
}

impl ExtensionConfigBase for () {
    type Partial = ();

    fn shallow_merge(&self, _overrides: &()) -> Self {}
}

impl ExtensionConfigBase for Object {
    type Partial = Object;

    /// `shallowMergeConfig`'s body: the config when every key of the
    /// overrides holds what the config holds (an absent key holds
    /// `undefined`), otherwise `{...config, ...overrides}`. Values compare
    /// by `==` where JS's `!==` compares objects by identity, which only
    /// decides whether the result is a copy.
    fn shallow_merge(&self, overrides: &Object) -> Self {
        let same = overrides
            .iter()
            .all(|(k, v)| self.get(k).unwrap_or(&Value::Undefined) == v);
        if same {
            return self.clone();
        }
        let mut merged = self.clone();
        for (k, v) in overrides.iter() {
            merged.insert(k, v.clone());
        }
        merged
    }
}

/// `InitialEditorStateType`: how `InitialStateExtension` fills a new editor.
#[derive(Clone)]
pub enum InitialEditorStateType {
    /// `null`: leave the root as it is, empty.
    Null,
    /// An editor state serialized to a JSON string.
    Json(String),
    /// A serialized editor state (`SerializedEditorState`), already parsed.
    Serialized(Value),
    /// An editor state that has been deserialized already.
    EditorState(EditorState),
    /// A function that updates the editor, called once inside an update.
    Update(InitialEditorStateFn),
}

/// `(editor: LexicalEditor) => void`, an initial state that updates the
/// editor.
pub type InitialEditorStateFn = Rc<dyn Fn(&Editor) -> Result<(), LexicalError>>;

/// `InitialEditorConfig`: the editor configuration an extension contributes,
/// and what `init` may change. Of JS's fields, the ones the port has: the
/// node classes and `$initialEditorState`. `namespace`, `theme`, `html`,
/// `editable`, `onError`, `onWarn`, `parentEditor`, `disableEvents` and
/// `dom` are editor options a headless Rust editor doesn't have.
#[derive(Clone, Default)]
pub struct InitialEditorConfig {
    /// `nodes`.
    pub nodes: Vec<NodeClass>,
    /// `$initialEditorState`; `None` is JS's `undefined`.
    pub initial_editor_state: Option<InitialEditorStateType>,
}

/// `init` (`LexicalExtension.init`).
pub type ExtensionInitFn<Config, Init> = Rc<
    dyn Fn(&mut InitialEditorConfig, &Config, &ExtensionInitState) -> Result<Init, LexicalError>,
>;

/// `build` (`LexicalExtension.build`).
pub type ExtensionBuildFn<Config, Output, Init> =
    Rc<dyn Fn(&Editor, &Config, &ExtensionBuildState<Init>) -> Result<Output, LexicalError>>;

/// `register` and `afterRegistration` (`LexicalExtension.register`): they
/// return the clean-up function.
pub type ExtensionRegisterFn<Config, Output, Init> = Rc<
    dyn Fn(
        &Editor,
        &Config,
        &ExtensionRegisterState<Init, Output>,
    ) -> Result<Unregister, LexicalError>,
>;

/// `mergeConfig` (`LexicalExtension.mergeConfig`).
pub type ExtensionMergeConfigFn<Config> =
    Rc<dyn Fn(&Config, &<Config as ExtensionConfigBase>::Partial) -> Config>;

/// `LexicalExtension`: a composable, named unit of editor configuration
/// (nodes, the initial state) and of behavior registered once the editor
/// exists. Write one with struct update syntax over
/// [`LexicalExtension::new`], and make it with
/// [`define_extension`](crate::define_extension).
///
/// An extension without `build` has no output, and one without `init` no
/// init result (JS's `undefined`).
pub struct LexicalExtension<
    Config: ExtensionConfigBase = (),
    Output: 'static = (),
    Init: 'static = (),
> {
    /// `name`: unique in an editor.
    pub name: String,
    /// `conflictsWith`: names of extensions this one can't be built with.
    pub conflicts_with: Vec<String>,
    /// `dependencies`: other extensions, optionally with config overrides.
    pub dependencies: Vec<AnyLexicalExtensionArgument>,
    /// `peerDependencies`: extensions, by name, this one configures or
    /// reads when another extension brings them.
    pub peer_dependencies: Vec<NormalizedPeerDependency>,
    /// `config`: the default config.
    pub config: Config,
    /// `mergeConfig`; without it, [`ExtensionConfigBase::shallow_merge`].
    pub merge_config: Option<ExtensionMergeConfigFn<Config>>,
    /// `nodes`.
    pub nodes: Vec<NodeClass>,
    /// `$initialEditorState`.
    pub initial_editor_state: Option<InitialEditorStateType>,
    pub init: Option<ExtensionInitFn<Config, Init>>,
    pub build: Option<ExtensionBuildFn<Config, Output, Init>>,
    pub register: Option<ExtensionRegisterFn<Config, Output, Init>>,
    pub after_registration: Option<ExtensionRegisterFn<Config, Output, Init>>,
}

impl<Config: ExtensionConfigBase, Output: 'static, Init: 'static>
    LexicalExtension<Config, Output, Init>
{
    /// An extension with a name and a default config, and nothing else.
    pub fn new(name: impl Into<String>, config: Config) -> Self {
        Self {
            name: name.into(),
            conflicts_with: Vec::new(),
            dependencies: Vec::new(),
            peer_dependencies: Vec::new(),
            config,
            merge_config: None,
            nodes: Vec::new(),
            initial_editor_state: None,
            init: None,
            build: None,
            register: None,
            after_registration: None,
        }
    }
}

/// An extension's config, init result or output, type-erased.
pub(crate) type Erased = Rc<dyn Any>;

mod sealed {
    pub trait Sealed {}
}

/// `AnyLexicalExtension`: any [`LexicalExtension`], as the builder sees it.
/// Only `LexicalExtension` implements it.
pub trait AnyLexicalExtension: sealed::Sealed + 'static {
    fn name(&self) -> &str;
    fn conflicts_with(&self) -> &[String];
    fn dependencies(&self) -> &[AnyLexicalExtensionArgument];
    fn peer_dependencies(&self) -> &[NormalizedPeerDependency];
    fn nodes(&self) -> &[NodeClass];
    fn initial_editor_state(&self) -> Option<&InitialEditorStateType>;
    /// Whether it defines `init` (`getInitResult` requires it).
    fn has_init(&self) -> bool;
    #[doc(hidden)]
    fn merge_configs_erased(&self, configs: &[Erased]) -> Result<Erased, LexicalError>;
    #[doc(hidden)]
    fn init_erased(
        &self,
        editor_config: &mut InitialEditorConfig,
        config: &Erased,
        state: &ExtensionInitState,
    ) -> Result<Option<Erased>, LexicalError>;
    #[doc(hidden)]
    fn build_erased(
        &self,
        editor: &Editor,
        config: &Erased,
        state: &ExtensionBuildState<()>,
    ) -> Result<Option<Erased>, LexicalError>;
    #[doc(hidden)]
    fn register_erased(
        &self,
        editor: &Editor,
        config: &Erased,
        state: &ExtensionRegisterState<(), ()>,
        after: bool,
    ) -> Result<Option<Unregister>, LexicalError>;
}

fn config_of<C: 'static>(config: &Erased) -> &C {
    config
        .downcast_ref::<C>()
        .expect("an extension's merged config has its config type")
}

impl<C: ExtensionConfigBase, O: 'static, I: 'static> sealed::Sealed for LexicalExtension<C, O, I> {}

impl<C: ExtensionConfigBase, O: 'static, I: 'static> AnyLexicalExtension
    for LexicalExtension<C, O, I>
{
    fn name(&self) -> &str {
        &self.name
    }
    fn conflicts_with(&self) -> &[String] {
        &self.conflicts_with
    }
    fn dependencies(&self) -> &[AnyLexicalExtensionArgument] {
        &self.dependencies
    }
    fn peer_dependencies(&self) -> &[NormalizedPeerDependency] {
        &self.peer_dependencies
    }
    fn nodes(&self) -> &[NodeClass] {
        &self.nodes
    }
    fn initial_editor_state(&self) -> Option<&InitialEditorStateType> {
        self.initial_editor_state.as_ref()
    }
    fn has_init(&self) -> bool {
        self.init.is_some()
    }

    /// `ExtensionRep.mergeConfigs`: the default config with each override
    /// merged in, in order.
    fn merge_configs_erased(&self, configs: &[Erased]) -> Result<Erased, LexicalError> {
        let mut config = self.config.clone();
        for cfg in configs {
            let Some(cfg) = cfg.downcast_ref::<C::Partial>() else {
                return Err(LexicalError::new(format!(
                    "LexicalBuilder: a config for {} has the wrong type",
                    self.name
                )));
            };
            config = match &self.merge_config {
                Some(merge_config) => merge_config(&config, cfg),
                None => config.shallow_merge(cfg),
            };
        }
        Ok(Rc::new(config))
    }

    fn init_erased(
        &self,
        editor_config: &mut InitialEditorConfig,
        config: &Erased,
        state: &ExtensionInitState,
    ) -> Result<Option<Erased>, LexicalError> {
        let Some(init) = &self.init else {
            return Ok(None);
        };
        let result = init(editor_config, config_of::<C>(config), state)?;
        Ok(Some(Rc::new(result)))
    }

    fn build_erased(
        &self,
        editor: &Editor,
        config: &Erased,
        state: &ExtensionBuildState<()>,
    ) -> Result<Option<Erased>, LexicalError> {
        let Some(build) = &self.build else {
            return Ok(None);
        };
        let output = build(editor, config_of::<C>(config), &state.retyped())?;
        Ok(Some(Rc::new(output)))
    }

    fn register_erased(
        &self,
        editor: &Editor,
        config: &Erased,
        state: &ExtensionRegisterState<(), ()>,
        after: bool,
    ) -> Result<Option<Unregister>, LexicalError> {
        let register = if after {
            &self.after_registration
        } else {
            &self.register
        };
        let Some(register) = register else {
            return Ok(None);
        };
        register(editor, config_of::<C>(config), &state.retyped()).map(Some)
    }
}

/// `AnyLexicalExtensionArgument`: an extension, alone or with config
/// overrides (`NormalizedLexicalExtensionArgument`, which
/// [`config_extension`](crate::config_extension) makes). Every extension
/// converts into one.
#[derive(Clone)]
pub struct AnyLexicalExtensionArgument {
    pub(crate) extension: Rc<dyn AnyLexicalExtension>,
    /// Each a `Partial<Config>` of the extension.
    pub(crate) configs: Vec<Erased>,
}

impl AnyLexicalExtensionArgument {
    /// The extension.
    pub fn extension(&self) -> &Rc<dyn AnyLexicalExtension> {
        &self.extension
    }
}

impl<C: ExtensionConfigBase, O: 'static, I: 'static> From<Rc<LexicalExtension<C, O, I>>>
    for AnyLexicalExtensionArgument
{
    fn from(extension: Rc<LexicalExtension<C, O, I>>) -> Self {
        Self {
            extension,
            configs: Vec::new(),
        }
    }
}

impl<C: ExtensionConfigBase, O: 'static, I: 'static> From<&Rc<LexicalExtension<C, O, I>>>
    for AnyLexicalExtensionArgument
{
    fn from(extension: &Rc<LexicalExtension<C, O, I>>) -> Self {
        extension.clone().into()
    }
}

/// `NormalizedPeerDependency`: `[name, config?]`, from
/// [`declare_peer_dependency`](crate::declare_peer_dependency).
#[derive(Clone)]
pub struct NormalizedPeerDependency {
    pub(crate) name: String,
    /// A `Partial<Config>` of the peer.
    pub(crate) config: Option<Erased>,
}

impl NormalizedPeerDependency {
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// `LexicalExtensionDependency`: a built extension's merged config, its
/// init result and its output (`None` without `init` or `build`, JS's
/// `undefined`).
pub struct LexicalExtensionDependency<Config, Output = (), Init = ()> {
    pub config: Rc<Config>,
    pub init: Option<Rc<Init>>,
    pub output: Option<Rc<Output>>,
}

impl<C, O, I> Clone for LexicalExtensionDependency<C, O, I> {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            init: self.init.clone(),
            output: self.output.clone(),
        }
    }
}

/// A dependency as the builder keeps it, type-erased.
#[derive(Clone)]
pub(crate) struct AnyExtensionDependency {
    pub(crate) config: Erased,
    pub(crate) init: Option<Erased>,
    pub(crate) output: Option<Erased>,
}

fn wrong_type(name: &str) -> LexicalError {
    LexicalError::new(format!(
        "Extension {name} does not have the requested config, init or output type"
    ))
}

impl AnyExtensionDependency {
    pub(crate) fn typed<C: 'static, O: 'static, I: 'static>(
        &self,
        name: &str,
    ) -> Result<LexicalExtensionDependency<C, O, I>, LexicalError> {
        let config = self
            .config
            .clone()
            .downcast::<C>()
            .map_err(|_| wrong_type(name))?;
        let init = match &self.init {
            None => None,
            Some(init) => Some(init.clone().downcast::<I>().map_err(|_| wrong_type(name))?),
        };
        let output = match &self.output {
            None => None,
            Some(output) => Some(
                output
                    .clone()
                    .downcast::<O>()
                    .map_err(|_| wrong_type(name))?,
            ),
        };
        Ok(LexicalExtensionDependency {
            config,
            init,
            output,
        })
    }
}

/// What the builder's record of an extension (`ExtensionRep`) answers for
/// the phase states.
pub(crate) trait ExtensionRepAccess {
    fn extension_name(&self) -> String;
    /// `getInitDependency(dep).config`.
    fn get_init_dependency(&self, dep: &dyn AnyLexicalExtension) -> Result<Erased, LexicalError>;
    /// `getInitPeer(name)?.config`.
    fn get_init_peer(&self, name: &str) -> Result<Option<Erased>, LexicalError>;
    fn get_dependency(
        &self,
        dep: &dyn AnyLexicalExtension,
    ) -> Result<AnyExtensionDependency, LexicalError>;
    fn get_peer(&self, name: &str) -> Result<Option<AnyExtensionDependency>, LexicalError>;
    fn get_init_result(&self) -> Result<Option<Erased>, LexicalError>;
    fn get_direct_dependent_names(&self) -> Vec<String>;
    fn get_peer_name_set(&self) -> Vec<String>;
    fn get_signal(&self) -> Result<AbortSignal, LexicalError>;
    fn get_output(&self) -> Option<Erased>;
}

fn downcast_config<C: 'static>(config: Erased, name: &str) -> Result<Rc<C>, LexicalError> {
    config.downcast::<C>().map_err(|_| {
        LexicalError::new(format!(
            "Extension {name} does not have the requested config type"
        ))
    })
}

/// `ExtensionInitState`: what `init` can ask of the builder about other
/// extensions, whose configs are merged before it runs.
pub struct ExtensionInitState {
    pub(crate) rep: Rc<dyn ExtensionRepAccess>,
}

impl ExtensionInitState {
    /// `getPeer(name)?.config`: the config of the peer named `name`, if the
    /// editor has it, as a `Config`.
    pub fn get_peer<Config: 'static>(
        &self,
        name: &str,
    ) -> Result<Option<Rc<Config>>, LexicalError> {
        match self.rep.get_init_peer(name)? {
            None => Ok(None),
            Some(config) => downcast_config(config, name).map(Some),
        }
    }

    /// `getDependency(dep).config`: the config of a dependency.
    pub fn get_dependency<C: ExtensionConfigBase, O: 'static, I: 'static>(
        &self,
        dep: &Rc<LexicalExtension<C, O, I>>,
    ) -> Result<Rc<C>, LexicalError> {
        downcast_config(self.rep.get_init_dependency(dep.as_ref())?, &dep.name)
    }

    /// `getDirectDependentNames()`: the names of the extensions that depend
    /// on this one directly, in the order they were added.
    pub fn get_direct_dependent_names(&self) -> Vec<String> {
        self.rep.get_direct_dependent_names()
    }

    /// `getPeerNameSet()`: the names of this extension's peer dependencies.
    pub fn get_peer_name_set(&self) -> Vec<String> {
        self.rep.get_peer_name_set()
    }
}

/// `ExtensionBuildState`: what `build` can ask of the builder; its
/// dependencies are built before it.
pub struct ExtensionBuildState<Init> {
    pub(crate) rep: Rc<dyn ExtensionRepAccess>,
    pub(crate) _init: PhantomData<fn() -> Init>,
}

impl<Init: 'static> ExtensionBuildState<Init> {
    fn retyped<I>(&self) -> ExtensionBuildState<I> {
        ExtensionBuildState {
            rep: self.rep.clone(),
            _init: PhantomData,
        }
    }

    /// `getPeer(name)`: the peer named `name`, if the editor has it, as an
    /// extension of the given types.
    pub fn get_peer<C: 'static, O: 'static, I: 'static>(
        &self,
        name: &str,
    ) -> Result<Option<LexicalExtensionDependency<C, O, I>>, LexicalError> {
        match self.rep.get_peer(name)? {
            None => Ok(None),
            Some(dep) => dep.typed(name).map(Some),
        }
    }

    /// `getDependency(dep)`: a dependency's config, init result and output.
    pub fn get_dependency<C: ExtensionConfigBase, O: 'static, I: 'static>(
        &self,
        dep: &Rc<LexicalExtension<C, O, I>>,
    ) -> Result<LexicalExtensionDependency<C, O, I>, LexicalError> {
        self.rep.get_dependency(dep.as_ref())?.typed(&dep.name)
    }

    /// `getInitResult()`: what this extension's `init` returned.
    pub fn get_init_result(&self) -> Result<Rc<Init>, LexicalError> {
        let result = self.rep.get_init_result()?;
        result
            .and_then(|r| r.downcast::<Init>().ok())
            .ok_or_else(|| {
                LexicalError::new(format!(
                    "ExtensionRep: getInitResult() for Extension {} has another type",
                    self.rep.extension_name()
                ))
            })
    }

    pub fn get_direct_dependent_names(&self) -> Vec<String> {
        self.rep.get_direct_dependent_names()
    }

    pub fn get_peer_name_set(&self) -> Vec<String> {
        self.rep.get_peer_name_set()
    }
}

/// `ExtensionRegisterState`: what `register` and `afterRegistration` can
/// ask of the builder.
pub struct ExtensionRegisterState<Init, Output> {
    pub(crate) build: ExtensionBuildState<Init>,
    pub(crate) _output: PhantomData<fn() -> Output>,
}

impl<Init: 'static, Output: 'static> ExtensionRegisterState<Init, Output> {
    fn retyped<I, O>(&self) -> ExtensionRegisterState<I, O> {
        ExtensionRegisterState {
            build: ExtensionBuildState {
                rep: self.build.rep.clone(),
                _init: PhantomData,
            },
            _output: PhantomData,
        }
    }

    /// `getOutput()`: what this extension's `build` returned, `None`
    /// without `build`.
    pub fn get_output(&self) -> Option<Rc<Output>> {
        self.build
            .rep
            .get_output()
            .and_then(|o| o.downcast::<Output>().ok())
    }

    /// `getSignal()`: aborted when the editor's registration is disposed.
    pub fn get_signal(&self) -> Result<AbortSignal, LexicalError> {
        self.build.rep.get_signal()
    }
}

impl<Init, Output> std::ops::Deref for ExtensionRegisterState<Init, Output> {
    type Target = ExtensionBuildState<Init>;

    fn deref(&self) -> &ExtensionBuildState<Init> {
        &self.build
    }
}

/// `AbortSignal`, as the builder hands it to `register`: aborted once,
/// when the editor is disposed. Cloning it shares the signal.
#[derive(Clone, Default)]
pub struct AbortSignal(Rc<AbortSignalInner>);

#[derive(Default)]
struct AbortSignalInner {
    aborted: Cell<bool>,
    listeners: RefCell<Vec<Box<dyn FnOnce()>>>,
}

impl AbortSignal {
    /// `signal.aborted`.
    pub fn aborted(&self) -> bool {
        self.0.aborted.get()
    }

    /// `signal.addEventListener('abort', listener)`: calls `listener` when
    /// the signal aborts. Listeners added after that never run, as in JS.
    pub fn add_abort_listener(&self, listener: impl FnOnce() + 'static) {
        if !self.aborted() {
            self.0.listeners.borrow_mut().push(Box::new(listener));
        }
    }
}

/// `AbortController`: what aborts an [`AbortSignal`].
#[derive(Default)]
pub struct AbortController {
    signal: AbortSignal,
}

impl AbortController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn signal(&self) -> AbortSignal {
        self.signal.clone()
    }

    /// `controller.abort()`: aborts the signal and calls its listeners, in
    /// the order they were added, once.
    pub fn abort(&self) {
        let inner = &self.signal.0;
        if inner.aborted.replace(true) {
            return;
        }
        let listeners = std::mem::take(&mut *inner.listeners.borrow_mut());
        for listener in listeners {
            listener();
        }
    }
}
