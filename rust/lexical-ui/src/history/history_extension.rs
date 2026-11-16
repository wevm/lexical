//! `HistoryExtension` and `SharedHistoryExtension`
//! (`packages/lexical-history/src/index.ts`, the `history` and `extension`
//! features).

use super::{
    DateNow, HistoryState, HistoryStateChange, create_empty_history_state, register_history_with,
    system_date_now,
};
use crate::extension::get_peer_dependency_from_editor;
use crate::extension::signals::{ReadonlySignal, Signal, batch, effect, signal};
use crate::extension_core::{
    ExtensionBuildState, ExtensionConfigBase, ExtensionInitState, ExtensionRegisterState,
    InitialEditorConfig, LexicalExtension, config_extension, define_extension,
};
use crate::{Editor, LexicalError};
use std::cell::RefCell;
use std::rc::Rc;

/// A [`HistoryState`] as a signal holds it: compared by identity, as JS's
/// `!==` compares the object.
#[derive(Clone)]
pub struct HistoryStateRef(pub SharedHistoryState);

impl PartialEq for HistoryStateRef {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// A history state as [`register_history`](super::register_history) shares
/// it.
pub type SharedHistoryState = Rc<RefCell<HistoryState>>;

/// `HistoryConfig.createInitialHistoryState`, compared by identity as JS
/// compares functions.
#[derive(Clone)]
pub struct CreateInitialHistoryState(pub Rc<dyn Fn(&Editor) -> SharedHistoryState>);

impl PartialEq for CreateInitialHistoryState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// `HistoryConfig.now`, the clock, compared by identity as JS compares
/// functions.
#[derive(Clone)]
pub struct HistoryNow(pub DateNow);

impl PartialEq for HistoryNow {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// `HistoryConfig`.
#[derive(Clone)]
pub struct HistoryConfig {
    /// `delay`: how long, in milliseconds, an update of the same kind as
    /// the one before it still merges into its entry. 300 by default.
    pub delay: f64,
    /// `createInitialHistoryState`: the history state the extension starts
    /// with, [`create_empty_history_state`] by default.
    pub create_initial_history_state: CreateInitialHistoryState,
    /// `disabled`: whether nothing is recorded. JS's default is
    /// `typeof window === 'undefined'`, true in a headless JS editor; the
    /// port's is `false`, as Swift's, since its editors are made for apps
    /// that edit (rust/PARITY.md).
    pub disabled: bool,
    /// `maxDepth`: the most entries the undo stack keeps; `None` (the
    /// default) keeps them all.
    pub max_depth: Option<usize>,
    /// `now`: the clock, `Date.now()` by default.
    pub now: HistoryNow,
}

/// `Partial<HistoryConfig>`.
#[derive(Clone, Default)]
pub struct HistoryConfigOverrides {
    pub delay: Option<f64>,
    pub create_initial_history_state: Option<CreateInitialHistoryState>,
    pub disabled: Option<bool>,
    pub max_depth: Option<Option<usize>>,
    pub now: Option<HistoryNow>,
}

impl ExtensionConfigBase for HistoryConfig {
    type Partial = HistoryConfigOverrides;

    fn shallow_merge(&self, o: &HistoryConfigOverrides) -> Self {
        HistoryConfig {
            delay: o.delay.unwrap_or(self.delay),
            create_initial_history_state: o
                .create_initial_history_state
                .clone()
                .unwrap_or_else(|| self.create_initial_history_state.clone()),
            disabled: o.disabled.unwrap_or(self.disabled),
            max_depth: o.max_depth.unwrap_or(self.max_depth),
            now: o.now.clone().unwrap_or_else(|| self.now.clone()),
        }
    }
}

/// `HistoryExtensionInit`: the writable signals the init phase makes.
#[derive(Clone)]
pub struct HistoryExtensionInit {
    pub can_redo: Signal<bool>,
    pub can_undo: Signal<bool>,
}

/// `HistoryExtensionOutput`. The config's signals are writable, so that
/// `SharedHistoryExtension` can redirect them; `can_undo` and `can_redo`
/// follow the history state's stacks.
#[derive(Clone)]
pub struct HistoryExtensionOutput {
    /// Whether the redo stack has an entry.
    pub can_redo: ReadonlySignal<bool>,
    /// Whether the undo stack has an entry.
    pub can_undo: ReadonlySignal<bool>,
    /// The merge delay, in milliseconds, read when an update may merge.
    pub delay: Signal<f64>,
    /// When `true` the history isn't registered.
    pub disabled: Signal<bool>,
    /// The history state.
    pub history_state: Signal<HistoryStateRef>,
    /// The cap of the undo stack, read when an entry is pushed.
    pub max_depth: Signal<Option<usize>>,
    /// The clock.
    pub now: Signal<HistoryNow>,
}

/// `HistoryExtension`.
pub type HistoryExtensionType =
    LexicalExtension<HistoryConfig, HistoryExtensionOutput, HistoryExtensionInit>;

fn make_history_extension() -> Rc<HistoryExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |editor: &Editor,
             config: &HistoryConfig,
             state: &ExtensionBuildState<HistoryExtensionInit>| {
                // `namedSignals({delay, disabled, historyState, maxDepth,
                // now})`, then the init result's signals.
                let history_state = (config.create_initial_history_state.0)(editor);
                let init = state.get_init_result()?;
                Ok(HistoryExtensionOutput {
                    delay: signal(config.delay),
                    disabled: signal(config.disabled),
                    history_state: signal(HistoryStateRef(history_state)),
                    max_depth: signal(config.max_depth),
                    now: signal(config.now.clone()),
                    can_redo: init.can_redo.as_readonly(),
                    can_undo: init.can_undo.as_readonly(),
                })
            },
        )),
        init: Some(Rc::new(
            |_: &mut InitialEditorConfig, _: &HistoryConfig, _: &ExtensionInitState| {
                Ok(HistoryExtensionInit {
                    can_redo: signal(false),
                    can_undo: signal(false),
                })
            },
        )),
        register:
            Some(
                Rc::new(
                    |editor: &Editor,
                     _config: &HistoryConfig,
                     state: &ExtensionRegisterState<
                        HistoryExtensionInit,
                        HistoryExtensionOutput,
                    >| {
                        let init = state.get_init_result()?;
                        let stores = state.get_output().expect("HistoryExtension has a build");
                        // One update path: `None` resets both signals (when
                        // disabled); a history state sets them from its stacks
                        // (`registerHistory` calls it on registration and after
                        // every change). The batch notifies their subscribers once.
                        let sync_from_history_state = {
                            let (can_undo, can_redo) =
                                (init.can_undo.clone(), init.can_redo.clone());
                            move |history_state: Option<&HistoryState>| {
                                batch(|| {
                                    can_undo.set_value(
                                        history_state.is_some_and(|h| !h.undo_stack.is_empty()),
                                    );
                                    can_redo.set_value(
                                        history_state.is_some_and(|h| !h.redo_stack.is_empty()),
                                    );
                                });
                            }
                        };
                        let editor = editor.weak();
                        Ok(effect(move || {
                            if stores.disabled.value() {
                                sync_from_history_state(None);
                                return None;
                            }
                            let history_state = stores.history_state.value();
                            let editor = editor.upgrade().map(Editor)?;
                            let delay = stores.delay.clone();
                            let now = stores.now.clone();
                            let max_depth = stores.max_depth.clone();
                            let sync = sync_from_history_state.clone();
                            Some(register_history_with(
                                &editor,
                                &history_state.0,
                                Rc::new(move || delay.peek()),
                                Some(Rc::new(move || (now.peek().0)())),
                                Some(Rc::new(move |h: &HistoryState| sync(Some(h)))
                                    as HistoryStateChange),
                                Rc::new(move || max_depth.peek()),
                            ))
                        }))
                    },
                ),
            ),
        ..LexicalExtension::new(
            "@lexical/history/History",
            HistoryConfig {
                create_initial_history_state: CreateInitialHistoryState(Rc::new(|_| {
                    create_empty_history_state()
                })),
                delay: 300.0,
                disabled: false,
                max_depth: None,
                now: HistoryNow(Rc::new(system_date_now)),
            },
        )
    })
}

/// `getHistoryPeer(editor)`: the `HistoryExtension` of `editor`, if it was
/// built with one.
fn get_history_peer(
    editor: Option<&Editor>,
) -> Result<
    Option<
        crate::LexicalExtensionDependency<
            HistoryConfig,
            HistoryExtensionOutput,
            HistoryExtensionInit,
        >,
    >,
    LexicalError,
> {
    match editor {
        None => Ok(None),
        Some(editor) => get_peer_dependency_from_editor(editor, "@lexical/history/History"),
    }
}

/// `SharedHistoryConfig`.
#[derive(Clone, Default)]
pub struct SharedHistoryConfig {
    /// `disabled`.
    pub disabled: bool,
    /// `parentEditor`: the editor whose history this one shares; without
    /// one, the editor's parent editor (which `NestedEditorExtension`
    /// sets).
    pub parent_editor: Option<Editor>,
}

/// `Partial<SharedHistoryConfig>`.
#[derive(Clone, Default)]
pub struct SharedHistoryConfigOverrides {
    pub disabled: Option<bool>,
    pub parent_editor: Option<Option<Editor>>,
}

impl ExtensionConfigBase for SharedHistoryConfig {
    type Partial = SharedHistoryConfigOverrides;

    fn shallow_merge(&self, o: &SharedHistoryConfigOverrides) -> Self {
        SharedHistoryConfig {
            disabled: o.disabled.unwrap_or(self.disabled),
            parent_editor: o
                .parent_editor
                .clone()
                .unwrap_or_else(|| self.parent_editor.clone()),
        }
    }
}

/// `namedSignals(SharedHistoryConfig)`: `SharedHistoryExtension`'s output.
#[derive(Clone)]
pub struct SharedHistoryOutput {
    pub disabled: Signal<bool>,
    pub parent_editor: Signal<Option<Editor>>,
}

/// `SharedHistoryExtension`.
pub type SharedHistoryExtensionType =
    LexicalExtension<SharedHistoryConfig, SharedHistoryOutput, ()>;

fn make_shared_history_extension() -> Rc<SharedHistoryExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |editor: &Editor, config: &SharedHistoryConfig, _: &ExtensionBuildState<()>| {
                Ok(SharedHistoryOutput {
                    disabled: signal(config.disabled),
                    // `parentEditor || editor._parentEditor`.
                    parent_editor: signal(
                        config
                            .parent_editor
                            .clone()
                            .or_else(|| editor.parent_editor().cloned()),
                    ),
                })
            },
        )),
        dependencies: vec![config_extension(
            &history_extension(),
            [HistoryConfigOverrides {
                disabled: Some(true),
                ..HistoryConfigOverrides::default()
            }],
        )],
        register: Some(Rc::new(
            |editor: &Editor,
             _config: &SharedHistoryConfig,
             state: &ExtensionRegisterState<(), SharedHistoryOutput>| {
                let stores = state
                    .get_output()
                    .expect("SharedHistoryExtension has a build");
                let output = state
                    .get_dependency(&history_extension())?
                    .output
                    .expect("HistoryExtension has a build");
                // JS also reads both editors' HMR restore counters, so that
                // a rebuilt history re-links; `HMRExtension` isn't ported.
                let _ = editor;
                Ok(effect(move || {
                    if stores.disabled.value() {
                        return None;
                    }
                    let parent_editor = stores.parent_editor.value();
                    // A failed lookup is an editor of another build, which
                    // JS's `getPeerDependencyFromEditor` throws for; an
                    // effect can't, so there is no parent history then.
                    let Ok(Some(parent_peer)) = get_history_peer(parent_editor.as_ref()) else {
                        return None;
                    };
                    let parent_output = parent_peer.output?;
                    batch(|| {
                        output.delay.set_value(parent_output.delay.value());
                        output
                            .history_state
                            .set_value(parent_output.history_state.value());
                        output.now.set_value(parent_output.now.value());
                        // The cap must come from the parent too: the child
                        // pushes onto the parent's shared undo stack.
                        output.max_depth.set_value(parent_output.max_depth.value());
                        // Toggling the parent's history forces this one's.
                        output.disabled.set_value(parent_output.disabled.value());
                    });
                    None
                }))
            },
        )),
        ..LexicalExtension::new(
            "@lexical/history/SharedHistory",
            SharedHistoryConfig::default(),
        )
    })
}

thread_local! {
    static HISTORY_EXTENSION: Rc<HistoryExtensionType> = make_history_extension();
    static SHARED_HISTORY_EXTENSION: Rc<SharedHistoryExtensionType> =
        make_shared_history_extension();
}

/// `HistoryExtension`: registers the undo and redo history
/// ([`register_history`](super::register_history)) with its config as
/// signals of its output, unless `disabled`.
pub fn history_extension() -> Rc<HistoryExtensionType> {
    HISTORY_EXTENSION.with(Rc::clone)
}

/// `SharedHistoryExtension`: makes the editor's `HistoryExtension` (which
/// it adds, disabled) use the history of its `parent_editor` (by default
/// its parent editor), when that editor was built with `HistoryExtension`.
pub fn shared_history_extension() -> Rc<SharedHistoryExtensionType> {
    SHARED_HISTORY_EXTENSION.with(Rc::clone)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extension::{build_editor_from_extensions, get_extension_dependency_from_editor};
    use crate::{UNDO_COMMAND, UpdateOptions, create_paragraph_node, create_text_node, get_root};

    fn type_text(editor: &Editor, text: &'static str) {
        editor
            .update(
                move || {
                    let root = get_root();
                    root.clear()?;
                    let paragraph = create_paragraph_node();
                    paragraph.append(&[create_text_node(text)])?;
                    root.append(&[paragraph])
                },
                UpdateOptions {
                    discrete: true,
                    ..UpdateOptions::default()
                },
            )
            .unwrap();
    }

    /// `SharedHistoryExtension` links a child editor to its parent's
    /// history: an edit in the child goes on the parent's undo stack, and
    /// `can_undo` follows in both.
    #[test]
    fn shared_history_uses_the_parent_history() {
        let parent = build_editor_from_extensions(vec![config_extension(
            &history_extension(),
            [HistoryConfigOverrides {
                delay: Some(0.0),
                ..HistoryConfigOverrides::default()
            }],
        )])
        .unwrap();
        let child = build_editor_from_extensions(vec![config_extension(
            &shared_history_extension(),
            [SharedHistoryConfigOverrides {
                parent_editor: Some(Some(parent.editor().clone())),
                ..SharedHistoryConfigOverrides::default()
            }],
        )])
        .unwrap();
        let parent_output =
            get_extension_dependency_from_editor(parent.editor(), &history_extension())
                .unwrap()
                .output
                .unwrap();
        let child_output =
            get_extension_dependency_from_editor(child.editor(), &history_extension())
                .unwrap()
                .output
                .unwrap();
        assert!(child_output.history_state.peek() == parent_output.history_state.peek());
        assert!(!child_output.disabled.peek());
        // Commit the initial states, which the builder's update queued.
        parent.editor().read(|| ()).unwrap();
        child.editor().read(|| ()).unwrap();
        type_text(child.editor(), "child");
        assert_eq!(
            parent_output
                .history_state
                .peek()
                .0
                .borrow()
                .undo_stack
                .len(),
            1
        );
        assert!(child_output.can_undo.peek());
        assert!(child.editor().dispatch_command(&UNDO_COMMAND, ()).unwrap());
        let text = child
            .editor()
            .read(|| get_root().get_text_content())
            .unwrap();
        assert_eq!(text, "");
        // Disabling the parent's history disables the child's.
        parent_output.disabled.set_value(true);
        assert!(child_output.disabled.peek());
        child.dispose();
        parent.dispose();
    }
}
