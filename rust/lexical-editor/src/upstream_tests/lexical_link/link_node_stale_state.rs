//! Ports of `packages/lexical-link/src/__tests__/unit/LinkNodeStaleState.test.ts`.

use crate::link::{AutoLinkAttributes, LinkAttributes, create_auto_link_node, create_link_node};
use crate::upstream_tests::support::{initialize_unit_test, update};
use crate::{CreateEditorArgs, NodeRef, create_paragraph_node, create_text_node, get_root};
use std::cell::RefCell;
use std::rc::Rc;

fn link(url: &str, text: &str) -> NodeRef {
    let link = create_link_node(url, &LinkAttributes::default());
    link.append(&[create_text_node(text)]).unwrap();
    link
}

fn append_paragraph(children: &[NodeRef]) -> Result<(), crate::LexicalError> {
    let paragraph = create_paragraph_node();
    paragraph.append(children)?;
    get_root().append(&[paragraph])
}

// upstream: lexical-link/src/__tests__/unit/LinkNodeStaleState.test.ts::LinkNode stale state readers > isEmailURI and isWebSiteURI agree with getURL after setURL
#[test]
fn is_email_uri_and_is_web_site_uri_agree_with_get_url_after_set_url() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let slot = Rc::new(RefCell::new(None));
    let out = slot.clone();
    update(&editor, move || {
        let l = link("https://lexical.dev/", "Hello");
        append_paragraph(std::slice::from_ref(&l))?;
        *out.borrow_mut() = Some(l);
        Ok(())
    });
    let link = slot.borrow().clone().unwrap();
    update(&editor, move || {
        // set_url goes through the writable node, which in a later update
        // is a clone; `link` is a key, so every reader reads the latest.
        link.set_url("mailto:someone@example.com");
        assert_eq!(
            link.get_url().as_deref(),
            Some("mailto:someone@example.com")
        );
        assert!(link.is_email_uri());
        assert!(!link.is_web_site_uri());
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LinkNodeStaleState.test.ts::LinkNode stale state readers > shouldMergeAdjacentLink compares the latest urls
#[test]
fn should_merge_adjacent_link_compares_the_latest_urls() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let slot = Rc::new(RefCell::new(None));
    let out = slot.clone();
    update(&editor, move || {
        let first = link("https://lexical.dev/", "a");
        let second = link("https://lexical.dev/", "b");
        append_paragraph(&[first.clone(), second.clone()])?;
        *out.borrow_mut() = Some((first, second));
        Ok(())
    });
    let (first, second) = slot.borrow().clone().unwrap();
    update(&editor, move || {
        first.set_url("https://example.com/");
        assert_ne!(first.get_url(), second.get_url());
        assert!(!first.should_merge_adjacent_link(&second));
        Ok(())
    });
}

// upstream: lexical-link/src/__tests__/unit/LinkNodeStaleState.test.ts::LinkNode stale state readers > getIsUnlinked reflects the latest value
#[test]
fn get_is_unlinked_reflects_the_latest_value() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let slot = Rc::new(RefCell::new(None));
    let out = slot.clone();
    update(&editor, move || {
        let auto_link =
            create_auto_link_node("https://lexical.dev/", &AutoLinkAttributes::default());
        auto_link.append(&[create_text_node("Hello")])?;
        append_paragraph(std::slice::from_ref(&auto_link))?;
        *out.borrow_mut() = Some(auto_link);
        Ok(())
    });
    let auto_link = slot.borrow().clone().unwrap();
    update(&editor, move || {
        auto_link.set_is_unlinked(true);
        assert_eq!(auto_link.get_is_unlinked(), Some(true));
        Ok(())
    });
}
