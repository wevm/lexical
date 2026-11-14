//! `internal/ExtensionRep.ts`: the builder's record of one extension, its
//! state through the phases (configured, initialized, built, registered),
//! and what it answers the phases about other extensions.

use super::lexical_builder::BuilderInner;
use crate::error::invariant;
use crate::extension_core::{
    AbortSignal, AnyExtensionDependency, AnyLexicalExtension, Erased, ExtensionBuildState,
    ExtensionInitState, ExtensionRegisterState, ExtensionRepAccess, InitialEditorConfig,
};
use crate::{Editor, LexicalError, Unregister};
use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

/// `ExtensionRepStateIds`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum StateId {
    Unmarked = 0,
    Temporary = 1,
    Permanent = 2,
    Configured = 3,
    Initialized = 4,
    Built = 5,
    Registered = 6,
    AfterRegistration = 7,
}

impl StateId {
    fn n(self) -> u8 {
        self as u8
    }
}

/// `ExtensionRepState`: the state id and what the states from `configured`
/// on add.
pub(crate) struct RepState {
    pub(crate) id: StateId,
    config: Option<Erased>,
    init_result: Option<Erased>,
    output: Option<Erased>,
}

/// `ExtensionRep`.
pub(crate) struct ExtensionRep {
    builder: Weak<BuilderInner>,
    this: Weak<ExtensionRep>,
    /// `configs`: a `Set` of `Partial<Config>`, so the same overrides
    /// object counts once.
    pub(crate) configs: RefCell<Vec<Erased>>,
    dependency: RefCell<Option<AnyExtensionDependency>>,
    peer_name_set: RefCell<Option<Vec<String>>>,
    pub(crate) extension: Rc<dyn AnyLexicalExtension>,
    pub(crate) state: RefCell<RepState>,
    signal: RefCell<Option<AbortSignal>>,
}

impl ExtensionRep {
    pub(crate) fn new(
        builder: Weak<BuilderInner>,
        extension: Rc<dyn AnyLexicalExtension>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|this| ExtensionRep {
            builder,
            this: this.clone(),
            configs: RefCell::new(Vec::new()),
            dependency: RefCell::new(None),
            peer_name_set: RefCell::new(None),
            extension,
            state: RefCell::new(RepState {
                id: StateId::Unmarked,
                config: None,
                init_result: None,
                output: None,
            }),
            signal: RefCell::new(None),
        })
    }

    fn builder(&self) -> Rc<BuilderInner> {
        self.builder
            .upgrade()
            .expect("an extension's builder outlives it")
    }

    fn access(&self) -> Rc<dyn ExtensionRepAccess> {
        self.this.upgrade().expect("an extension rep is alive")
    }

    fn name(&self) -> &str {
        self.extension.name()
    }

    pub(crate) fn id(&self) -> StateId {
        self.state.borrow().id
    }

    pub(crate) fn set_id(&self, id: StateId) {
        self.state.borrow_mut().id = id;
    }

    /// `configs.add(config)`.
    pub(crate) fn add_config(&self, config: &Erased) {
        let mut configs = self.configs.borrow_mut();
        if !configs.iter().any(|c| same_object(c, config)) {
            configs.push(config.clone());
        }
    }

    /// `mergeConfigs()`: the default config with each config of `configs`
    /// merged in, with the extension's `mergeConfig` or `shallowMergeConfig`.
    fn merge_configs(&self) -> Result<Erased, LexicalError> {
        let configs = self.configs.borrow().clone();
        self.extension.merge_configs_erased(&configs)
    }

    fn config(&self) -> Erased {
        self.state
            .borrow()
            .config
            .clone()
            .expect("a configured extension has a config")
    }

    /// `init(editorConfig)`.
    pub(crate) fn init(&self, editor_config: &mut InitialEditorConfig) -> Result<(), LexicalError> {
        let id = self.id();
        invariant!(
            id == StateId::Permanent,
            "ExtensionRep: Can not configure from state id {}",
            id.n()
        );
        let config = self.merge_configs()?;
        {
            let mut state = self.state.borrow_mut();
            state.config = Some(config.clone());
            state.id = StateId::Configured;
        }
        let init_state = ExtensionInitState { rep: self.access() };
        let init_result = self
            .extension
            .init_erased(editor_config, &config, &init_state)?;
        let mut state = self.state.borrow_mut();
        state.init_result = init_result;
        state.id = StateId::Initialized;
        Ok(())
    }

    /// `build(editor)`.
    pub(crate) fn build(&self, editor: &Editor) -> Result<(), LexicalError> {
        let id = self.id();
        invariant!(
            id == StateId::Initialized,
            "ExtensionRep: register called in state id {} (expected {} initialized)",
            id.n(),
            StateId::Built.n()
        );
        let build_state = ExtensionBuildState {
            rep: self.access(),
            _init: PhantomData,
        };
        let output = self
            .extension
            .build_erased(editor, &self.config(), &build_state)?;
        let mut state = self.state.borrow_mut();
        state.output = output;
        state.id = StateId::Built;
        Ok(())
    }

    fn register_state(&self) -> ExtensionRegisterState<(), ()> {
        ExtensionRegisterState {
            build: ExtensionBuildState {
                rep: self.access(),
                _init: PhantomData,
            },
            _output: PhantomData,
        }
    }

    /// `register(editor, signal)`: registers the extension and returns the
    /// clean-up, which returns it to the built state and runs the clean-up
    /// its `register` returned.
    pub(crate) fn register(
        &self,
        editor: &Editor,
        signal: AbortSignal,
    ) -> Result<Unregister, LexicalError> {
        *self.signal.borrow_mut() = Some(signal);
        let id = self.id();
        invariant!(
            id == StateId::Built,
            "ExtensionRep: register called in state id {} (expected {} built)",
            id.n(),
            StateId::Built.n()
        );
        let cleanup = self.extension.register_erased(
            editor,
            &self.config(),
            &self.register_state(),
            false,
        )?;
        self.set_id(StateId::Registered);
        let this = self.access_rep();
        Ok(Box::new(move || {
            let after = this.id();
            // An invariant in JS, which only a dispose before registration
            // finished could break.
            assert!(
                after == StateId::AfterRegistration,
                "ExtensionRep: rollbackToBuiltState called in state id {} (expected {} afterRegistration)",
                id.n(),
                StateId::AfterRegistration.n()
            );
            this.set_id(StateId::Built);
            if let Some(cleanup) = cleanup {
                cleanup();
            }
        }))
    }

    fn access_rep(&self) -> Rc<ExtensionRep> {
        self.this.upgrade().expect("an extension rep is alive")
    }

    /// `afterRegistration(editor)`.
    pub(crate) fn after_registration(
        &self,
        editor: &Editor,
    ) -> Result<Option<Unregister>, LexicalError> {
        let id = self.id();
        invariant!(
            id == StateId::Registered,
            "ExtensionRep: afterRegistration called in state id {} (expected {} registered)",
            id.n(),
            StateId::Registered.n()
        );
        let result =
            self.extension
                .register_erased(editor, &self.config(), &self.register_state(), true)?;
        self.set_id(StateId::AfterRegistration);
        Ok(result)
    }

    /// `getExtensionInitDependency()`: the config, once configured.
    fn get_extension_init_dependency(&self) -> Result<Erased, LexicalError> {
        let id = self.id();
        invariant!(
            id >= StateId::Configured,
            "ExtensionRep: getExtensionInitDependency called in state id {} (expected >= {} configured)",
            id.n(),
            StateId::Configured.n()
        );
        Ok(self.config())
    }

    /// `getExtensionDependency()`: the config, init result and output, once
    /// built.
    pub(crate) fn get_extension_dependency(&self) -> Result<AnyExtensionDependency, LexicalError> {
        if let Some(dependency) = self.dependency.borrow().clone() {
            return Ok(dependency);
        }
        let state = self.state.borrow();
        invariant!(
            state.id >= StateId::Built,
            "Extension {} used as a dependency before build",
            self.name()
        );
        let dependency = AnyExtensionDependency {
            config: state
                .config
                .clone()
                .expect("a built extension has a config"),
            init: state.init_result.clone(),
            output: state.output.clone(),
        };
        drop(state);
        *self.dependency.borrow_mut() = Some(dependency.clone());
        Ok(dependency)
    }

    fn missing_dependency(&self, dep: &dyn AnyLexicalExtension) -> LexicalError {
        LexicalError::new(format!(
            "LexicalExtensionBuilder: Extension {} missing dependency extension {} to be in registry",
            self.name(),
            dep.name()
        ))
    }
}

/// JS's `===` on two objects held as `Rc<dyn Any>`.
fn same_object(a: &Erased, b: &Erased) -> bool {
    std::ptr::addr_eq(Rc::as_ptr(a), Rc::as_ptr(b))
}

impl ExtensionRepAccess for ExtensionRep {
    fn extension_name(&self) -> String {
        self.name().to_owned()
    }

    /// `getInitDependency(dep).config`.
    fn get_init_dependency(&self, dep: &dyn AnyLexicalExtension) -> Result<Erased, LexicalError> {
        let rep = self.builder().get_extension_rep(dep)?;
        let Some(rep) = rep else {
            return Err(self.missing_dependency(dep));
        };
        rep.get_extension_init_dependency()
    }

    /// `getInitPeer(name)?.config`.
    fn get_init_peer(&self, name: &str) -> Result<Option<Erased>, LexicalError> {
        let rep = self.builder().extension_rep_by_name(name);
        rep.map(|rep| rep.get_extension_init_dependency())
            .transpose()
    }

    /// `getDependency(dep)`.
    fn get_dependency(
        &self,
        dep: &dyn AnyLexicalExtension,
    ) -> Result<AnyExtensionDependency, LexicalError> {
        let rep = self.builder().get_extension_rep(dep)?;
        let Some(rep) = rep else {
            return Err(self.missing_dependency(dep));
        };
        rep.get_extension_dependency()
    }

    /// `getPeer(name)`.
    fn get_peer(&self, name: &str) -> Result<Option<AnyExtensionDependency>, LexicalError> {
        let rep = self.builder().extension_rep_by_name(name);
        rep.map(|rep| rep.get_extension_dependency()).transpose()
    }

    /// `getInitResult()`.
    fn get_init_result(&self) -> Result<Option<Erased>, LexicalError> {
        invariant!(
            self.extension.has_init(),
            "ExtensionRep: getInitResult() called for Extension {} that does not define init",
            self.name()
        );
        let state = self.state.borrow();
        invariant!(
            state.id >= StateId::Initialized,
            "ExtensionRep: getInitResult() called for ExtensionRep in state id {} < {} (initialized)",
            state.id.n(),
            StateId::Initialized.n()
        );
        Ok(state.init_result.clone())
    }

    /// `getDirectDependentNames()`.
    fn get_direct_dependent_names(&self) -> Vec<String> {
        self.builder().incoming_edge_names(self.name())
    }

    /// `getPeerNameSet()`.
    fn get_peer_name_set(&self) -> Vec<String> {
        if let Some(names) = self.peer_name_set.borrow().clone() {
            return names;
        }
        let mut names: Vec<String> = Vec::new();
        for peer in self.extension.peer_dependencies() {
            if !names.iter().any(|n| n == peer.name()) {
                names.push(peer.name().to_owned());
            }
        }
        *self.peer_name_set.borrow_mut() = Some(names.clone());
        names
    }

    /// `getSignal()`.
    fn get_signal(&self) -> Result<AbortSignal, LexicalError> {
        self.signal
            .borrow()
            .clone()
            .ok_or_else(|| LexicalError::new("ExtensionRep.getSignal() called before register"))
    }

    fn get_output(&self) -> Option<Erased> {
        self.state.borrow().output.clone()
    }
}
