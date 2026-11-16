//! The typeahead and node menus of `@lexical/react` without their views
//! (the `menu` feature): `LexicalTypeaheadMenuPluginUtils.ts`
//! ([`PUNCTUATION`], [`basic_typeahead_trigger_match`]),
//! `LexicalMenuOption.ts` ([`MenuOption`]), and the platform-neutral parts
//! of `shared/LexicalMenu.tsx` ([`MenuTextMatch`], [`MenuResolution`],
//! [`get_full_match_offset`], [`split_node_containing_query`], the menu's
//! state and option selection, [`MenuState`]),
//! `LexicalTypeaheadMenuPlugin.tsx` ([`get_text_up_to_anchor`],
//! [`is_selection_on_entity_boundary`], [`register_typeahead_menu`]) and
//! `LexicalNodeMenuPlugin.tsx` ([`NodeMenu`]).
//!
//! The app's view draws the menu: it reads [`MenuState`] (its
//! [`resolution`](MenuState::resolution), options and highlighted index)
//! and places it at the resolution's model range, where JS positions an
//! anchor `div` at a DOM `Range`'s rect (`tryToPositionRange`).
//!
//! While a menu is open it handles the keys the core's keydown handler
//! dispatches, at its `commandPriority`: the up and down arrows move the
//! highlight, wrapping around, and dispatch
//! [`SCROLL_TYPEAHEAD_OPTION_INTO_VIEW_COMMAND`] for the view to scroll to
//! it; Escape closes the menu; Tab and Enter (not Shift+Enter) choose the
//! highlighted option.
//!
//! Not ported: an `onClose` that returns a promise; and the React
//! rendering, the anchor element and its scroll handling (rust/PARITY.md).

mod lexical_typeahead_menu_plugin_utils;

pub use lexical_typeahead_menu_plugin_utils::{
    PUNCTUATION, SCROLL_TYPEAHEAD_OPTION_INTO_VIEW_COMMAND, ScrollTypeaheadOptionIntoViewPayload,
    TypeaheadTriggerOptions, basic_typeahead_trigger_match,
};

use crate::commands::{
    KEY_ARROW_DOWN_COMMAND, KEY_ARROW_UP_COMMAND, KEY_ENTER_COMMAND, KEY_ESCAPE_COMMAND,
    KEY_TAB_COMMAND,
};
use crate::editor::COMMAND_PRIORITY_LOW;
use crate::events::KeyboardEvent;
use crate::lexical_selection::{PointType, RangeSelection, get_selection};
use crate::merge_register::merge_register;
use crate::{Editor, LexicalError, NodeKey, NodeRef, Unregister, UpdateOptions, utf16};
use std::cell::RefCell;
use std::rc::Rc;

/// `MenuTextMatch`: where a trigger matched the text before the caret: the
/// `leadOffset` (UTF-16) where the match starts, the query after the
/// trigger (`matchingString`), and the text, trigger included, that an
/// option replaces (`replaceableString`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuTextMatch {
    pub lead_offset: usize,
    pub matching_string: String,
    pub replaceable_string: String,
}

/// `TriggerFn`: the match of the text before the caret that opens a menu.
pub type TriggerFn = Rc<dyn Fn(&str, &Editor) -> Option<MenuTextMatch>>;

/// `MenuOption`: an item of a typeahead or node menu, with a unique key.
/// JS's `ref` and `icon` belong to the view that draws the option.
pub trait MenuOption {
    /// `key`.
    fn key(&self) -> &str;
    /// `title`: the text the menu shows for the option.
    fn title(&self) -> Option<&str> {
        None
    }
}

/// `MenuResolution`: what an open menu is anchored at. JS carries a
/// `getRect` over a DOM `Range`; here the anchor is the model range from
/// the trigger to the caret (`anchor_key`, UTF-16 offsets), which the view
/// measures. A node menu has no match and anchors at its node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuResolution {
    /// `match`.
    pub text_match: Option<MenuTextMatch>,
    /// The text node the range is in, or the node a node menu is for.
    pub anchor_key: NodeKey,
    pub start_offset: usize,
    pub end_offset: usize,
}

/// `getFullMatchOffset(documentText, entryText, offset)`: walks back along
/// the typed text and forward through the entry's text, to replace as much
/// of what was typed as matches the entry. In UTF-16 code units.
pub fn get_full_match_offset(document_text: &str, entry_text: &str, offset: usize) -> usize {
    let document: Vec<u16> = document_text.encode_utf16().collect();
    let entry: Vec<u16> = entry_text.encode_utf16().collect();
    let mut trigger_offset = offset;
    for i in offset..=entry.len() {
        // `documentText.slice(-i)`: the whole text for 0.
        let suffix = if i == 0 {
            &document[..]
        } else {
            &document[document.len().saturating_sub(i)..]
        };
        if suffix == &entry[..i] {
            trigger_offset = i;
        }
    }
    trigger_offset
}

/// The anchor of a collapsed range selection in simple text, with the
/// text before it: what `$splitNodeContainingQuery` and
/// `getTextUpToAnchor` both read.
fn anchor_text(selection: &RangeSelection) -> Option<(NodeRef, String)> {
    let anchor = &selection.anchor;
    if anchor.point_type != PointType::Text {
        return None;
    }
    let anchor_node = anchor.get_node().ok()?;
    if !anchor_node.is_simple_text() {
        return None;
    }
    let text = utf16::slice(&anchor_node.get_text_content(), 0, anchor.offset);
    Some((anchor_node, text))
}

/// `$splitNodeContainingQuery(match)`: splits the text node at the caret so
/// the query, with its trigger, is a node of its own, and returns it;
/// `None` without a collapsed range selection in simple text, or when the
/// query starts before the node.
pub fn split_node_containing_query(
    text_match: &MenuTextMatch,
) -> Result<Option<NodeRef>, LexicalError> {
    let Some(selection) = get_selection().and_then(|s| s.range()) else {
        return Ok(None);
    };
    if !selection.is_collapsed() {
        return Ok(None);
    }
    let Some((anchor_node, text_content)) = anchor_text(&selection) else {
        return Ok(None);
    };
    let selection_offset = selection.anchor.offset;
    let character_offset = utf16::len(&text_match.replaceable_string);
    let query_offset =
        get_full_match_offset(&text_content, &text_match.matching_string, character_offset);
    let Some(start_offset) = selection_offset.checked_sub(query_offset) else {
        return Ok(None);
    };
    let parts = if start_offset == 0 {
        anchor_node.split_text(&[selection_offset as i64])?
    } else {
        let mut parts = anchor_node.split_text(&[start_offset as i64, selection_offset as i64])?;
        if parts.is_empty() {
            parts
        } else {
            parts.split_off(1)
        }
    };
    Ok(parts.into_iter().next())
}

/// `getTextUpToAnchor(selection)`: the text of the anchor's node before
/// it, when the anchor is in simple text.
pub fn get_text_up_to_anchor(selection: &RangeSelection) -> Option<String> {
    anchor_text(selection).map(|(_, text)| text)
}

/// `isSelectionOnEntityBoundary(editor, offset)`: whether a match at
/// `offset` 0 follows a text entity (a hashtag), in the editor's latest
/// state.
pub fn is_selection_on_entity_boundary(editor: &Editor, offset: usize) -> bool {
    if offset != 0 {
        return false;
    }
    editor.get_editor_state().read(|| {
        get_selection()
            .and_then(|s| s.range())
            .and_then(|range| range.anchor.get_node().ok())
            .and_then(|node| node.get_previous_sibling())
            .is_some_and(|prev| prev.is_text() && prev.is_text_entity())
    })
}

/// What a menu runs when an option is chosen (`onSelectOption`): the
/// option, the text node holding the query (split off when the menu splits
/// it), what closes the menu, and the query.
pub type MenuSelectOption<O> =
    Rc<dyn Fn(&Rc<O>, Option<NodeRef>, &dyn Fn(), &str) -> Result<(), LexicalError>>;

struct MenuInner<O> {
    resolution: Option<MenuResolution>,
    options: Vec<Rc<O>>,
    /// The highlighted index as set: -1 after an arrow key found no option
    /// to highlight (JS's `updateSelectedIndex(-1)`).
    selected_index: Option<isize>,
    /// Whether the menu is open as far as `onOpen` and `onClose` are
    /// concerned (`isOpenRef`).
    is_open: bool,
    /// Whether the view draws the menu with no options (JS's
    /// `menuRenderFn`).
    draws_empty_menu: bool,
    /// What unregisters the keyboard commands while the menu is open.
    unregister_commands: Option<Unregister>,
}

/// An open or closed menu and its options: the state `LexicalMenu`,
/// `LexicalTypeaheadMenuPlugin` and `LexicalNodeMenuPlugin` keep in React.
/// The view that draws the menu is told of each change (`on_change`), and
/// calls [`select_option_and_clean_up`](Self::select_option_and_clean_up)
/// and [`set_highlighted_index`](Self::set_highlighted_index) for its input.
/// Cloning it shares the menu.
pub struct MenuState<O> {
    editor: Editor,
    inner: Rc<RefCell<MenuInner<O>>>,
    command_priority: u8,
    preselect_first_item: bool,
    should_split_node_with_query: bool,
    on_select_option: MenuSelectOption<O>,
    callbacks: Rc<MenuCallbacks>,
}

impl<O> Clone for MenuState<O> {
    fn clone(&self) -> Self {
        self.with_editor(&self.editor)
    }
}

impl<O> MenuState<O> {
    /// The same menu, holding `editor`.
    fn with_editor(&self, editor: &Editor) -> Self {
        MenuState {
            editor: editor.clone(),
            inner: self.inner.clone(),
            command_priority: self.command_priority,
            preselect_first_item: self.preselect_first_item,
            should_split_node_with_query: self.should_split_node_with_query,
            on_select_option: self.on_select_option.clone(),
            callbacks: self.callbacks.clone(),
        }
    }

    /// The menu without its editor, which a keyboard command's listener
    /// holds and makes the menu again from the editor it is dispatched in.
    fn detach(&self) -> DetachedMenu<O> {
        DetachedMenu {
            inner: self.inner.clone(),
            command_priority: self.command_priority,
            preselect_first_item: self.preselect_first_item,
            should_split_node_with_query: self.should_split_node_with_query,
            on_select_option: self.on_select_option.clone(),
            callbacks: self.callbacks.clone(),
        }
    }
}

/// A [`MenuState`] without its editor.
struct DetachedMenu<O> {
    inner: Rc<RefCell<MenuInner<O>>>,
    command_priority: u8,
    preselect_first_item: bool,
    should_split_node_with_query: bool,
    on_select_option: MenuSelectOption<O>,
    callbacks: Rc<MenuCallbacks>,
}

impl<O> DetachedMenu<O> {
    fn menu(&self, editor: &Editor) -> MenuState<O> {
        MenuState {
            editor: editor.clone(),
            inner: self.inner.clone(),
            command_priority: self.command_priority,
            preselect_first_item: self.preselect_first_item,
            should_split_node_with_query: self.should_split_node_with_query,
            on_select_option: self.on_select_option.clone(),
            callbacks: self.callbacks.clone(),
        }
    }
}

/// `onOpen(resolution)`.
pub type OnOpen = Rc<dyn Fn(&MenuResolution)>;

/// `onQueryChange(matchingString)`.
pub type OnQueryChange = Rc<dyn Fn(Option<&str>)>;

/// A menu's `onOpen`, `onClose`, and what the view hears of its changes.
#[derive(Default)]
pub struct MenuCallbacks {
    /// `onOpen(resolution)`.
    pub on_open: RefCell<Option<OnOpen>>,
    /// `onClose()`.
    pub on_close: RefCell<Option<Rc<dyn Fn()>>>,
    /// Called when the resolution, the options or the highlighted index
    /// change, for the view to draw the menu again.
    pub on_change: RefCell<Option<Rc<dyn Fn()>>>,
}

impl<O: MenuOption + 'static> MenuState<O> {
    fn new(
        editor: &Editor,
        command_priority: u8,
        preselect_first_item: bool,
        should_split_node_with_query: bool,
        on_select_option: MenuSelectOption<O>,
    ) -> Self {
        MenuState {
            editor: editor.clone(),
            inner: Rc::new(RefCell::new(MenuInner {
                resolution: None,
                options: Vec::new(),
                selected_index: None,
                is_open: false,
                draws_empty_menu: false,
                unregister_commands: None,
            })),
            command_priority,
            preselect_first_item,
            should_split_node_with_query,
            on_select_option,
            callbacks: Rc::new(MenuCallbacks::default()),
        }
    }

    /// The menu's `onOpen`, `onClose` and change callback.
    pub fn callbacks(&self) -> &MenuCallbacks {
        &self.callbacks
    }

    fn changed(&self) {
        let on_change = self.callbacks.on_change.borrow().clone();
        if let Some(on_change) = on_change {
            on_change();
        }
    }

    /// The resolution while the menu is open, which the view anchors it at.
    pub fn resolution(&self) -> Option<MenuResolution> {
        self.inner.borrow().resolution.clone()
    }

    /// The options (JS's `options` prop).
    pub fn options(&self) -> Vec<Rc<O>> {
        self.inner.borrow().options.clone()
    }

    /// The highlighted option's index, clamped to the options
    /// (`Math.min(options.length - 1, selectedIndex)`); `None` when no
    /// option is highlighted.
    pub fn selected_index(&self) -> Option<usize> {
        self.clamped_index()
            .and_then(|index| usize::try_from(index).ok())
    }

    /// `selectedIndex` as `LexicalMenu` reads it: clamped, and -1 (or
    /// below) when there is no option to highlight.
    fn clamped_index(&self) -> Option<isize> {
        let inner = self.inner.borrow();
        let last = inner.options.len() as isize - 1;
        inner.selected_index.map(|index| index.min(last))
    }

    /// The highlighted option, if any.
    fn selected_option(&self) -> Option<Rc<O>> {
        let index = self.selected_index()?;
        self.inner.borrow().options.get(index).cloned()
    }

    /// The command priority its keyboard commands are registered at (JS's
    /// `commandPriority`, `COMMAND_PRIORITY_LOW` by default).
    pub fn command_priority(&self) -> u8 {
        self.command_priority
    }

    /// Whether the view draws the menu even with no options, as a JS
    /// `menuRenderFn` is called whatever the options are and may show a
    /// "no results" panel. While it is false (the default renderer), an
    /// empty menu draws nothing and its keys pass through to the editor
    /// (`rendersNothing`).
    pub fn set_draws_empty_menu(&self, draws_empty_menu: bool) {
        self.inner.borrow_mut().draws_empty_menu = draws_empty_menu;
    }

    /// `rendersNothing`: the default renderer with no options.
    fn renders_nothing(&self) -> bool {
        let inner = self.inner.borrow();
        !inner.draws_empty_menu && inner.options.is_empty()
    }

    /// Replaces the options (JS renders the menu with new `options`); an
    /// open menu that preselects its first item highlights it.
    pub fn set_options(&self, options: Vec<Rc<O>>) {
        {
            let mut inner = self.inner.borrow_mut();
            inner.options = options;
            if inner.selected_index.is_none()
                && self.preselect_first_item
                && inner.resolution.is_some()
            {
                inner.selected_index = Some(0);
            }
        }
        self.changed();
    }

    /// `setHighlightedIndex(index)`.
    pub fn set_highlighted_index(&self, index: Option<usize>) {
        self.inner.borrow_mut().selected_index = index.map(|i| i as isize);
        self.changed();
    }

    /// `openTypeahead(resolution)` / `openNodeMenu(resolution)`: a new query
    /// highlights the first option again (`preselectFirstItem`).
    fn open(&self, resolution: MenuResolution) {
        let was_open = {
            let mut inner = self.inner.borrow_mut();
            let query = |r: &Option<MenuResolution>| {
                r.as_ref()
                    .and_then(|r| r.text_match.as_ref())
                    .map(|m| m.matching_string.clone())
            };
            if self.preselect_first_item
                && query(&inner.resolution)
                    != resolution
                        .text_match
                        .as_ref()
                        .map(|m| m.matching_string.clone())
            {
                inner.selected_index = Some(0);
            }
            inner.resolution = Some(resolution.clone());
            std::mem::replace(&mut inner.is_open, true)
        };
        if !was_open {
            self.register_commands();
            let on_open = self.callbacks.on_open.borrow().clone();
            if let Some(on_open) = on_open {
                on_open(&resolution);
            }
        }
        self.changed();
    }

    /// `closeTypeahead()` / `closeNodeMenu()`: `onClose`, then the menu is
    /// cleared.
    pub fn close(&self) {
        if !std::mem::replace(&mut self.inner.borrow_mut().is_open, false) {
            return;
        }
        let on_close = self.callbacks.on_close.borrow().clone();
        if let Some(on_close) = on_close {
            on_close();
        }
        let unregister_commands = {
            let mut inner = self.inner.borrow_mut();
            inner.resolution = None;
            inner.selected_index = None;
            inner.unregister_commands.take()
        };
        if let Some(unregister) = unregister_commands {
            unregister();
        }
        self.changed();
    }

    /// The up and down arrows (`forward`): the next or previous option is
    /// highlighted, wrapping around, and scrolled into view
    /// ([`SCROLL_TYPEAHEAD_OPTION_INTO_VIEW_COMMAND`]); with no option to
    /// highlight, none is. A menu that draws nothing lets the key go on.
    fn move_highlight(&self, forward: bool, event: &KeyboardEvent) -> Result<bool, LexicalError> {
        if self.renders_nothing() {
            // There is nothing to move through, and no menu on screen, so
            // the key has to reach whatever would otherwise move the caret.
            return Ok(false);
        }
        let last = self.inner.borrow().options.len() as isize - 1;
        let new_index = match self.clamped_index() {
            None if forward => 0,
            None => last,
            Some(index) if forward => {
                if index != last {
                    index + 1
                } else {
                    0
                }
            }
            Some(index) => {
                if index != 0 {
                    index - 1
                } else {
                    last
                }
            }
        };
        self.inner.borrow_mut().selected_index = Some(new_index);
        let option = usize::try_from(new_index)
            .ok()
            .and_then(|index| self.inner.borrow().options.get(index).cloned());
        let Some(option) = option else {
            self.inner.borrow_mut().selected_index = Some(-1);
            self.changed();
            event.prevent_default();
            return Ok(true);
        };
        self.editor.dispatch_command(
            &SCROLL_TYPEAHEAD_OPTION_INTO_VIEW_COMMAND,
            ScrollTypeaheadOptionIntoViewPayload {
                index: new_index as usize,
                option,
            },
        )?;
        self.changed();
        event.prevent_default();
        Ok(true)
    }

    /// `LexicalMenu`'s keyboard commands, at the menu's command priority,
    /// while it is open.
    fn register_commands(&self) {
        let priority = self.command_priority;
        // Each listener holds the menu without its editor, so the editor's
        // listeners don't keep the editor alive.
        let (down, up, escape, tab, enter) = (
            self.detach(),
            self.detach(),
            self.detach(),
            self.detach(),
            self.detach(),
        );
        let unregister = merge_register(vec![
            self.editor.register_command(
                &KEY_ARROW_DOWN_COMMAND,
                move |event, editor| down.menu(editor).move_highlight(true, event),
                priority,
            ),
            self.editor.register_command(
                &KEY_ARROW_UP_COMMAND,
                move |event, editor| up.menu(editor).move_highlight(false, event),
                priority,
            ),
            self.editor.register_command(
                &KEY_ESCAPE_COMMAND,
                move |event, editor| {
                    let menu = escape.menu(editor);
                    if menu.renders_nothing() {
                        // With nothing on screen there is no menu to dismiss.
                        return Ok(false);
                    }
                    event.prevent_default();
                    menu.close();
                    Ok(true)
                },
                priority,
            ),
            self.editor.register_command(
                &KEY_TAB_COMMAND,
                move |event, editor| {
                    let menu = tab.menu(editor);
                    let Some(option) = menu.selected_option() else {
                        return Ok(false);
                    };
                    event.prevent_default();
                    menu.select_option_and_clean_up(&option)?;
                    Ok(true)
                },
                priority,
            ),
            self.editor.register_command(
                &KEY_ENTER_COMMAND,
                move |event, editor| {
                    let menu = enter.menu(editor);
                    let Some(option) = menu.selected_option() else {
                        return Ok(false);
                    };
                    // Shift+Enter must reach rich text's line break.
                    if event.as_ref().is_some_and(|event| event.shift_key) {
                        return Ok(false);
                    }
                    if let Some(event) = event {
                        event.prevent_default();
                    }
                    menu.select_option_and_clean_up(&option)?;
                    Ok(true)
                },
                priority,
            ),
        ]);
        self.inner.borrow_mut().unregister_commands = Some(unregister);
    }

    /// `selectOptionAndCleanUp(option)`: in an update, splits off the query
    /// (`$splitNodeContainingQuery`) for a typeahead menu, then runs
    /// `onSelectOption`.
    pub fn select_option_and_clean_up(&self, option: &Rc<O>) -> Result<(), LexicalError> {
        let text_match = self.resolution().and_then(|r| r.text_match);
        let (menu, option) = (self.clone(), option.clone());
        self.editor.update(
            move || {
                let text_node_containing_query = match &text_match {
                    Some(text_match) if menu.should_split_node_with_query => {
                        split_node_containing_query(text_match)?
                    }
                    _ => None,
                };
                let close_menu = || menu.close();
                (menu.on_select_option)(
                    &option,
                    text_node_containing_query,
                    &close_menu,
                    text_match
                        .as_ref()
                        .map_or("", |m| m.matching_string.as_str()),
                )
            },
            UpdateOptions::default(),
        )
    }
}

/// `LexicalTypeaheadMenuPlugin`'s options besides its callbacks.
#[derive(Clone, Copy, Debug)]
pub struct TypeaheadMenuOptions {
    /// `commandPriority` (`COMMAND_PRIORITY_LOW`): the priority of the
    /// menu's keyboard commands.
    pub command_priority: u8,
    /// `preselectFirstItem` (true).
    pub preselect_first_item: bool,
    /// `ignoreEntityBoundary` (false): a match right after a text entity
    /// opens the menu too.
    pub ignore_entity_boundary: bool,
}

impl Default for TypeaheadMenuOptions {
    fn default() -> Self {
        TypeaheadMenuOptions {
            command_priority: COMMAND_PRIORITY_LOW,
            preselect_first_item: true,
            ignore_entity_boundary: false,
        }
    }
}

/// `LexicalTypeaheadMenuPlugin`: after each update, opens the menu while
/// `trigger_fn` matches the text before a collapsed caret in an editable
/// editor (and not right after a text entity, unless
/// `ignore_entity_boundary`), reports the query to `on_query_change`, and
/// closes it otherwise; choosing an option replaces the query. Returns the
/// menu and what unregisters it.
pub fn register_typeahead_menu<O: MenuOption + 'static>(
    editor: &Editor,
    trigger_fn: TriggerFn,
    on_query_change: OnQueryChange,
    on_select_option: MenuSelectOption<O>,
    options: TypeaheadMenuOptions,
) -> (MenuState<O>, Unregister) {
    let menu = MenuState::new(
        editor,
        options.command_priority,
        options.preselect_first_item,
        true,
        on_select_option,
    );
    let listening = menu.clone();
    let weak = editor.weak();
    let unregister_update = editor.register_update_listener(move |_| {
        let Some(editor) = weak.upgrade().map(Editor) else {
            return;
        };
        let menu = &listening;
        // `editor.read('latest', ...)`.
        enum Next {
            Close,
            Open(MenuResolution),
        }
        let next = editor.get_editor_state().read(|| {
            if !editor.is_editable() {
                return Next::Close;
            }
            let Some(selection) = get_selection().and_then(|s| s.range()) else {
                return Next::Close;
            };
            let Some(text) = get_text_up_to_anchor(&selection) else {
                return Next::Close;
            };
            if !selection.is_collapsed() {
                return Next::Close;
            }
            let text_match = trigger_fn(&text, &editor);
            on_query_change(text_match.as_ref().map(|m| m.matching_string.as_str()));
            match text_match {
                Some(text_match)
                    if options.ignore_entity_boundary
                        || !is_selection_on_entity_boundary(&editor, text_match.lead_offset) =>
                {
                    // `tryToPositionRange`'s model half: the range from the
                    // trigger to the caret.
                    Next::Open(MenuResolution {
                        start_offset: text_match.lead_offset,
                        end_offset: selection.anchor.offset,
                        anchor_key: selection.anchor.key.clone(),
                        text_match: Some(text_match),
                    })
                }
                _ => Next::Close,
            }
        });
        // There is no composition to wait for (`closeUnlessComposing`).
        match next {
            Next::Close => menu.close(),
            Next::Open(resolution) => menu.open(resolution),
        }
    });
    let closing = menu.clone();
    let unregister_editable = editor.register_editable_listener(move |is_editable| {
        if !is_editable {
            closing.close();
        }
    });
    let unregister = merge_register(vec![unregister_update, unregister_editable]);
    (menu, unregister)
}

/// `LexicalNodeMenuPlugin`: a menu anchored at the node `node_key`, open
/// while it is set and the node exists; [`set_node_key`](Self::set_node_key)
/// is the `nodeKey` prop.
pub struct NodeMenu<O> {
    menu: MenuState<O>,
    node_key: Rc<RefCell<Option<NodeKey>>>,
}

impl<O> Clone for NodeMenu<O> {
    fn clone(&self) -> Self {
        NodeMenu {
            menu: self.menu.clone(),
            node_key: self.node_key.clone(),
        }
    }
}

impl<O: MenuOption + 'static> NodeMenu<O> {
    /// A closed node menu that runs `on_select_option` for the chosen
    /// option, with no query, whose keyboard commands are at
    /// `command_priority` (JS's `commandPriority`, `COMMAND_PRIORITY_LOW`
    /// by default).
    pub fn new(
        editor: &Editor,
        on_select_option: MenuSelectOption<O>,
        command_priority: u8,
    ) -> Self {
        NodeMenu {
            menu: MenuState::new(editor, command_priority, true, false, on_select_option),
            node_key: Rc::new(RefCell::new(None)),
        }
    }

    /// The menu.
    pub fn menu(&self) -> &MenuState<O> {
        &self.menu
    }

    /// The `nodeKey` prop.
    pub fn node_key(&self) -> Option<NodeKey> {
        self.node_key.borrow().clone()
    }

    /// `positionOrCloseMenu()` for a new `nodeKey`: opens the menu at the
    /// node when it exists, and closes it without a key.
    pub fn set_node_key(&self, key: Option<NodeKey>) {
        *self.node_key.borrow_mut() = key.clone();
        let Some(key) = key else {
            if self.menu.resolution().is_some() {
                self.menu.close();
            }
            return;
        };
        let exists = self
            .menu
            .editor
            .get_editor_state()
            .read(|| crate::get_node_by_key(&key).is_some());
        if exists && self.menu.resolution().is_none() {
            self.menu.open(MenuResolution {
                text_match: None,
                anchor_key: key,
                start_offset: 0,
                end_offset: 0,
            });
        }
    }
}

#[cfg(all(test, feature = "rich-text"))]
mod tests {
    //! The menu's keyboard commands, as `swift/Tests/LexicalMenuTests` checks
    //! them (JS's are React hooks, which no vector records): the arrows move
    //! the highlight, wrapping around, at the menu's priority; Enter and Tab
    //! choose, Shift+Enter is left to rich text; Escape closes; an empty
    //! menu that draws nothing lets the keys go on.
    use super::*;
    use crate::commands::KEY_TAB_COMMAND;
    use crate::editor::COMMAND_PRIORITY_EDITOR;
    use crate::{
        CreateEditorArgs, create_editor, create_paragraph_node, create_text_node, get_root,
        rich_text,
    };
    use std::cell::Cell;

    struct Item(String);

    impl MenuOption for Item {
        fn key(&self) -> &str {
            &self.0
        }
    }

    struct Fixture {
        editor: Editor,
        menu: MenuState<Item>,
        chosen: Rc<RefCell<Vec<String>>>,
        _unregister: Vec<Unregister>,
    }

    impl Fixture {
        /// A rich text editor with a `/` typeahead menu of the options
        /// `a`, `b` and `c`, which removes the query of the option chosen.
        fn new() -> Self {
            let editor = create_editor(CreateEditorArgs {
                nodes: rich_text::rich_text_nodes(),
                ..CreateEditorArgs::default()
            });
            let rich_text = rich_text::register_rich_text(&editor);
            let chosen = Rc::new(RefCell::new(Vec::new()));
            let log = chosen.clone();
            let (menu, unregister) = register_typeahead_menu::<Item>(
                &editor,
                basic_typeahead_trigger_match(
                    "/",
                    TypeaheadTriggerOptions {
                        min_length: 0,
                        allow_whitespace: true,
                        ..TypeaheadTriggerOptions::default()
                    },
                ),
                Rc::new(|_| {}),
                Rc::new(move |option, query, close, _| {
                    log.borrow_mut().push(option.key().to_owned());
                    if let Some(query) = query {
                        query.remove(false)?;
                    }
                    close();
                    Ok(())
                }),
                TypeaheadMenuOptions::default(),
            );
            menu.set_options(
                ["a", "b", "c"]
                    .into_iter()
                    .map(|key| Rc::new(Item(key.to_owned())))
                    .collect(),
            );
            Fixture {
                editor,
                menu,
                chosen,
                _unregister: vec![rich_text, unregister],
            }
        }

        /// Replaces the document with `text`, the caret at its end.
        fn type_(&self, text: &str) {
            let text = text.to_owned();
            self.editor
                .update(
                    move || {
                        let root = get_root();
                        root.clear()?;
                        let paragraph = create_paragraph_node();
                        let node = create_text_node(&text);
                        paragraph.append(std::slice::from_ref(&node))?;
                        root.append(&[paragraph])?;
                        node.select_end().map(|_| ())
                    },
                    UpdateOptions {
                        discrete: true,
                        ..UpdateOptions::default()
                    },
                )
                .unwrap();
        }

        fn text(&self) -> String {
            self.editor.read(|| get_root().get_text_content()).unwrap()
        }

        fn key(key: &str, shift_key: bool) -> Rc<KeyboardEvent> {
            Rc::new(KeyboardEvent {
                key: key.to_owned(),
                code: key.to_owned(),
                shift_key,
                ..KeyboardEvent::default()
            })
        }
    }

    #[test]
    fn arrows_wrap_and_enter_chooses() {
        let f = Fixture::new();
        f.type_("x /q");
        assert_eq!(f.menu.selected_index(), Some(0));
        let up = Fixture::key("ArrowUp", false);
        assert!(
            f.editor
                .dispatch_command(&KEY_ARROW_UP_COMMAND, up.clone())
                .unwrap()
        );
        assert!(up.default_prevented());
        assert_eq!(f.menu.selected_index(), Some(2));
        let scrolled = Rc::new(RefCell::new(Vec::new()));
        let log = scrolled.clone();
        let _scroll = f.editor.register_command(
            &SCROLL_TYPEAHEAD_OPTION_INTO_VIEW_COMMAND,
            move |payload, _| {
                log.borrow_mut()
                    .push((payload.index, payload.option.key().to_owned()));
                Ok(true)
            },
            crate::editor::COMMAND_PRIORITY_CRITICAL,
        );
        let down = || Fixture::key("ArrowDown", false);
        assert!(
            f.editor
                .dispatch_command(&KEY_ARROW_DOWN_COMMAND, down())
                .unwrap()
        );
        assert_eq!(f.menu.selected_index(), Some(0));
        f.editor
            .dispatch_command(&KEY_ARROW_DOWN_COMMAND, down())
            .unwrap();
        assert_eq!(
            *scrolled.borrow(),
            [(0, "a".to_owned()), (1, "b".to_owned())]
        );
        let enter = Fixture::key("Enter", false);
        assert!(
            f.editor
                .dispatch_command(&KEY_ENTER_COMMAND, Some(enter.clone()))
                .unwrap()
        );
        assert!(enter.default_prevented());
        assert_eq!(*f.chosen.borrow(), ["b"]);
        // The query, trigger included, was split off and removed.
        assert_eq!(f.text(), "x ");
        assert_eq!(f.menu.resolution(), None);
    }

    #[test]
    fn tab_chooses_and_shift_enter_is_left_to_rich_text() {
        let f = Fixture::new();
        f.type_("/q");
        f.editor
            .dispatch_command(&KEY_ENTER_COMMAND, Some(Fixture::key("Enter", true)))
            .unwrap();
        assert!(f.chosen.borrow().is_empty());
        f.type_("/q");
        let tab = Fixture::key("Tab", false);
        assert!(
            f.editor
                .dispatch_command(&KEY_TAB_COMMAND, tab.clone())
                .unwrap()
        );
        assert!(tab.default_prevented());
        assert_eq!(*f.chosen.borrow(), ["a"]);
    }

    #[test]
    fn escape_closes_and_keys_pass_through_when_closed_or_empty() {
        let f = Fixture::new();
        f.type_("/");
        assert!(
            f.editor
                .dispatch_command(&KEY_ESCAPE_COMMAND, Fixture::key("Escape", false))
                .unwrap()
        );
        assert_eq!(f.menu.resolution(), None);
        // Closed: the menu's commands are unregistered.
        assert!(
            !f.editor
                .dispatch_command(&KEY_TAB_COMMAND, Fixture::key("Tab", false))
                .unwrap()
        );
        f.type_("/z");
        // With no options nothing is drawn, so the keys are not the menu's.
        f.menu.set_options(Vec::new());
        let reached_editor = Rc::new(Cell::new(false));
        let reached = reached_editor.clone();
        let _fallback = f.editor.register_command(
            &KEY_ARROW_DOWN_COMMAND,
            move |_, _| {
                reached.set(true);
                Ok(false)
            },
            COMMAND_PRIORITY_EDITOR,
        );
        let down = Fixture::key("ArrowDown", false);
        assert!(
            !f.editor
                .dispatch_command(&KEY_ARROW_DOWN_COMMAND, down.clone())
                .unwrap()
        );
        assert!(reached_editor.get());
        assert!(!down.default_prevented());
        f.editor
            .dispatch_command(&KEY_ESCAPE_COMMAND, Fixture::key("Escape", false))
            .unwrap();
        assert!(f.menu.resolution().is_some());
        // A menu that draws an empty panel takes the arrows, with no option
        // to highlight.
        f.menu.set_draws_empty_menu(true);
        let down = Fixture::key("ArrowDown", false);
        assert!(
            f.editor
                .dispatch_command(&KEY_ARROW_DOWN_COMMAND, down.clone())
                .unwrap()
        );
        assert!(down.default_prevented());
        assert_eq!(f.menu.selected_index(), None);
    }

    #[test]
    fn commands_are_at_the_menu_priority() {
        let f = Fixture::new();
        assert_eq!(f.menu.command_priority(), COMMAND_PRIORITY_LOW);
        f.type_("/q");
        // A handler above the menu's priority takes the key first.
        let _above = f.editor.register_command(
            &KEY_ARROW_DOWN_COMMAND,
            |_, _| Ok(true),
            crate::editor::COMMAND_PRIORITY_NORMAL,
        );
        f.editor
            .dispatch_command(&KEY_ARROW_DOWN_COMMAND, Fixture::key("ArrowDown", false))
            .unwrap();
        assert_eq!(f.menu.selected_index(), Some(0));
    }
}
