//! `NormalizeTripleClickSelectionExtension.ts`: `$fixFocusOverselection`
//! and `NormalizeTripleClickSelectionExtension`.

use super::named_signals::named_signals_config;
use super::signals::effect;
use crate::extension_core::{
    ExtensionBuildState, ExtensionRegisterState, LexicalExtension, define_extension,
};
use crate::{
    CLICK_COMMAND, COMMAND_PRIORITY_BEFORE_CRITICAL, CaretDirection, Editor, LexicalError,
    caret_range_from_selection, get_caret_range, get_caret_range_in_direction, get_child_caret,
    get_selection, get_sibling_caret, normalize_caret, rewind_sibling_caret,
    set_selection_from_caret_range,
};
use std::rc::Rc;

/// `$fixFocusOverselection()`: a triple click selects a whole block, and
/// the selection's focus lands at the start of the next one, so formatting
/// the selection would reach into that block. Moves the focus back to the
/// end of the clicked block's content: out of the next text node when none
/// of it is selected, behind a single line break, out of the start of any
/// elements, and into the element it then follows. The config's default.
///
/// JS also writes the new selection to the DOM at once, to avoid a flash of
/// the overselection; the app's view draws it with the update.
pub fn fix_focus_overselection() -> Result<(), LexicalError> {
    let Some(selection) = get_selection() else {
        return Ok(());
    };
    let Some(range_selection) = selection.range() else {
        return Ok(());
    };
    if range_selection.is_collapsed() {
        return Ok(());
    }
    let range = get_caret_range_in_direction(
        &caret_range_from_selection(&range_selection)?,
        CaretDirection::Next,
    )?;
    let anchor_origin = range.anchor.origin().clone();
    let mut focus_caret = range.focus.clone();
    // Move it out of the next TextNode if none of it is selected
    if focus_caret.is_text_point_caret()
        && anchor_origin != *focus_caret.origin()
        && focus_caret.offset() == Some(0)
    {
        focus_caret = rewind_sibling_caret(&focus_caret.get_sibling_caret())?;
    }
    // Move it behind a single LineBreakNode
    if focus_caret.is_sibling_caret()
        && anchor_origin != *focus_caret.origin()
        && focus_caret.origin().get_type() == "linebreak"
    {
        focus_caret = rewind_sibling_caret(&focus_caret)?;
    }
    // Move the focus out of the start of any elements
    while focus_caret.is_child_caret() && anchor_origin != *focus_caret.origin() {
        focus_caret = rewind_sibling_caret(&get_sibling_caret(
            focus_caret.origin(),
            CaretDirection::Next,
        ))?;
    }
    // Move it inside the containing element
    if focus_caret.is_sibling_caret() && focus_caret.origin().is_element() {
        let child = get_child_caret(focus_caret.origin(), CaretDirection::Previous)
            .expect("an element has a child caret");
        focus_caret = normalize_caret(&child).get_flipped()?;
    }
    focus_caret = normalize_caret(&focus_caret);
    if !focus_caret.is_same_point_caret(&range.focus) {
        set_selection_from_caret_range(&get_caret_range(range.anchor.clone(), focus_caret)?)?;
    }
    Ok(())
}

/// `$fixFocusOverselection`: the update function a triple click calls,
/// compared by identity as JS compares functions.
#[derive(Clone)]
pub struct FixFocusOverselection(pub Rc<dyn Fn() -> Result<(), LexicalError>>);

impl PartialEq for FixFocusOverselection {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

named_signals_config! {
    /// `NormalizeTripleClickSelectionConfig`. JS's deprecated
    /// `thresholdMsec` and `dateNow`, which nothing reads, aren't ported.
    pub struct NormalizeTripleClickSelectionConfig;
    /// `Partial<NormalizeTripleClickSelectionConfig>`.
    pub struct NormalizeTripleClickSelectionConfigOverrides;
    /// `NormalizeTripleClickSelectionOutput`: `namedSignals(config)`.
    pub struct NormalizeTripleClickSelectionOutput {
        /// `disabled`.
        disabled: bool,
        /// `$fixFocusOverselection`: what a triple click runs,
        /// [`fix_focus_overselection`] by default.
        fix_focus_overselection: FixFocusOverselection,
    }
}

/// `NormalizeTripleClickSelectionExtension`.
pub type NormalizeTripleClickSelectionExtensionType =
    LexicalExtension<NormalizeTripleClickSelectionConfig, NormalizeTripleClickSelectionOutput, ()>;

fn make() -> Rc<NormalizeTripleClickSelectionExtensionType> {
    define_extension(LexicalExtension {
        build: Some(Rc::new(
            |_: &Editor,
             config: &NormalizeTripleClickSelectionConfig,
             _: &ExtensionBuildState<()>| {
                Ok(NormalizeTripleClickSelectionOutput::of(config))
            },
        )),
        register: Some(Rc::new(
            |editor: &Editor,
             _: &NormalizeTripleClickSelectionConfig,
             state: &ExtensionRegisterState<(), NormalizeTripleClickSelectionOutput>| {
                let output = state
                    .get_output()
                    .expect("NormalizeTripleClickSelectionExtension has a build");
                let editor = editor.weak();
                Ok(effect(move || {
                    if output.disabled.value() {
                        return None;
                    }
                    let editor = editor.upgrade().map(Editor)?;
                    let fix = output.fix_focus_overselection.clone();
                    Some(editor.register_command(
                        &CLICK_COMMAND,
                        move |event, _| {
                            if event.detail >= 3 {
                                (fix.peek().0)()?;
                            }
                            Ok(false)
                        },
                        COMMAND_PRIORITY_BEFORE_CRITICAL,
                    ))
                }))
            },
        )),
        ..LexicalExtension::new(
            "@lexical/NormalizeTripleClickSelection",
            NormalizeTripleClickSelectionConfig {
                disabled: false,
                fix_focus_overselection: FixFocusOverselection(Rc::new(fix_focus_overselection)),
            },
        )
    })
}

thread_local! {
    static NORMALIZE_TRIPLE_CLICK_SELECTION_EXTENSION:
        Rc<NormalizeTripleClickSelectionExtensionType> = make();
}

/// `NormalizeTripleClickSelectionExtension`: trims the block selection a
/// triple (or later) click makes so that it ends inside the clicked block
/// ([`fix_focus_overselection`], or the config's replacement, which the
/// output keeps as a signal so other code can call or wrap it).
///
/// JS arms on a `mousedown` whose `detail` is 3 or more and trims the next
/// selection change that matches the DOM, as the browser reports its
/// selection asynchronously. A native view has synced its selection by the
/// time it reports the click, which it reports as [`CLICK_COMMAND`] with
/// the click count as [`detail`](crate::MouseEvent::detail), so the
/// extension trims the selection when that command arrives, at
/// [`COMMAND_PRIORITY_BEFORE_CRITICAL`] (JS's priority for its
/// `SELECTION_CHANGE_COMMAND` handler), as Swift's does.
pub fn normalize_triple_click_selection_extension() -> Rc<NormalizeTripleClickSelectionExtensionType>
{
    NORMALIZE_TRIPLE_CLICK_SELECTION_EXTENSION.with(Rc::clone)
}
