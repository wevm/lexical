//! Ports of `packages/lexical-react/src/__tests__/unit/LexicalMenu.test.tsx`,
//! `LexicalMenuEmptyOptions.test.tsx`, `LexicalNodeMenuPlugin.test.tsx`,
//! `LexicalTypeaheadMenuPlugin.test.tsx` and
//! `LexicalTypeaheadMenuPluginUtils.test.ts` against the `menu` feature.
//!
//! The React tests render `LexicalMenu` and read the DOM it draws, or what
//! a `menuRenderFn` is given (`options`, `selectedIndex`,
//! `matchingString`). Without React the app's view draws the menu from
//! [`MenuState`], so these assert the state that view reads (a drawn menu
//! is an open one, `resolution()` set), and the commands the menu handles.
//! An `onClose` that returns a promise is `on_close_async`: finishing the
//! [`MenuCloseDone`] resolves it, dropping it rejects it, and a test's
//! `settle` stands for the microtasks `act` flushes. The jsdom
//! `Selection.modify` patch the onClose tests install is
//! [`CharacterMovement`].

#![cfg(all(feature = "menu", feature = "rich-text"))]

use crate::menu::{
    MenuCloseDone, MenuOption, MenuSelectOption, MenuState, NodeMenu,
    SCROLL_TYPEAHEAD_OPTION_INTO_VIEW_COMMAND, TypeaheadMenuOptions, TypeaheadTriggerOptions,
    basic_typeahead_trigger_match, register_typeahead_menu,
};
use crate::{
    COMMAND_PRIORITY_CRITICAL, COMMAND_PRIORITY_EDITOR, COMMAND_PRIORITY_LOW, CreateEditorArgs,
    DELETE_CHARACTER_COMMAND, Editor, KEY_ARROW_DOWN_COMMAND, KEY_ARROW_UP_COMMAND,
    KEY_ENTER_COMMAND, KEY_ESCAPE_COMMAND, KeyboardEvent, LexicalCommand, LexicalError, Point,
    PointType, SelectionGranularity, SelectionMovement, Unregister, UpdateOptions, create_editor,
    create_paragraph_node, get_root, get_selection, rich_text,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

struct Item(String);

impl MenuOption for Item {
    fn key(&self) -> &str {
        &self.0
    }
    fn title(&self) -> Option<&str> {
        Some(&self.0)
    }
}

fn items(keys: &[&str]) -> Vec<Rc<Item>> {
    keys.iter()
        .map(|k| Rc::new(Item((*k).to_owned())))
        .collect()
}

/// Moves a text point one UTF-16 unit within its text node: what the
/// onClose tests' `Selection.prototype.modify` patch does.
struct CharacterMovement;

impl SelectionMovement for CharacterMovement {
    fn landed_point(
        &self,
        from: &Point,
        is_backward: bool,
        _granularity: SelectionGranularity,
    ) -> Result<Option<Point>, LexicalError> {
        if from.point_type != PointType::Text {
            return Ok(None);
        }
        let text = from.get_node()?;
        let offset = from.offset as i64 + if is_backward { -1 } else { 1 };
        if offset < 0 || offset as usize > text.get_text_content_size() {
            return Ok(None);
        }
        Ok(Some(Point::new(
            text.get_key(),
            offset as usize,
            PointType::Text,
        )))
    }
}

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

#[derive(Default)]
struct Log {
    chosen: RefCell<Vec<String>>,
    queries: RefCell<Vec<Option<String>>>,
    opened: Cell<usize>,
    closed: Cell<usize>,
    /// The completions of the closes `on_close_async` was given.
    pending: RefCell<Vec<MenuCloseDone>>,
    /// How the close promise settles once settled: `Some(true)` resolves,
    /// `Some(false)` rejects.
    settlement: Cell<Option<bool>>,
}

struct Fixture {
    editor: Editor,
    menu: MenuState<Item>,
    log: Rc<Log>,
    unregister: Vec<Unregister>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for unregister in self.unregister.drain(..) {
            unregister();
        }
    }
}

impl Fixture {
    /// A `/` typeahead menu (`minLength: 0`) over `options`, in a rich text
    /// editor when `rich_text`.
    fn new(options: &[&str], with_rich_text: bool) -> Self {
        let editor = create_editor(CreateEditorArgs {
            nodes: rich_text::rich_text_nodes(),
            ..CreateEditorArgs::default()
        });
        editor.set_selection_movement(Some(Rc::new(CharacterMovement)));
        let mut unregister = Vec::new();
        if with_rich_text {
            unregister.push(rich_text::register_rich_text(&editor));
        }
        let log = Rc::new(Log::default());
        let (chosen, queries) = (log.clone(), log.clone());
        let on_select_option: MenuSelectOption<Item> = Rc::new(move |option, query, close, _| {
            chosen.chosen.borrow_mut().push(option.key().to_owned());
            if let Some(query) = query {
                query.remove(false)?;
            }
            close();
            Ok(())
        });
        let (menu, unregister_menu) = register_typeahead_menu::<Item>(
            &editor,
            basic_typeahead_trigger_match(
                "/",
                TypeaheadTriggerOptions {
                    min_length: 0,
                    ..TypeaheadTriggerOptions::default()
                },
            ),
            Rc::new(move |q| queries.queries.borrow_mut().push(q.map(str::to_owned))),
            on_select_option,
            TypeaheadMenuOptions::default(),
        );
        unregister.push(unregister_menu);
        let opened = log.clone();
        *menu.callbacks().on_open.borrow_mut() =
            Some(Rc::new(move |_| opened.opened.set(opened.opened.get() + 1)));
        let closed = log.clone();
        *menu.callbacks().on_close.borrow_mut() =
            Some(Rc::new(move || closed.closed.set(closed.closed.get() + 1)));
        menu.set_options(items(options));
        Fixture {
            editor,
            menu,
            log,
            unregister,
        }
    }

    fn default() -> Self {
        Self::new(&["a", "b", "c"], true)
    }

    /// Makes `onClose` return a promise, settled by [`Self::settle_close`].
    fn with_async_close(self) -> Self {
        let log = self.log.clone();
        *self.menu.callbacks().on_close_async.borrow_mut() = Some(Rc::new(move |done| {
            log.pending.borrow_mut().push(done);
        }));
        self
    }

    /// Settles the close promise (`true` resolves it), and every close
    /// waiting for it.
    fn settle_close(&self, resolve: bool) {
        self.log.settlement.set(Some(resolve));
        self.flush();
    }

    /// The microtasks `act` flushes: the closes of a settled promise end.
    fn flush(&self) {
        let Some(resolve) = self.log.settlement.get() else {
            return;
        };
        let pending: Vec<MenuCloseDone> = self.log.pending.borrow_mut().drain(..).collect();
        for done in pending {
            if resolve {
                done.finish();
            } else {
                drop(done);
            }
        }
    }

    /// `$getRoot().clear().append($createParagraphNode())` with the caret
    /// in the paragraph.
    fn empty_paragraph(&self) {
        self.editor
            .update(
                || {
                    let root = get_root();
                    root.clear()?;
                    let paragraph = create_paragraph_node();
                    root.append(std::slice::from_ref(&paragraph))?;
                    paragraph.select(None, None).map(|_| ())
                },
                discrete(),
            )
            .unwrap();
    }

    /// `selection.insertText(text)` in its own update.
    fn insert_text(&self, text: &str) {
        let text = text.to_owned();
        self.editor
            .update(
                move || get_selection().unwrap().insert_text(&text),
                discrete(),
            )
            .unwrap();
        self.flush();
    }

    /// An empty paragraph, then `text` typed into it.
    fn type_(&self, text: &str) {
        self.empty_paragraph();
        self.insert_text(text);
    }

    fn query(&self) -> Option<String> {
        self.menu
            .resolution()
            .and_then(|r| r.text_match)
            .map(|m| m.matching_string)
    }

    /// `DELETE_CHARACTER_COMMAND` backward, as Backspace dispatches it.
    fn backspace(&self) {
        let editor = self.editor.clone();
        self.editor
            .update(
                move || {
                    editor
                        .dispatch_command(&DELETE_CHARACTER_COMMAND, true)
                        .map(|_| ())
                },
                discrete(),
            )
            .unwrap();
        self.flush();
    }

    /// `pressKey(command)` of LexicalMenuEmptyOptions.test.tsx: dispatches
    /// `command` with a fallback handler below the menu's priority.
    fn press(&self, command: &LexicalCommand<Rc<KeyboardEvent>>) -> (bool, bool, bool) {
        let reached_editor = Rc::new(Cell::new(false));
        let reached = reached_editor.clone();
        let remove = self.editor.register_command(
            command,
            move |_, _| {
                reached.set(true);
                Ok(false)
            },
            COMMAND_PRIORITY_EDITOR,
        );
        let event = key("");
        let handled = self
            .editor
            .dispatch_command(command, event.clone())
            .unwrap();
        remove();
        (handled, event.default_prevented(), reached_editor.get())
    }
}

fn key(key: &str) -> Rc<KeyboardEvent> {
    Rc::new(KeyboardEvent {
        key: key.to_owned(),
        code: key.to_owned(),
        ..KeyboardEvent::default()
    })
}

// upstream: lexical-react/src/__tests__/unit/LexicalMenu.test.tsx::MenuOption > should set key from constructor
// upstream: lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPlugin.test.tsx::LexicalTypeaheadMenuPlugin > exports > should export MenuOption class
// upstream: lexical-react/src/__tests__/unit/LexicalNodeMenuPlugin.test.tsx::LexicalNodeMenuPlugin > exports > should export MenuOption class
#[test]
fn menu_option_keeps_its_key() {
    assert_eq!(Item("test-key".to_owned()).key(), "test-key");
}

// upstream: lexical-react/src/__tests__/unit/LexicalMenu.test.tsx::MenuOption > should support optional title property
#[test]
fn menu_option_has_an_optional_title() {
    struct Untitled;
    impl MenuOption for Untitled {
        fn key(&self) -> &str {
            "test-key"
        }
    }
    assert_eq!(Untitled.title(), None);
    assert_eq!(Item("Test Title".to_owned()).title(), Some("Test Title"));
}

// upstream: lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPluginUtils.test.ts::SCROLL_TYPEAHEAD_OPTION_INTO_VIEW_COMMAND > is one command, whichever module it is reached through
#[test]
fn the_scroll_command_is_the_one_the_menu_dispatches() {
    // Commands match by identity: the command an app registers for is the
    // one the menu dispatches as the highlight moves.
    let f = Fixture::default();
    let scrolled = Rc::new(RefCell::new(Vec::new()));
    let log = scrolled.clone();
    let remove = f.editor.register_command(
        &SCROLL_TYPEAHEAD_OPTION_INTO_VIEW_COMMAND,
        move |payload, _| {
            log.borrow_mut()
                .push((payload.index, payload.option.key().to_owned()));
            Ok(true)
        },
        COMMAND_PRIORITY_CRITICAL,
    );
    f.type_("/q");
    f.editor
        .dispatch_command(&KEY_ARROW_DOWN_COMMAND, key("ArrowDown"))
        .unwrap();
    assert_eq!(*scrolled.borrow(), [(1, "b".to_owned())]);
    remove();
}

// upstream: lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPluginUtils.test.ts::MenuOption > is one class, whichever module it is reached through
#[test]
fn typeahead_and_node_menus_share_menu_option() {
    // The typeahead menu and the node menu take the same `MenuOption`, so
    // an option chosen in either is the app's type.
    let f = Fixture::default();
    let chosen: Rc<RefCell<Vec<Rc<Item>>>> = Rc::default();
    let log = chosen.clone();
    let node_menu = NodeMenu::<Item>::new(
        &f.editor,
        Rc::new(move |option, _, close, _| {
            log.borrow_mut().push(option.clone());
            close();
            Ok(())
        }),
        COMMAND_PRIORITY_LOW,
    );
    let typeahead_option: Rc<Item> = f.menu.options()[0].clone();
    assert_eq!(typeahead_option.key(), "a");
    f.type_("x");
    let key_ = f
        .editor
        .read(|| get_root().get_first_child().unwrap().get_key().to_owned())
        .unwrap();
    node_menu.set_node_key(Some(key_));
    node_menu.menu().set_options(items(&["n"]));
    f.editor
        .dispatch_command(&KEY_ENTER_COMMAND, Some(key("Enter")))
        .unwrap();
    assert_eq!(chosen.borrow().len(), 1);
    assert_eq!(chosen.borrow()[0].key(), "n");
}

// upstream: lexical-react/src/__tests__/unit/LexicalMenu.test.tsx::LexicalMenu > default rendering (without menuRenderFn) > should apply selected class to preselected first item
// upstream: lexical-react/src/__tests__/unit/LexicalMenu.test.tsx::LexicalMenu > custom rendering (with menuRenderFn) > should pass selectedIndex to menuRenderFn
#[test]
fn preselects_the_first_option() {
    let f = Fixture::new(&["First", "Second"], true);
    f.type_("/test");
    assert_eq!(f.menu.selected_index(), Some(0));
}

// upstream: lexical-react/src/__tests__/unit/LexicalMenu.test.tsx::LexicalMenu > custom rendering (with menuRenderFn) > should use menuRenderFn when provided
// upstream: lexical-react/src/__tests__/unit/LexicalMenu.test.tsx::LexicalMenu > custom rendering (with menuRenderFn) > should pass options array to menuRenderFn
#[test]
fn exposes_the_options_and_the_query_to_the_view() {
    let f = Fixture::new(&["X", "Y", "Z"], true);
    f.type_("/hello");
    assert_eq!(
        f.menu
            .options()
            .iter()
            .map(|o| o.title().unwrap().to_owned())
            .collect::<Vec<_>>(),
        ["X", "Y", "Z"]
    );
    assert_eq!(f.query().as_deref(), Some("hello"));
}

// upstream: lexical-react/src/__tests__/unit/LexicalMenu.test.tsx::LexicalMenu > custom rendering (with menuRenderFn) > should pass empty string as matchingString when no match
#[test]
fn a_menu_without_a_match_has_an_empty_query() {
    let f = Fixture::default();
    let queries = Rc::new(RefCell::new(Vec::new()));
    let log = queries.clone();
    let node_menu = NodeMenu::<Item>::new(
        &f.editor,
        Rc::new(move |_, _, close, matching_string| {
            log.borrow_mut().push(matching_string.to_owned());
            close();
            Ok(())
        }),
        COMMAND_PRIORITY_LOW,
    );
    f.empty_paragraph();
    let key_ = f
        .editor
        .read(|| get_root().get_first_child().unwrap().get_key().to_owned())
        .unwrap();
    node_menu.set_node_key(Some(key_));
    node_menu.menu().set_options(items(&["A"]));
    assert_eq!(node_menu.menu().resolution().unwrap().text_match, None);
    f.editor
        .dispatch_command(&KEY_ENTER_COMMAND, Some(key("Enter")))
        .unwrap();
    assert_eq!(*queries.borrow(), [""]);
}

/// The menu open at `/zz` over `options`, without rich text, drawing an
/// empty menu when `draws_empty_menu` (a `menuRenderFn`).
fn empty_options_fixture(options: &[&str], draws_empty_menu: bool) -> Fixture {
    let f = Fixture::new(options, false);
    f.menu.set_draws_empty_menu(draws_empty_menu);
    f.type_("/zz");
    assert!(f.menu.resolution().is_some());
    f
}

// upstream: lexical-react/src/__tests__/unit/LexicalMenuEmptyOptions.test.tsx::LexicalMenu arrow keys with no options > lets ArrowUp through when there are no options
// upstream: lexical-react/src/__tests__/unit/LexicalMenuEmptyOptions.test.tsx::LexicalMenu arrow keys with no options > lets Escape through when there are no options
#[test]
fn keys_pass_through_an_empty_menu() {
    for command in [
        &KEY_ARROW_DOWN_COMMAND,
        &KEY_ARROW_UP_COMMAND,
        &KEY_ESCAPE_COMMAND,
    ] {
        // An empty option list draws no menu, so the key still reaches what
        // would otherwise move the caret.
        let f = empty_options_fixture(&[], false);
        assert_eq!(f.press(command), (false, false, true));
    }
}

// upstream: lexical-react/src/__tests__/unit/LexicalMenuEmptyOptions.test.tsx::LexicalMenu arrow keys with no options > still consumes ArrowDown when there are options
// upstream: lexical-react/src/__tests__/unit/LexicalMenuEmptyOptions.test.tsx::LexicalMenu arrow keys with no options > still consumes Escape when there are options
#[test]
fn a_menu_with_options_consumes_its_keys() {
    for command in [
        &KEY_ARROW_DOWN_COMMAND,
        &KEY_ARROW_UP_COMMAND,
        &KEY_ESCAPE_COMMAND,
    ] {
        let f = empty_options_fixture(&["a", "b"], false);
        assert_eq!(f.press(command), (true, true, false));
    }
}

// upstream: lexical-react/src/__tests__/unit/LexicalMenuEmptyOptions.test.tsx::LexicalMenu arrow keys with no options > still consumes ArrowUp when a custom renderer draws an empty menu
// upstream: lexical-react/src/__tests__/unit/LexicalMenuEmptyOptions.test.tsx::LexicalMenu arrow keys with no options > still consumes Escape when a custom renderer draws an empty menu
#[test]
fn a_view_that_draws_an_empty_menu_keeps_its_keys() {
    for command in [
        &KEY_ARROW_DOWN_COMMAND,
        &KEY_ARROW_UP_COMMAND,
        &KEY_ESCAPE_COMMAND,
    ] {
        // A view that shows a "no results" panel still has to be moved
        // through and dismissed.
        let f = empty_options_fixture(&[], true);
        assert_eq!(f.press(command), (true, true, false));
    }
}

// upstream: lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPlugin.test.tsx::LexicalTypeaheadMenuPlugin > onClose > awaits async onClose before unmounting the menu
#[test]
fn awaits_async_on_close_before_clearing_the_menu() {
    let f = Fixture::default().with_async_close();
    f.type_("/");
    assert!(f.menu.resolution().is_some());
    assert_eq!(f.log.closed.get(), 0);
    f.backspace();
    assert_eq!(f.log.closed.get(), 1);
    // Still drawn, with its keys, while onClose is pending.
    assert!(f.menu.resolution().is_some());
    assert!(f.press(&KEY_ARROW_DOWN_COMMAND).0);
    f.settle_close(true);
    assert_eq!(f.menu.resolution(), None);
    assert!(!f.press(&KEY_ARROW_DOWN_COMMAND).0);
}

/// `preserves a newer query when an earlier onClose promise %ss after %i
/// close requests`.
fn a_pending_close_keeps_a_newer_query(resolve: bool, close_requests: usize) {
    let f = Fixture::default().with_async_close();
    f.type_("/old");
    assert_eq!(f.query().as_deref(), Some("old"));
    for _ in 0..close_requests {
        f.insert_text(" ");
    }
    // A close that is already pending is not requested again.
    assert_eq!(f.log.closed.get(), 1);
    assert_eq!(f.query().as_deref(), Some("old"));
    f.insert_text("/new");
    // The newer query reopens the menu that onClose was told about.
    assert_eq!(f.log.opened.get(), 2);
    assert_eq!(f.query().as_deref(), Some("new"));
    f.settle_close(resolve);
    assert_eq!(f.query().as_deref(), Some("new"));
    f.insert_text("er");
    assert_eq!(f.query().as_deref(), Some("newer"));
    f.insert_text(" ");
    assert_eq!(f.log.closed.get(), 2);
    assert_eq!(f.log.opened.get(), 2);
    assert_eq!(f.menu.resolution(), None);
}

// upstream: lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPlugin.test.tsx::LexicalTypeaheadMenuPlugin > onClose > preserves a newer query when an earlier onClose promise resolves after 1 close requests
#[test]
fn a_pending_close_keeps_a_newer_query_resolves_after_1() {
    a_pending_close_keeps_a_newer_query(true, 1);
}

// upstream: lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPlugin.test.tsx::LexicalTypeaheadMenuPlugin > onClose > preserves a newer query when an earlier onClose promise rejects after 1 close requests
#[test]
fn a_pending_close_keeps_a_newer_query_rejects_after_1() {
    a_pending_close_keeps_a_newer_query(false, 1);
}

// upstream: lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPlugin.test.tsx::LexicalTypeaheadMenuPlugin > onClose > preserves a newer query when an earlier onClose promise resolves after 2 close requests
#[test]
fn a_pending_close_keeps_a_newer_query_resolves_after_2() {
    a_pending_close_keeps_a_newer_query(true, 2);
}

// upstream: lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPlugin.test.tsx::LexicalTypeaheadMenuPlugin > onClose > preserves a newer query when an earlier onClose promise rejects after 2 close requests
#[test]
fn a_pending_close_keeps_a_newer_query_rejects_after_2() {
    a_pending_close_keeps_a_newer_query(false, 2);
}

/// `clears an uncommitted query when onClose is %s`: `None` synchronous,
/// else a promise that is already resolved (`true`) or rejected.
fn clears_an_uncommitted_query(settlement: Option<bool>) {
    let mut f = Fixture::default();
    if let Some(resolve) = settlement {
        f = f.with_async_close();
        f.log.settlement.set(Some(resolve));
    }
    f.type_("/old");
    f.insert_text("er");
    f.insert_text(" ");
    assert_eq!(f.log.closed.get(), 1);
    assert_eq!(f.menu.resolution(), None);
}

// upstream: lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPlugin.test.tsx::LexicalTypeaheadMenuPlugin > onClose > clears an uncommitted query when onClose is synchronous
#[test]
fn clears_an_uncommitted_query_when_on_close_is_synchronous() {
    clears_an_uncommitted_query(None);
}

// upstream: lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPlugin.test.tsx::LexicalTypeaheadMenuPlugin > onClose > clears an uncommitted query when onClose is resolved
#[test]
fn clears_an_uncommitted_query_when_on_close_is_resolved() {
    clears_an_uncommitted_query(Some(true));
}

// upstream: lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPlugin.test.tsx::LexicalTypeaheadMenuPlugin > onClose > clears an uncommitted query when onClose is rejected
#[test]
fn clears_an_uncommitted_query_when_on_close_is_rejected() {
    clears_an_uncommitted_query(Some(false));
}

// upstream: lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPlugin.test.tsx::LexicalTypeaheadMenuPlugin > onClose > closes a first query that has not rendered yet
#[test]
fn closes_a_first_query_that_has_not_rendered_yet() {
    let f = Fixture::default();
    f.empty_paragraph();
    f.insert_text("/a");
    f.insert_text(" ");
    assert_eq!(f.log.opened.get(), 1);
    assert_eq!(f.log.closed.get(), 1);
    assert_eq!(f.menu.resolution(), None);
}

// upstream: lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPlugin.test.tsx::LexicalTypeaheadMenuPlugin > onClose > runs synchronous onClose before clearing the menu
#[test]
fn runs_synchronous_on_close_before_clearing_the_menu() {
    let f = Fixture::default();
    let open_when_closing = Rc::new(RefCell::new(Vec::new()));
    let (log, menu) = (open_when_closing.clone(), f.menu.clone());
    *f.menu.callbacks().on_close.borrow_mut() = Some(Rc::new(move || {
        log.borrow_mut().push(menu.resolution().is_some());
    }));
    f.type_("/");
    assert!(f.menu.resolution().is_some());
    f.backspace();
    // onClose ran once, while the menu was still there.
    assert_eq!(*open_when_closing.borrow(), [true]);
    assert_eq!(f.menu.resolution(), None);
}

/// `mountAndCompose`: the trigger typed, then `햄` composed after it.
fn composing_fixture() -> Fixture {
    let f = Fixture::default();
    f.type_("/");
    assert!(f.menu.resolution().is_some());
    f.log.queries.borrow_mut().clear();
    f.editor
        .update(
            || {
                let selection = get_selection().unwrap();
                selection.insert_text("햄")?;
                let key = selection.point(crate::Which::Anchor).unwrap().key.clone();
                crate::set_composition_key(Some(&key));
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    f
}

/// `$dropTriggerWhileComposing`: the composition replaces the query,
/// trigger and all.
fn drop_trigger_while_composing(f: &Fixture) {
    f.editor
        .update(
            || {
                let text = get_root().get_first_descendant().unwrap();
                assert!(text.is_text(), "expected a text node holding the query");
                text.set_text_content("햄");
                crate::set_composition_key(Some(text.get_key()));
                Ok(())
            },
            discrete(),
        )
        .unwrap();
}

// upstream: lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPlugin.test.tsx::LexicalTypeaheadMenuPlugin > IME composition > reports the query while composing without closing the menu
#[test]
fn reports_the_query_while_composing_without_closing_the_menu() {
    let f = composing_fixture();
    assert!(f.editor.is_composing());
    assert!(f.log.queries.borrow().contains(&Some("햄".to_owned())));
    assert_eq!(f.query().as_deref(), Some("햄"));
}

// upstream: lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPlugin.test.tsx::LexicalTypeaheadMenuPlugin > IME composition > keeps the menu open when the trigger match is lost while composing
#[test]
fn keeps_the_menu_open_when_the_trigger_match_is_lost_while_composing() {
    let f = composing_fixture();
    drop_trigger_while_composing(&f);
    assert!(f.editor.is_composing());
    assert_eq!(f.log.closed.get(), 0);
    assert!(f.menu.resolution().is_some());
}

// upstream: lexical-react/src/__tests__/unit/LexicalTypeaheadMenuPlugin.test.tsx::LexicalTypeaheadMenuPlugin > IME composition > closes the menu once composition ends without a trigger match
#[test]
fn closes_the_menu_once_composition_ends_without_a_trigger_match() {
    let f = composing_fixture();
    drop_trigger_while_composing(&f);
    assert!(f.menu.resolution().is_some());
    f.editor
        .update(
            || {
                crate::set_composition_key(None);
                get_root().get_first_descendant().unwrap().mark_dirty();
                Ok(())
            },
            discrete(),
        )
        .unwrap();
    assert!(!f.editor.is_composing());
    assert_eq!(f.log.closed.get(), 1);
    assert_eq!(f.menu.resolution(), None);
}
