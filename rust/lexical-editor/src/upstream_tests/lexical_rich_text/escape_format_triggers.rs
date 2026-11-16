//! Ports of
//! `packages/lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts`.

#![cfg(feature = "extension")]

use crate::constants::{IS_CAPITALIZE, IS_CODE, IS_LOWERCASE, IS_UPPERCASE};
use crate::extension::{
    LexicalEditorWithDispose, build_editor_from_extensions, get_extension_dependency_from_editor,
};
use crate::rich_text::{
    EscapeFormatTriggerConfig, RichTextConfigOverrides, TriggerConfig, rich_text_extension,
};
use crate::{
    AnyLexicalExtensionArgument, CLICK_COMMAND, InitialEditorStateType, KEY_ARROW_LEFT_COMMAND,
    KEY_ARROW_RIGHT_COMMAND, KEY_ENTER_COMMAND, KEY_SPACE_COMMAND, KEY_TAB_COMMAND, KeyboardEvent,
    LexicalError, LexicalExtension, MouseEvent, NodeRef, PointType, SelectionRef, config_extension,
    create_paragraph_node, create_text_node, define_extension, get_root, get_selection,
};
use std::rc::Rc;

/// `$updateSelectionFormat(selection, expectFormat)`: what selection change
/// handling does, the selection's format set from its text node's.
fn update_selection_format(
    selection: SelectionRef,
    expect_format: i32,
) -> Result<(), LexicalError> {
    let range = selection.range().expect("a range");
    assert!(
        range.anchor.point_type == PointType::Text
            && range.focus.point_type == PointType::Text
            && range.anchor.key == range.focus.key,
        "Selection must have anchor and focus on the same TextNode"
    );
    let format = range.anchor.get_node()?.text_data().get_format();
    assert_eq!(format, f64::from(expect_format));
    selection.set_format(format);
    Ok(())
}

fn build(
    initial: impl Fn() -> Result<(), LexicalError> + 'static,
    rich_text: AnyLexicalExtensionArgument,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(move |_| initial()))),
        dependencies: vec![rich_text],
        ..LexicalExtension::<(), (), ()>::new("test", ())
    })
    .into()])
    .unwrap()
}

fn with_triggers(escape_format_triggers: EscapeFormatTriggerConfig) -> AnyLexicalExtensionArgument {
    config_extension(
        &rich_text_extension(),
        [RichTextConfigOverrides {
            escape_format_triggers: Some(escape_format_triggers),
            ..RichTextConfigOverrides::default()
        }],
    )
}

/// `createEditor($initialEditorState)`: code escapes on arrow, click and
/// Enter, only at its boundary.
fn create_editor(
    initial: impl Fn() -> Result<(), LexicalError> + 'static,
) -> LexicalEditorWithDispose {
    build(
        initial,
        with_triggers(vec![(
            "code".to_owned(),
            Some(TriggerConfig {
                arrow: Some(true),
                click: Some(true),
                enter: Some(true),
                only_at_boundary: Some(true),
                ..TriggerConfig::default()
            }),
        )]),
    )
}

fn first_text_node() -> NodeRef {
    let node = get_root().get_first_descendant().unwrap();
    assert!(node.is_text());
    node
}

fn text_with_format(text: &str, format: &str) -> NodeRef {
    let node = create_text_node(text);
    node.toggle_format(format);
    node
}

/// `$setupCodeTextNode()`.
fn setup_code_text_node() -> Result<(), LexicalError> {
    let paragraph = create_paragraph_node();
    paragraph.append(&[text_with_format("code text", "code")])?;
    get_root().append(&[paragraph])
}

fn code_at_end() -> Result<(), LexicalError> {
    setup_code_text_node()?;
    update_selection_format(get_root().select_end()?, IS_CODE)
}

fn code_at_start() -> Result<(), LexicalError> {
    setup_code_text_node()?;
    update_selection_format(get_root().select_start()?, IS_CODE)
}

fn code_in_middle() -> Result<(), LexicalError> {
    setup_code_text_node()?;
    update_selection_format(first_text_node().select(Some(3), Some(3))?, IS_CODE)
}

fn key(key: &str, shift_key: bool) -> Rc<KeyboardEvent> {
    Rc::new(KeyboardEvent {
        key: key.to_owned(),
        shift_key,
        ..KeyboardEvent::default()
    })
}

fn click(editor: &LexicalEditorWithDispose) {
    editor
        .dispatch_command(&CLICK_COMMAND, Rc::new(MouseEvent::new("click")))
        .unwrap();
}

/// The selection's `(format, style)`.
fn selection_format(editor: &LexicalEditorWithDispose) -> (f64, String) {
    editor
        .read(|| {
            let s = get_selection().and_then(|s| s.range()).expect("a range");
            (s.format, s.style)
        })
        .unwrap()
}

fn expect_cleared(editor: &LexicalEditorWithDispose) {
    assert_eq!(selection_format(editor), (0.0, String::new()));
}

fn expect_code(editor: &LexicalEditorWithDispose) {
    assert_eq!(selection_format(editor).0, f64::from(IS_CODE));
}

fn selection_has_format(editor: &LexicalEditorWithDispose, format: &'static str) -> bool {
    editor
        .read(|| {
            get_selection()
                .filter(SelectionRef::is_range)
                .expect("a range")
                .has_format(format)
        })
        .unwrap()
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > CLICK_COMMAND > clears format when clicking at the end of a code node with no next sibling
#[test]
fn clears_format_when_clicking_at_the_end_of_a_code_node() {
    let editor = create_editor(code_at_end);
    click(&editor);
    expect_cleared(&editor);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > CLICK_COMMAND > clears format when clicking at the start of a code node with no previous sibling
#[test]
fn clears_format_when_clicking_at_the_start_of_a_code_node() {
    let editor = create_editor(code_at_start);
    click(&editor);
    expect_cleared(&editor);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > CLICK_COMMAND > does not clear format when clicking in the middle of a code node
#[test]
fn does_not_clear_format_when_clicking_in_the_middle_of_a_code_node() {
    let editor = create_editor(code_in_middle);
    click(&editor);
    expect_code(&editor);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > CLICK_COMMAND > does not clear format at end of code node when it has a next sibling
#[test]
fn does_not_clear_format_at_end_of_code_node_when_it_has_a_next_sibling() {
    let editor = create_editor(|| {
        let code_node = text_with_format("code", "code");
        let paragraph = create_paragraph_node();
        paragraph.append(&[code_node.clone(), create_text_node(" plain")])?;
        get_root().append(&[paragraph])?;
        update_selection_format(code_node.select(None, None)?, IS_CODE)
    });
    click(&editor);
    expect_code(&editor);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > KEY_ENTER_COMMAND > clears format when pressing Enter at the end of a code node
#[test]
fn clears_format_when_pressing_enter_at_the_end_of_a_code_node() {
    let editor = create_editor(code_at_end);
    editor.dispatch_command(&KEY_ENTER_COMMAND, None).unwrap();
    expect_cleared(&editor);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > KEY_ENTER_COMMAND > clears format when pressing Enter at the start of a code node
#[test]
fn clears_format_when_pressing_enter_at_the_start_of_a_code_node() {
    let editor = create_editor(code_at_start);
    editor.dispatch_command(&KEY_ENTER_COMMAND, None).unwrap();
    expect_cleared(&editor);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > KEY_ENTER_COMMAND > preserves code format when pressing Enter in the middle of a code node
#[test]
fn preserves_code_format_when_pressing_enter_in_the_middle_of_a_code_node() {
    let editor = create_editor(code_in_middle);
    editor.dispatch_command(&KEY_ENTER_COMMAND, None).unwrap();
    assert_ne!(selection_format(&editor).0, 0.0);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > KEY_ARROW_RIGHT_COMMAND > clears format when arrowing right at the end of a code node
#[test]
fn clears_format_when_arrowing_right_at_the_end_of_a_code_node() {
    let editor = create_editor(code_at_end);
    editor
        .dispatch_command(&KEY_ARROW_RIGHT_COMMAND, key("", false))
        .unwrap();
    expect_cleared(&editor);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > KEY_ARROW_RIGHT_COMMAND > does not clear format when shift is held (extending selection)
#[test]
fn arrow_right_does_not_clear_format_when_shift_is_held() {
    let editor = create_editor(code_at_end);
    editor
        .dispatch_command(&KEY_ARROW_RIGHT_COMMAND, key("ArrowRight", true))
        .unwrap();
    expect_code(&editor);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > KEY_ARROW_RIGHT_COMMAND > does not clear format in the middle of a code node
#[test]
fn arrow_right_does_not_clear_format_in_the_middle_of_a_code_node() {
    let editor = create_editor(code_in_middle);
    editor
        .dispatch_command(&KEY_ARROW_RIGHT_COMMAND, key("ArrowRight", false))
        .unwrap();
    expect_code(&editor);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > KEY_ARROW_LEFT_COMMAND > clears format when arrowing left at the start of a code node
#[test]
fn clears_format_when_arrowing_left_at_the_start_of_a_code_node() {
    let editor = create_editor(code_at_start);
    editor
        .dispatch_command(&KEY_ARROW_LEFT_COMMAND, key("ArrowLeft", false))
        .unwrap();
    expect_cleared(&editor);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > KEY_ARROW_LEFT_COMMAND > does not clear format when shift is held (extending selection)
#[test]
fn arrow_left_does_not_clear_format_when_shift_is_held() {
    let editor = create_editor(code_at_start);
    editor
        .dispatch_command(&KEY_ARROW_LEFT_COMMAND, key("ArrowLeft", true))
        .unwrap();
    expect_code(&editor);
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers > KEY_ARROW_LEFT_COMMAND > does not clear format in the middle of a code node
#[test]
fn arrow_left_does_not_clear_format_in_the_middle_of_a_code_node() {
    let editor = create_editor(code_in_middle);
    editor
        .dispatch_command(&KEY_ARROW_LEFT_COMMAND, key("ArrowLeft", false))
        .unwrap();
    expect_code(&editor);
    editor.dispose();
}

/// `$initialEditorState` of the capitalization tests: `hello` with
/// `format`, the selection at the end with its format.
fn hello_with(format: &'static str, flag: i32) -> impl Fn() -> Result<(), LexicalError> + 'static {
    move || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[text_with_format("hello", format)])?;
        let root = get_root();
        root.append(&[paragraph])?;
        update_selection_format(root.select_end()?, flag)
    }
}

#[derive(Clone, Copy)]
enum Key {
    Enter,
    Space,
    Tab,
}

/// `clears on ${COMMAND.type}` of `${format} format`.
fn clears_on(format: &'static str, flag: i32, command: Key) {
    let editor = build(hello_with(format, flag), rich_text_extension().into());
    let event = key("", false);
    match command {
        Key::Enter => editor.dispatch_command(&KEY_ENTER_COMMAND, Some(event)),
        Key::Space => editor.dispatch_command(&KEY_SPACE_COMMAND, event),
        Key::Tab => editor.dispatch_command(&KEY_TAB_COMMAND, event),
    }
    .unwrap();
    assert!(!selection_has_format(&editor, format));
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension default capitalization reset > capitalize format > clears on KEY_ENTER_COMMAND
#[test]
fn capitalize_clears_on_key_enter_command() {
    clears_on("capitalize", IS_CAPITALIZE, Key::Enter);
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension default capitalization reset > capitalize format > clears on KEY_SPACE_COMMAND
#[test]
fn capitalize_clears_on_key_space_command() {
    clears_on("capitalize", IS_CAPITALIZE, Key::Space);
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension default capitalization reset > capitalize format > clears on KEY_TAB_COMMAND
#[test]
fn capitalize_clears_on_key_tab_command() {
    clears_on("capitalize", IS_CAPITALIZE, Key::Tab);
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension default capitalization reset > uppercase format > clears on KEY_ENTER_COMMAND
#[test]
fn uppercase_clears_on_key_enter_command() {
    clears_on("uppercase", IS_UPPERCASE, Key::Enter);
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension default capitalization reset > uppercase format > clears on KEY_SPACE_COMMAND
#[test]
fn uppercase_clears_on_key_space_command() {
    clears_on("uppercase", IS_UPPERCASE, Key::Space);
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension default capitalization reset > uppercase format > clears on KEY_TAB_COMMAND
#[test]
fn uppercase_clears_on_key_tab_command() {
    clears_on("uppercase", IS_UPPERCASE, Key::Tab);
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension default capitalization reset > lowercase format > clears on KEY_ENTER_COMMAND
#[test]
fn lowercase_clears_on_key_enter_command() {
    clears_on("lowercase", IS_LOWERCASE, Key::Enter);
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension default capitalization reset > lowercase format > clears on KEY_SPACE_COMMAND
#[test]
fn lowercase_clears_on_key_space_command() {
    clears_on("lowercase", IS_LOWERCASE, Key::Space);
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension default capitalization reset > lowercase format > clears on KEY_TAB_COMMAND
#[test]
fn lowercase_clears_on_key_tab_command() {
    clears_on("lowercase", IS_LOWERCASE, Key::Tab);
}

/// `buildEditorWithEscapeFormatTriggers(escapeFormatTriggers)`.
fn build_editor_with_escape_format_triggers(
    escape_format_triggers: EscapeFormatTriggerConfig,
) -> LexicalEditorWithDispose {
    build(
        hello_with("uppercase", IS_UPPERCASE),
        with_triggers(escape_format_triggers),
    )
}

fn uppercase_disabled() -> EscapeFormatTriggerConfig {
    vec![("uppercase".to_owned(), None)]
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers mergeConfig > overriding one format preserves default capitalization resets
#[test]
fn overriding_one_format_preserves_default_capitalization_resets() {
    let editor = build_editor_with_escape_format_triggers(vec![(
        "code".to_owned(),
        Some(TriggerConfig {
            enter: Some(true),
            only_at_boundary: Some(true),
            ..TriggerConfig::default()
        }),
    )]);
    editor.dispatch_command(&KEY_ENTER_COMMAND, None).unwrap();
    assert!(!selection_has_format(&editor, "uppercase"));
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers mergeConfig > null disables a default format
#[test]
fn null_disables_a_default_format() {
    let editor = build_editor_with_escape_format_triggers(uppercase_disabled());
    editor.dispatch_command(&KEY_ENTER_COMMAND, None).unwrap();
    assert!(selection_has_format(&editor, "uppercase"));
    editor.dispose();
}

// upstream: lexical-rich-text/src/__tests__/unit/EscapeFormatTriggers.test.ts::RichTextExtension escapeFormatTriggers mergeConfig > escapeFormatTriggers can be modified as a signal
#[test]
fn escape_format_triggers_can_be_modified_as_a_signal() {
    let editor = build_editor_with_escape_format_triggers(uppercase_disabled());
    // re-enable the behavior here
    let triggers = get_extension_dependency_from_editor(&editor, &rich_text_extension())
        .unwrap()
        .output
        .unwrap()
        .escape_format_triggers
        .clone();
    let mut value = triggers.peek();
    let uppercase = value
        .iter_mut()
        .find(|(format, _)| format == "uppercase")
        .expect("uppercase is configured");
    assert!(uppercase.1.is_none());
    uppercase.1 = Some(TriggerConfig {
        enter: Some(true),
        ..TriggerConfig::default()
    });
    triggers.set_value(value);
    editor.dispatch_command(&KEY_ENTER_COMMAND, None).unwrap();
    assert!(!selection_has_format(&editor, "uppercase"));
    editor.dispose();
}
