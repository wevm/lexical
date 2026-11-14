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
//! Not ported: the menu's keyboard commands (`KEY_ARROW_DOWN_COMMAND`...)
//! and `SCROLL_TYPEAHEAD_OPTION_INTO_VIEW_COMMAND`, which wait for an input
//! layer; an `onClose` that returns a promise; and the React rendering, the
//! anchor element and its scroll handling (rust/PARITY.md).

mod lexical_typeahead_menu_plugin_utils;

pub use lexical_typeahead_menu_plugin_utils::{
    PUNCTUATION, TypeaheadTriggerOptions, basic_typeahead_trigger_match,
};

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
    selected_index: Option<usize>,
    /// Whether the menu is open as far as `onOpen` and `onClose` are
    /// concerned (`isOpenRef`).
    is_open: bool,
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
    preselect_first_item: bool,
    should_split_node_with_query: bool,
    on_select_option: MenuSelectOption<O>,
    callbacks: Rc<MenuCallbacks>,
}

impl<O> Clone for MenuState<O> {
    fn clone(&self) -> Self {
        MenuState {
            editor: self.editor.clone(),
            inner: self.inner.clone(),
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
            })),
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
    /// (`Math.min(options.length - 1, selectedIndex)`).
    pub fn selected_index(&self) -> Option<usize> {
        let inner = self.inner.borrow();
        let index = inner.selected_index?;
        inner
            .options
            .len()
            .checked_sub(1)
            .map(|last| index.min(last))
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
        self.inner.borrow_mut().selected_index = index;
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
        {
            let mut inner = self.inner.borrow_mut();
            inner.resolution = None;
            inner.selected_index = None;
        }
        self.changed();
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
    /// `preselectFirstItem` (true).
    pub preselect_first_item: bool,
    /// `ignoreEntityBoundary` (false): a match right after a text entity
    /// opens the menu too.
    pub ignore_entity_boundary: bool,
}

impl Default for TypeaheadMenuOptions {
    fn default() -> Self {
        TypeaheadMenuOptions {
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
    let menu = MenuState::new(editor, options.preselect_first_item, true, on_select_option);
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
    /// option, with no query.
    pub fn new(editor: &Editor, on_select_option: MenuSelectOption<O>) -> Self {
        NodeMenu {
            menu: MenuState::new(editor, true, false, on_select_option),
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
