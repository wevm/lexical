//! Ports of `packages/lexical-link/src/__tests__/unit/ClickableLinkExtension.test.ts`.
//!
//! Upstream spies on `window.open` and dispatches `click` and `auxclick` at
//! the mounted `<a>`, whose listener on the root element runs the handler.
//! A native view reports a click as `CLICK_COMMAND` with the node under the
//! pointer (rust/PARITY.md, `ClickableLinkExtension`), so these dispatch
//! that, with the link's text as the target, and record what the
//! configured `LinkWindow` opens, as Swift's port does.

use crate::extension::build_editor_from_extensions;
use crate::link::{
    ClickableLinkConfigOverrides, GetLinkWindow, LinkAttributes, LinkWindow,
    clickable_link_extension, create_link_node,
};
use crate::rich_text::rich_text_extension;
use crate::upstream_tests::support::set_test_root_element;
use crate::{
    CLICK_COMMAND, Editor, InitialEditorStateType, LexicalExtension, MouseEvent, UpdateOptions,
    config_extension, create_paragraph_node, create_text_node, define_extension, get_root,
};
use std::cell::RefCell;
use std::rc::Rc;

const URL: &str = "https://lexical.dev/";

/// `window.open`'s spy: the arguments of each call.
#[derive(Default)]
struct RecordingWindow {
    calls: RefCell<Vec<Vec<String>>>,
}

impl LinkWindow for RecordingWindow {
    fn open(&self, url: &str, target: &str, features: Option<&str>) {
        let mut call = vec![url.to_owned(), target.to_owned()];
        call.extend(features.map(str::to_owned));
        self.calls.borrow_mut().push(call);
    }
}

/// `withEditor(fn, rootExtension)`: `extension`, or `newTabExtension` when
/// `new_tab`, mounted; `fn` gets the window and a dispatcher of a click of
/// a type and button on the link.
fn with_editor(
    new_tab: bool,
    f: impl FnOnce(&RecordingWindow, &dyn Fn(&str, i32) -> Rc<MouseEvent>),
) {
    let window = Rc::new(RecordingWindow::default());
    let w = window.clone();
    let get_window = GetLinkWindow(Rc::new(move |_: &Editor| {
        Some(w.clone() as Rc<dyn LinkWindow>)
    }));
    let mut dependencies = vec![
        config_extension(
            &clickable_link_extension(),
            [ClickableLinkConfigOverrides {
                get_window: Some(get_window),
                ..ClickableLinkConfigOverrides::default()
            }],
        ),
        rich_text_extension().into(),
    ];
    if new_tab {
        dependencies.push(config_extension(
            &clickable_link_extension(),
            [ClickableLinkConfigOverrides {
                new_tab: Some(true),
                ..ClickableLinkConfigOverrides::default()
            }],
        ));
    }
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(|_| {
            let paragraph = create_paragraph_node();
            let link = create_link_node(URL, &LinkAttributes::default());
            link.append(&[create_text_node("Hello")])?;
            paragraph.append(&[link])?;
            get_root().append(&[paragraph])
        }))),
        dependencies,
        ..LexicalExtension::<(), (), ()>::new("[root]", ())
    })
    .into()])
    .unwrap();
    set_test_root_element(&editor);
    editor
        .update(
            || Ok(()),
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
    let text_key = editor
        .read(|| {
            get_root()
                .get_last_descendant()
                .unwrap()
                .get_key()
                .to_owned()
        })
        .unwrap();
    let dispatch = |type_: &str, button: i32| {
        let event = Rc::new(MouseEvent {
            button,
            target_key: Some(text_key.clone()),
            ..MouseEvent::new(type_)
        });
        editor
            .dispatch_command(&CLICK_COMMAND, event.clone())
            .unwrap();
        event
    };
    f(&window, &dispatch);
    editor.set_root_element(None).unwrap();
}

fn calls(window: &RecordingWindow) -> Vec<Vec<String>> {
    window.calls.borrow().clone()
}

// upstream: lexical-link/src/__tests__/unit/ClickableLinkExtension.test.ts::ClickableLinkExtension > a left click opens the link in the same tab
#[test]
fn a_left_click_opens_the_link_in_the_same_tab() {
    with_editor(false, |window, dispatch| {
        dispatch("click", 0);
        assert_eq!(calls(window), [[URL, "_self"]]);
    });
}

// upstream: lexical-link/src/__tests__/unit/ClickableLinkExtension.test.ts::ClickableLinkExtension > a middle click opens the link in a new tab
#[test]
fn a_middle_click_opens_the_link_in_a_new_tab() {
    with_editor(false, |window, dispatch| {
        dispatch("auxclick", 1);
        assert_eq!(calls(window), [[URL, "_blank", "noopener"]]);
    });
}

// upstream: lexical-link/src/__tests__/unit/ClickableLinkExtension.test.ts::ClickableLinkExtension > with newTab, a left click opens the link in a new tab without an opener
#[test]
fn with_new_tab_a_left_click_opens_the_link_in_a_new_tab_without_an_opener() {
    with_editor(true, |window, dispatch| {
        dispatch("click", 0);
        assert_eq!(calls(window), [[URL, "_blank", "noopener"]]);
    });
}

// upstream: lexical-link/src/__tests__/unit/ClickableLinkExtension.test.ts::ClickableLinkExtension > a middle click cancels the browser opening the link itself
#[test]
fn a_middle_click_cancels_the_browser_opening_the_link_itself() {
    with_editor(false, |_, dispatch| {
        assert!(dispatch("auxclick", 1).event.default_prevented());
    });
}

// upstream: lexical-link/src/__tests__/unit/ClickableLinkExtension.test.ts::ClickableLinkExtension > a right click does not open the link
// upstream: lexical-link/src/__tests__/unit/ClickableLinkExtension.test.ts::ClickableLinkExtension > a back click does not open the link
// upstream: lexical-link/src/__tests__/unit/ClickableLinkExtension.test.ts::ClickableLinkExtension > a forward click does not open the link
#[test]
fn a_right_back_or_forward_click_does_not_open_the_link() {
    for button in [2, 3, 4] {
        with_editor(false, |window, dispatch| {
            let event = dispatch("auxclick", button);
            assert!(calls(window).is_empty(), "{button}");
            assert!(!event.event.default_prevented(), "{button}");
        });
    }
}
