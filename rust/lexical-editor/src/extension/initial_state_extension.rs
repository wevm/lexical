//! `InitialStateExtension.ts`: the extension every editor built from
//! extensions starts with, which sets the initial state once everything is
//! registered.

use crate::extension_core::{
    ExtensionConfigBase, ExtensionInitState, ExtensionRegisterState, InitialEditorConfig,
    InitialEditorStateType, LexicalExtension, define_extension,
};
use crate::nodes::{
    line_break::LineBreakNode, paragraph::ParagraphNode, root::RootNode, tab::TabNode,
    text::TextNode,
};
use crate::{
    Editor, HISTORY_MERGE_TAG, LexicalError, NodeClass, Unregister, UpdateOptions,
    create_paragraph_node, get_root,
};
use std::cell::Cell;
use std::rc::Rc;

/// `EditorSetOptions`: the options of `setEditorState`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EditorSetOptions {
    pub tag: Option<String>,
}

/// `InitialStateConfig`: the options of the update or the
/// `setEditorState` that sets the initial state.
#[derive(Clone, Debug)]
pub struct InitialStateConfig {
    pub update_options: UpdateOptions,
    pub set_options: EditorSetOptions,
}

/// `Partial<InitialStateConfig>`.
#[derive(Clone, Debug, Default)]
pub struct InitialStateConfigOverrides {
    pub update_options: Option<UpdateOptions>,
    pub set_options: Option<EditorSetOptions>,
}

impl ExtensionConfigBase for InitialStateConfig {
    type Partial = InitialStateConfigOverrides;

    fn shallow_merge(&self, overrides: &InitialStateConfigOverrides) -> Self {
        InitialStateConfig {
            update_options: overrides
                .update_options
                .clone()
                .unwrap_or_else(|| self.update_options.clone()),
            set_options: overrides
                .set_options
                .clone()
                .unwrap_or_else(|| self.set_options.clone()),
        }
    }
}

/// What `InitialStateExtension`'s `init` returns: the initial state, and
/// whether it was set.
pub struct InitialStateInit {
    pub initial_editor_state: InitialEditorStateType,
    pub initialized: Cell<bool>,
}

/// `InitialStateExtension`.
pub type InitialStateExtensionType = LexicalExtension<InitialStateConfig, (), InitialStateInit>;

/// `$defaultInitializer`: an empty root gets a paragraph.
fn default_initializer(_editor: &Editor) -> Result<(), LexicalError> {
    let root = get_root();
    if root.is_empty() {
        root.append(&[create_paragraph_node()])?;
    }
    Ok(())
}

fn init(
    editor_config: &mut InitialEditorConfig,
    _config: &InitialStateConfig,
    _state: &ExtensionInitState,
) -> Result<InitialStateInit, LexicalError> {
    let initial_editor_state = editor_config
        .initial_editor_state
        .clone()
        .unwrap_or_else(|| InitialEditorStateType::Update(Rc::new(default_initializer)));
    Ok(InitialStateInit {
        initial_editor_state,
        initialized: Cell::new(false),
    })
}

fn after_registration(
    editor: &Editor,
    config: &InitialStateConfig,
    state: &ExtensionRegisterState<InitialStateInit, ()>,
) -> Result<Unregister, LexicalError> {
    let init_result = state.get_init_result()?;
    if !init_result.initialized.replace(true) {
        let tag = config.set_options.tag.as_deref();
        match &init_result.initial_editor_state {
            InitialEditorStateType::EditorState(editor_state) => {
                editor.set_editor_state(editor_state, tag)?;
            }
            InitialEditorStateType::Update(update) => {
                let (update, target) = (update.clone(), editor.clone());
                editor.update(move || update(&target), config.update_options.clone())?;
            }
            // `''` is falsy in JS, so it sets nothing.
            InitialEditorStateType::Json(json) if !json.is_empty() => {
                let parsed_editor_state = editor.parse_editor_state(json)?;
                editor.set_editor_state(&parsed_editor_state, tag)?;
            }
            InitialEditorStateType::Serialized(json) => {
                let parsed_editor_state = editor.parse_editor_state_json(json)?;
                editor.set_editor_state(&parsed_editor_state, tag)?;
            }
            InitialEditorStateType::Json(_) | InitialEditorStateType::Null => {}
        }
    }
    Ok(Box::new(|| {}))
}

fn make() -> Rc<InitialStateExtensionType> {
    let history_merge = || vec![HISTORY_MERGE_TAG.to_owned()];
    define_extension(LexicalExtension {
        init: Some(Rc::new(init)),
        after_registration: Some(Rc::new(after_registration)),
        // These are what `createEditor` registers anyway; listing them
        // lets every `init` see all the known types before the editor
        // exists.
        nodes: vec![
            NodeClass::with_import_json("root", RootNode::import_json),
            NodeClass::of::<TextNode>(),
            NodeClass::of::<LineBreakNode>(),
            NodeClass::of::<TabNode>(),
            NodeClass::of::<ParagraphNode>(),
        ],
        ..LexicalExtension::new(
            "@lexical/extension/InitialState",
            InitialStateConfig {
                update_options: UpdateOptions {
                    tags: history_merge(),
                    ..UpdateOptions::default()
                },
                set_options: EditorSetOptions {
                    tag: Some(HISTORY_MERGE_TAG.to_owned()),
                },
            },
        )
    })
}

thread_local! {
    static INITIAL_STATE_EXTENSION: Rc<InitialStateExtensionType> = make();
}

/// `InitialStateExtension`: sets the editor's initial state from
/// `$initialEditorState` (the last extension that gives one wins; by
/// default an empty root gets a paragraph), in `afterRegistration`, so the
/// initial state may depend on registered commands. Every editor
/// [`build_editor_from_extensions`](super::build_editor_from_extensions)
/// builds has it, first.
pub fn initial_state_extension() -> Rc<InitialStateExtensionType> {
    INITIAL_STATE_EXTENSION.with(Rc::clone)
}
