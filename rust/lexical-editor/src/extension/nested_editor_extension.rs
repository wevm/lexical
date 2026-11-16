//! `NestedEditorExtension.ts`: makes the editor being built a nested editor
//! of a parent one, with the parent's theme and, optionally, its editable
//! mode.

use super::lexical_builder::LexicalBuilder;
use super::signals::{Signal, effect, signal};
use crate::extension_core::{
    ExtensionBuildState, ExtensionConfigBase, ExtensionInitState, ExtensionRegisterState,
    InitialEditorConfig, LexicalExtension, define_extension,
};
use crate::updates::get_active_editor;
use crate::{Editor, LexicalError};
use std::rc::Rc;

/// `NestedEditorConfig.$getParentEditor`: the editor to nest the new one
/// in, compared by identity as JS compares functions.
#[derive(Clone)]
pub struct GetParentEditor(pub Rc<dyn Fn() -> Result<Editor, LexicalError>>);

impl PartialEq for GetParentEditor {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// `$defaultGetParentEditor()`: the active editor (`$getEditor()`), which
/// must have been built from extensions (`LexicalBuilder.fromEditor`).
pub fn default_get_parent_editor() -> Result<Editor, LexicalError> {
    let editor = get_active_editor().ok_or_else(|| {
        LexicalError::new(
            "Unable to find an active editor. This method can only be used synchronously during the callback of editor.update(), editor.read(), or editor.getEditorState().read(..., {editor}).",
        )
    })?;
    LexicalBuilder::from_editor(&editor)?;
    Ok(editor)
}

/// `NestedEditorConfig`.
#[derive(Clone)]
pub struct NestedEditorConfig {
    /// `$getParentEditor`: [`default_get_parent_editor`] by default, so the
    /// nested editor is built in an update or read of its parent.
    pub get_parent_editor: GetParentEditor,
    /// `inheritEditableFromParent`: follow the parent's editable mode.
    pub inherit_editable_from_parent: bool,
}

/// `Partial<NestedEditorConfig>`.
#[derive(Clone, Default)]
pub struct NestedEditorConfigOverrides {
    pub get_parent_editor: Option<GetParentEditor>,
    pub inherit_editable_from_parent: Option<bool>,
}

impl ExtensionConfigBase for NestedEditorConfig {
    type Partial = NestedEditorConfigOverrides;

    fn shallow_merge(&self, o: &NestedEditorConfigOverrides) -> Self {
        NestedEditorConfig {
            get_parent_editor: o
                .get_parent_editor
                .clone()
                .unwrap_or_else(|| self.get_parent_editor.clone()),
            inherit_editable_from_parent: o
                .inherit_editable_from_parent
                .unwrap_or(self.inherit_editable_from_parent),
        }
    }
}

/// `namedSignals({inheritEditableFromParent})`: the extension's output.
#[derive(Clone)]
pub struct NestedEditorOutput {
    pub inherit_editable_from_parent: Signal<bool>,
}

/// `NestedEditorExtension`.
pub type NestedEditorExtensionType = LexicalExtension<NestedEditorConfig, NestedEditorOutput, ()>;

fn make() -> Rc<NestedEditorExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |_: &Editor, config: &NestedEditorConfig, _: &ExtensionBuildState<()>| {
                Ok(NestedEditorOutput {
                    inherit_editable_from_parent: signal(config.inherit_editable_from_parent),
                })
            },
        )),
        init: Some(Rc::new(
            |editor_config: &mut InitialEditorConfig,
             config: &NestedEditorConfig,
             _: &ExtensionInitState| {
                let parent_editor = (config.get_parent_editor.0)()?;
                // `editorConfig.theme || parentEditor._config.theme`: the
                // builder leaves `theme` unset when the extensions have none.
                if editor_config.theme.is_none() {
                    editor_config.theme = Some(parent_editor.config().theme.clone());
                }
                editor_config.parent_editor = Some(parent_editor);
                Ok(())
            },
        )),
        register: Some(Rc::new(
            |editor: &Editor,
             _: &NestedEditorConfig,
             state: &ExtensionRegisterState<(), NestedEditorOutput>| {
                let stores = state
                    .get_output()
                    .expect("NestedEditorExtension has a build");
                let editor = editor.weak();
                Ok(effect(move || {
                    let editor = editor.upgrade().map(Editor)?;
                    let parent_editor = editor.parent_editor()?.clone();
                    if !stores.inherit_editable_from_parent.value() {
                        return None;
                    }
                    editor.set_editable(parent_editor.is_editable());
                    // Weakly, so the parent's listener doesn't keep the
                    // nested editor alive.
                    let editor = editor.weak();
                    Some(parent_editor.register_editable_listener(move |editable| {
                        if let Some(editor) = editor.upgrade().map(Editor) {
                            editor.set_editable(editable);
                        }
                    }))
                }))
            },
        )),
        ..LexicalExtension::new(
            "@lexical/extension/NestedEditor",
            NestedEditorConfig {
                get_parent_editor: GetParentEditor(Rc::new(default_get_parent_editor)),
                inherit_editable_from_parent: false,
            },
        )
    })
}

thread_local! {
    static NESTED_EDITOR_EXTENSION: Rc<NestedEditorExtensionType> = make();
}

/// `NestedEditorExtension`: the editor it builds is nested in the one
/// `$getParentEditor` returns (its `_parentEditor`, so commands dispatched
/// in it reach the parent's listeners), takes the parent's theme unless
/// its extensions give one, and, with `inheritEditableFromParent`, follows
/// the parent's editable mode.
pub fn nested_editor_extension() -> Rc<NestedEditorExtensionType> {
    NESTED_EDITOR_EXTENSION.with(Rc::clone)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extension::{
        LexicalEditorWithDispose, build_editor_from_extensions,
        get_extension_dependency_from_editor,
    };
    use crate::json::Value;
    use crate::{
        AnyLexicalExtensionArgument, COMMAND_PRIORITY_EDITOR, UpdateOptions, config_extension,
        create_command,
    };
    use std::cell::{Cell, RefCell};

    /// `buildEditorFromExtensions({name, theme, dependencies})`.
    fn build(
        name: &str,
        theme: Option<&str>,
        dependencies: Vec<AnyLexicalExtensionArgument>,
    ) -> LexicalEditorWithDispose {
        build_editor_from_extensions([define_extension(LexicalExtension {
            dependencies,
            theme: theme.map(|t| crate::json::parse(t).unwrap().as_object().unwrap().clone()),
            ..LexicalExtension::<(), (), ()>::new(name, ())
        })
        .into()])
        .unwrap()
    }

    fn nested(parent: &Editor, dependencies: Vec<AnyLexicalExtensionArgument>) -> Editor {
        parent
            .read(|| build("child", None, dependencies))
            .unwrap()
            .editor()
            .clone()
    }

    fn text_theme(editor: &Editor, format: &str) -> Option<Value> {
        editor
            .config()
            .theme
            .get("text")
            .and_then(|t| t.get(format))
            .cloned()
    }

    /// The JS unit tests `it sets _parentEditor implicitly` and
    /// `$getParentEditor can be overridden`.
    #[test]
    fn sets_the_parent_editor() {
        let parent = build("parent", None, vec![]);
        let child = nested(&parent, vec![nested_editor_extension().into()]);
        assert!(child.parent_editor() == Some(parent.editor()));
        assert_eq!(child.get_namespace(), parent.get_namespace());
        let given = parent.editor().clone();
        let other = build(
            "child",
            None,
            vec![config_extension(
                &nested_editor_extension(),
                [NestedEditorConfigOverrides {
                    get_parent_editor: Some(GetParentEditor(Rc::new(move || Ok(given.clone())))),
                    ..Default::default()
                }],
            )],
        );
        assert!(other.parent_editor() == Some(parent.editor()));
        // Without an active editor the default has no parent to find.
        let error = build_editor_from_extensions([nested_editor_extension().into()]).err();
        assert!(
            error
                .unwrap()
                .message()
                .starts_with("Unable to find an active editor")
        );
    }

    /// `The theme is inherited by default` and `If the child has a theme it
    /// is not inherited`.
    #[test]
    fn inherits_the_theme() {
        let parent = build(
            "parent",
            Some(r#"{"text":{"bold":"bold","italic":"italic"}}"#),
            vec![],
        );
        let child = nested(&parent, vec![nested_editor_extension().into()]);
        assert_eq!(text_theme(&child, "bold"), Some("bold".into()));
        let own = parent
            .read(|| {
                build(
                    "child",
                    Some(r#"{"text":{"italic":"child-italic"}}"#),
                    vec![nested_editor_extension().into()],
                )
            })
            .unwrap();
        assert_eq!(text_theme(&own, "bold"), None);
        assert_eq!(text_theme(&own, "italic"), Some("child-italic".into()));
    }

    /// `inheritEditableFromParent defaults false but can be enabled later`.
    #[test]
    fn inherits_the_editable_mode() {
        let parent = build("parent", None, vec![]);
        let child = nested(&parent, vec![nested_editor_extension().into()]);
        child.set_editable(false);
        let output = get_extension_dependency_from_editor(&child, &nested_editor_extension())
            .unwrap()
            .output
            .unwrap();
        output.inherit_editable_from_parent.set_value(true);
        assert!(child.is_editable());
        parent.set_editable(false);
        assert!(!child.is_editable());
    }

    /// `Commands delegate to parent synchronously when parent is not
    /// updating` and `... asynchronously when parent is updating`.
    #[test]
    fn commands_delegate_to_the_parent() {
        let command = create_command::<String>("TEST_COMMAND");
        let parent = build("parent", None, vec![]);
        let child = nested(&parent, vec![nested_editor_extension().into()]);
        let calls: Rc<RefCell<Vec<&str>>> = Rc::default();
        let (c, from) = (calls.clone(), child.clone());
        let _child_listener = child.register_command(
            &command,
            move |_, editor| {
                assert!(*editor == from);
                assert!(get_active_editor().as_ref() == Some(&from));
                c.borrow_mut().push("child");
                Ok(false)
            },
            COMMAND_PRIORITY_EDITOR,
        );
        let (c, from, active) = (calls.clone(), child.clone(), parent.editor().clone());
        let _parent_listener = parent.register_command(
            &command,
            move |_, editor| {
                assert!(*editor == from);
                assert!(get_active_editor().as_ref() == Some(&active));
                c.borrow_mut().push("parent");
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        );
        let discrete = || UpdateOptions {
            discrete: true,
            ..UpdateOptions::default()
        };
        let result = Rc::new(Cell::new(None));
        let (r, c, cmd, ch) = (
            result.clone(),
            calls.clone(),
            command.clone(),
            child.clone(),
        );
        child
            .update(
                move || {
                    r.set(Some(ch.dispatch_command(&cmd, "payload".to_owned())?));
                    assert_eq!(*c.borrow(), ["child", "parent"]);
                    Ok(())
                },
                discrete(),
            )
            .unwrap();
        assert_eq!(result.get(), Some(true));
        calls.borrow_mut().clear();
        let (r, c, cmd, ch) = (
            result.clone(),
            calls.clone(),
            command.clone(),
            child.clone(),
        );
        parent
            .update(
                move || {
                    r.set(Some(ch.dispatch_command(&cmd, "payload".to_owned())?));
                    // The parent's listener waits for its update.
                    assert_eq!(*c.borrow(), ["child"]);
                    Ok(())
                },
                discrete(),
            )
            .unwrap();
        assert_eq!(result.get(), Some(false));
        assert_eq!(*calls.borrow(), ["child", "parent"]);
    }
}
