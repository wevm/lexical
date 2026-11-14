//! `LexicalBuilder.ts`: `buildEditorFromExtensions` and `LexicalBuilder`,
//! which resolves the extensions' dependency graph, merges their configs
//! in dependency order, and runs their phases: `init` (before the editor
//! exists), `build`, `register` and `afterRegistration`.

use super::extension_rep::{ExtensionRep, StateId};
use super::initial_state_extension::initial_state_extension;
use crate::error::invariant;
use crate::extension_core::{
    AbortController, AnyLexicalExtension, AnyLexicalExtensionArgument, Erased, InitialEditorConfig,
};
use crate::merge_register::merge_register;
use crate::ordered::{OrderedMap, OrderedSet};
use crate::{CreateEditorArgs, Editor, LexicalError, NodeClass, Unregister, create_editor};
use std::cell::RefCell;
use std::rc::Rc;

/// `buildEditorFromExtensions(...extensions)`: an editor built from the
/// extensions (with `InitialStateExtension` first) and their dependencies,
/// each configured, built and registered, with its initial state set.
pub fn build_editor_from_extensions(
    extensions: impl IntoIterator<Item = AnyLexicalExtensionArgument>,
) -> Result<LexicalEditorWithDispose, LexicalError> {
    LexicalBuilder::from_extensions(extensions)?.build_editor()
}

/// `LexicalEditorWithDispose`: an editor built from extensions, with the
/// `dispose` that undoes their registration. It dereferences to its
/// [`Editor`]. Dropping it doesn't dispose it (JS's `using` does; call
/// [`dispose`](Self::dispose)).
#[derive(Clone)]
pub struct LexicalEditorWithDispose {
    editor: Editor,
    dispose: Rc<RefCell<Option<Unregister>>>,
}

impl LexicalEditorWithDispose {
    /// `dispose()`: runs every clean-up the extensions' `register` and
    /// `afterRegistration` returned, in the reverse of the order they
    /// registered, then aborts their signal. Only the first call does
    /// anything.
    pub fn dispose(&self) {
        let dispose = self.dispose.borrow_mut().take();
        if let Some(dispose) = dispose {
            dispose();
        }
    }

    /// The editor.
    pub fn editor(&self) -> &Editor {
        &self.editor
    }
}

impl std::ops::Deref for LexicalEditorWithDispose {
    type Target = Editor;

    fn deref(&self) -> &Editor {
        &self.editor
    }
}

/// `LexicalBuilder`: the extensions an editor is built from, by name, with
/// the edges between them. Cloning it shares it.
#[derive(Clone)]
pub struct LexicalBuilder(pub(crate) Rc<BuilderInner>);

pub(crate) struct BuilderInner {
    roots: Vec<AnyLexicalExtensionArgument>,
    extension_name_map: RefCell<OrderedMap<Rc<ExtensionRep>>>,
    /// Per extension, the configs it gives each dependency and peer.
    outgoing_config_edges: RefCell<OrderedMap<OrderedMap<Vec<Erased>>>>,
    incoming_edges: RefCell<OrderedMap<OrderedSet>>,
    conflicts: RefCell<OrderedMap<String>>,
    sorted_extension_reps: RefCell<Option<Vec<Rc<ExtensionRep>>>>,
}

impl BuilderInner {
    /// `extensionNameMap.get(name)`.
    pub(crate) fn extension_rep_by_name(&self, name: &str) -> Option<Rc<ExtensionRep>> {
        self.extension_name_map.borrow().get(name).cloned()
    }

    /// `getExtensionRep(extension)`: the extension's record, an error when
    /// another extension has its name.
    pub(crate) fn get_extension_rep(
        &self,
        extension: &dyn AnyLexicalExtension,
    ) -> Result<Option<Rc<ExtensionRep>>, LexicalError> {
        let Some(rep) = self.extension_rep_by_name(extension.name()) else {
            return Ok(None);
        };
        invariant!(
            same_extension(rep.extension.as_ref(), extension),
            "LexicalBuilder: A registered extension with name {} exists but does not match the given extension",
            extension.name()
        );
        Ok(Some(rep))
    }

    /// `incomingEdges.get(name)`, in the order the edges were added.
    pub(crate) fn incoming_edge_names(&self, name: &str) -> Vec<String> {
        self.incoming_edges
            .borrow()
            .get(name)
            .map(|set| set.iter().map(str::to_owned).collect())
            .unwrap_or_default()
    }
}

/// JS's `===` on two extensions.
fn same_extension(a: &dyn AnyLexicalExtension, b: &dyn AnyLexicalExtension) -> bool {
    std::ptr::addr_eq(
        a as *const dyn AnyLexicalExtension,
        b as *const dyn AnyLexicalExtension,
    )
}

impl LexicalBuilder {
    /// `new LexicalBuilder(roots)`.
    fn new(roots: Vec<AnyLexicalExtensionArgument>) -> Result<Self, LexicalError> {
        let builder = LexicalBuilder(Rc::new(BuilderInner {
            roots: roots.clone(),
            extension_name_map: RefCell::new(OrderedMap::new()),
            outgoing_config_edges: RefCell::new(OrderedMap::new()),
            incoming_edges: RefCell::new(OrderedMap::new()),
            conflicts: RefCell::new(OrderedMap::new()),
            sorted_extension_reps: RefCell::new(None),
        }));
        for extension in &roots {
            builder.add_extension(extension)?;
        }
        Ok(builder)
    }

    /// `LexicalBuilder.fromExtensions(extensions)`: a builder of
    /// `InitialStateExtension` and the extensions.
    pub fn from_extensions(
        extensions: impl IntoIterator<Item = AnyLexicalExtensionArgument>,
    ) -> Result<Self, LexicalError> {
        let mut roots: Vec<AnyLexicalExtensionArgument> = vec![initial_state_extension().into()];
        roots.extend(extensions);
        Self::new(roots)
    }

    /// `LexicalBuilder.maybeFromEditor(editor)`: the builder that built the
    /// editor, if one did.
    pub fn maybe_from_editor(editor: &Editor) -> Option<LexicalBuilder> {
        editor
            .0
            .builder
            .borrow()
            .as_ref()
            .and_then(|b| b.downcast_ref::<LexicalBuilder>().cloned())
    }

    /// `LexicalBuilder.fromEditor(editor)`.
    pub fn from_editor(editor: &Editor) -> Result<LexicalBuilder, LexicalError> {
        Self::maybe_from_editor(editor).ok_or_else(|| {
            LexicalError::new(
                "LexicalBuilder.fromEditor: The given editor was not created with LexicalBuilder",
            )
        })
    }

    /// `constructEditor()`: the editor of the merged configuration, with
    /// every extension built.
    pub fn construct_editor(&self) -> Result<Editor, LexicalError> {
        let config = self.build_create_editor_args()?;
        // `namespace` isn't in `InitialEditorConfig` yet, so the editor gets
        // `createEditor`'s random one.
        let editor = create_editor(CreateEditorArgs {
            nodes: config.nodes,
            ..Default::default()
        });
        *editor.0.builder.borrow_mut() = Some(Rc::new(self.clone()));
        for extension_rep in self.sorted_extension_reps()? {
            extension_rep.build(&editor)?;
        }
        Ok(editor)
    }

    /// `buildEditor()`.
    pub fn build_editor(&self) -> Result<LexicalEditorWithDispose, LexicalError> {
        let editor = self.construct_editor()?;
        // JS also detaches the root element (`setRootElement(null)`), which
        // a headless editor doesn't have.
        let dispose = self.register_editor(&editor)?;
        Ok(LexicalEditorWithDispose {
            editor,
            dispose: Rc::new(RefCell::new(Some(dispose))),
        })
    }

    /// `hasExtensionByName(name)`.
    pub fn has_extension_by_name(&self, name: &str) -> bool {
        self.0.extension_name_map.borrow().contains(name)
    }

    /// `addEdge(from, to, configs)`: an extension's configs for another
    /// are kept in the order they were seen, even when it reaches the
    /// other more than once.
    fn add_edge(&self, from_extension_name: &str, to_extension_name: &str, configs: Vec<Erased>) {
        {
            let mut outgoing_edges = self.0.outgoing_config_edges.borrow_mut();
            if let Some(outgoing) = outgoing_edges.get_mut(from_extension_name) {
                if let Some(existing) = outgoing.get_mut(to_extension_name) {
                    existing.extend(configs);
                } else {
                    outgoing.set(to_extension_name, configs);
                }
            } else {
                let mut outgoing = OrderedMap::new();
                outgoing.set(to_extension_name, configs);
                outgoing_edges.set(from_extension_name, outgoing);
            }
        }
        let mut incoming_edges = self.0.incoming_edges.borrow_mut();
        if let Some(incoming) = incoming_edges.get_mut(to_extension_name) {
            incoming.add(from_extension_name);
        } else {
            let mut incoming = OrderedSet::new();
            incoming.add(from_extension_name);
            incoming_edges.set(to_extension_name, incoming);
        }
    }

    /// `addExtension(arg)`: adds the extension, unless it is there, then
    /// its dependencies and the edges to them and to its peers.
    pub fn add_extension(&self, arg: &AnyLexicalExtensionArgument) -> Result<(), LexicalError> {
        invariant!(
            self.0.sorted_extension_reps.borrow().is_none(),
            "LexicalBuilder: addExtension called after finalization"
        );
        let extension = &arg.extension;
        let name = extension.name();
        let existing = self.0.extension_rep_by_name(name);
        invariant!(
            existing
                .as_ref()
                .is_none_or(|rep| same_extension(rep.extension.as_ref(), extension.as_ref())),
            "LexicalBuilder: Multiple extensions registered with name {}, names must be unique",
            name
        );
        if existing.is_some() {
            return Ok(());
        }
        let extension_rep = ExtensionRep::new(Rc::downgrade(&self.0), extension.clone());
        self.0
            .extension_name_map
            .borrow_mut()
            .set(name, extension_rep);
        let has_conflict = self.0.conflicts.borrow().get(name).cloned();
        if let Some(has_conflict) = has_conflict {
            return Err(LexicalError::new(format!(
                "LexicalBuilder: extension {name} conflicts with {has_conflict}"
            )));
        }
        for conflict in extension.conflicts_with() {
            invariant!(
                !self.0.extension_name_map.borrow().contains(conflict),
                "LexicalBuilder: extension {} conflicts with {}",
                name,
                conflict
            );
            self.0.conflicts.borrow_mut().set(conflict, name.to_owned());
        }
        for dep in extension.dependencies() {
            self.add_edge(name, dep.extension.name(), dep.configs.clone());
            self.add_extension(dep)?;
        }
        for peer in extension.peer_dependencies() {
            self.add_edge(name, peer.name(), peer.config.iter().cloned().collect());
        }
        Ok(())
    }

    fn visit(
        &self,
        rep: &Rc<ExtensionRep>,
        from_extension_name: Option<&str>,
        sorted: &mut Vec<Rc<ExtensionRep>>,
    ) -> Result<(), LexicalError> {
        let mark = rep.id();
        if mark == StateId::Permanent {
            return Ok(());
        }
        let extension_name = rep.extension.name().to_owned();
        invariant!(
            mark == StateId::Unmarked,
            "LexicalBuilder: Circular dependency detected for Extension {} from {}",
            extension_name,
            from_extension_name.unwrap_or("[unknown]")
        );
        rep.set_id(StateId::Temporary);
        let to_names: Vec<String> = self
            .0
            .outgoing_config_edges
            .borrow()
            .get(&extension_name)
            .map(|edges| edges.keys().map(str::to_owned).collect())
            .unwrap_or_default();
        for to_extension_name in to_names {
            // None for an optional peer dependency.
            if let Some(to_rep) = self.0.extension_rep_by_name(&to_extension_name) {
                self.visit(&to_rep, Some(&extension_name), sorted)?;
            }
        }
        rep.set_id(StateId::Permanent);
        sorted.push(rep.clone());
        Ok(())
    }

    /// `sortedExtensionReps()`: the extensions in dependency order (a
    /// depth-first topological sort, dependencies and peers first), with
    /// each one's configs from its dependents in that order, then from the
    /// roots.
    pub(crate) fn sorted_extension_reps(&self) -> Result<Vec<Rc<ExtensionRep>>, LexicalError> {
        if let Some(sorted) = self.0.sorted_extension_reps.borrow().clone() {
            return Ok(sorted);
        }
        let mut sorted_extension_reps = Vec::new();
        let reps: Vec<Rc<ExtensionRep>> = self
            .0
            .extension_name_map
            .borrow()
            .values()
            .cloned()
            .collect();
        for rep in &reps {
            if rep.id() == StateId::Unmarked {
                self.visit(rep, None, &mut sorted_extension_reps)?;
            }
        }
        for rep in &sorted_extension_reps {
            let edges: Vec<(String, Vec<Erased>)> = self
                .0
                .outgoing_config_edges
                .borrow()
                .get(rep.extension.name())
                .map(|edges| {
                    edges
                        .iter()
                        .map(|(k, v)| (k.to_owned(), v.clone()))
                        .collect()
                })
                .unwrap_or_default();
            for (to_extension_name, configs) in edges {
                if let Some(to_rep) = self.0.extension_rep_by_name(&to_extension_name) {
                    for config in &configs {
                        to_rep.add_config(config);
                    }
                }
            }
        }
        for root in &self.0.roots {
            if !root.configs.is_empty() {
                let to_rep = self.0.extension_rep_by_name(root.extension.name());
                let Some(to_rep) = to_rep else {
                    return Err(LexicalError::new(format!(
                        "LexicalBuilder: Expecting existing ExtensionRep for {}",
                        root.extension.name()
                    )));
                };
                for config in &root.configs {
                    to_rep.add_config(config);
                }
            }
        }
        *self.0.sorted_extension_reps.borrow_mut() = Some(sorted_extension_reps.clone());
        Ok(sorted_extension_reps)
    }

    /// `registerEditor(editor)`: registers every extension, then runs every
    /// `afterRegistration`, and returns the clean-up of all of it.
    pub fn register_editor(&self, editor: &Editor) -> Result<Unregister, LexicalError> {
        let extension_reps = self.sorted_extension_reps()?;
        let controller = AbortController::new();
        let signal = controller.signal();
        let mut cleanups: Vec<Unregister> = vec![Box::new(move || controller.abort())];
        for extension_rep in &extension_reps {
            cleanups.push(extension_rep.register(editor, signal.clone())?);
        }
        for extension_rep in &extension_reps {
            if let Some(cleanup) = extension_rep.after_registration(editor)? {
                cleanups.push(cleanup);
            }
        }
        Ok(merge_register(cleanups))
    }

    /// `buildCreateEditorArgs()`: the editor configuration the extensions
    /// contribute, merged in dependency order (the last
    /// `$initialEditorState` wins, nodes are added once), after every
    /// extension's `init` has seen it.
    pub fn build_create_editor_args(&self) -> Result<InitialEditorConfig, LexicalError> {
        let mut config = InitialEditorConfig::default();
        let mut nodes: Vec<NodeClass> = Vec::new();
        let extension_reps = self.sorted_extension_reps()?;
        for extension_rep in &extension_reps {
            let extension = &extension_rep.extension;
            if let Some(initial_editor_state) = extension.initial_editor_state() {
                config.initial_editor_state = Some(initial_editor_state.clone());
            }
            for node in extension.nodes() {
                if !nodes.iter().any(|n| n.node_type() == node.node_type()) {
                    nodes.push(*node);
                }
            }
        }
        config.nodes = nodes;
        for extension_rep in &extension_reps {
            extension_rep.init(&mut config)?;
        }
        Ok(config)
    }
}
