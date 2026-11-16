//! Ports of `packages/lexical-link/src/__tests__/unit/LinkExtensionValidateUrl.test.ts`.

use crate::extension::build_editor_from_extensions;
use crate::link::{
    LinkAttributes, LinkConfigOverrides, TOGGLE_LINK_COMMAND, ToggleLinkPayload, ValidateUrl,
    is_link_node, link_extension,
};
use crate::rich_text::rich_text_extension;
use crate::{
    InitialEditorStateType, LexicalExtension, UpdateOptions, config_extension,
    create_paragraph_node, create_text_node, define_extension, get_root,
};
use std::cell::Cell;
use std::rc::Rc;

const ALLOWED: &str = "https://lexical.dev/";
// Rejected by the configured policy below (https-only).
const REJECTED: &str = "http://not-allowed.example.com/";

/// `toggleLink(payload, validateUrl)`: dispatches `payload` over a
/// selection of "Hello", and reports whether the command claimed it and
/// whether a `LinkNode` was made.
fn toggle_link(payload: ToggleLinkPayload, validate_url: Option<ValidateUrl>) -> (bool, bool) {
    let overrides = match validate_url {
        None => LinkConfigOverrides::default(),
        Some(v) => LinkConfigOverrides {
            validate_url: Some(Some(v)),
            ..LinkConfigOverrides::default()
        },
    };
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(|_| {
            let paragraph = create_paragraph_node();
            paragraph.append(&[create_text_node("Hello")])?;
            get_root().append(&[paragraph])
        }))),
        dependencies: vec![
            config_extension(&link_extension(), [overrides]),
            rich_text_extension().into(),
        ],
        ..LexicalExtension::<(), (), ()>::new("[root-validate-url]", ())
    })
    .into()])
    .unwrap();
    let handled = Rc::new(Cell::new(false));
    let out = handled.clone();
    let inner = editor.clone();
    editor
        .update(
            move || {
                let text_node = get_root().get_last_descendant().unwrap();
                assert!(text_node.is_text());
                text_node.select(Some(0), Some(text_node.get_text_content_size()))?;
                out.set(inner.dispatch_command(&TOGGLE_LINK_COMMAND, Some(payload))?);
                Ok(())
            },
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
    let linked = editor
        .read(|| {
            let paragraph = get_root().get_first_child().unwrap();
            assert_eq!(paragraph.get_type(), "paragraph");
            paragraph
                .get_first_child()
                .is_some_and(|c| is_link_node(&c))
        })
        .unwrap();
    (handled.get(), linked)
}

fn is_https() -> Option<ValidateUrl> {
    Some(Rc::new(|url: &str| url.starts_with("https://")))
}

fn object(url: &str) -> ToggleLinkPayload {
    ToggleLinkPayload::Attributes {
        url: url.to_owned(),
        attributes: LinkAttributes::default(),
    }
}

fn string(url: &str) -> ToggleLinkPayload {
    ToggleLinkPayload::Url(url.to_owned())
}

// upstream: lexical-link/src/__tests__/unit/LinkExtensionValidateUrl.test.ts::LinkExtension validateUrl > rejects a URL the validator refuses, for an object payload
#[test]
fn rejects_a_url_the_validator_refuses_for_an_object_payload() {
    assert_eq!(toggle_link(object(REJECTED), is_https()), (false, false));
}

// upstream: lexical-link/src/__tests__/unit/LinkExtensionValidateUrl.test.ts::LinkExtension validateUrl > rejects a URL the validator refuses, for a string payload
#[test]
fn rejects_a_url_the_validator_refuses_for_a_string_payload() {
    assert_eq!(toggle_link(string(REJECTED), is_https()), (false, false));
}

// upstream: lexical-link/src/__tests__/unit/LinkExtensionValidateUrl.test.ts::LinkExtension validateUrl > accepts a URL the validator allows, for an object payload
#[test]
fn accepts_a_url_the_validator_allows_for_an_object_payload() {
    assert_eq!(toggle_link(object(ALLOWED), is_https()), (true, true));
}

// upstream: lexical-link/src/__tests__/unit/LinkExtensionValidateUrl.test.ts::LinkExtension validateUrl > accepts any URL when no validator is configured
#[test]
fn accepts_any_url_when_no_validator_is_configured() {
    assert_eq!(toggle_link(object(REJECTED), None), (true, true));
    assert_eq!(toggle_link(string(REJECTED), None), (true, true));
}
