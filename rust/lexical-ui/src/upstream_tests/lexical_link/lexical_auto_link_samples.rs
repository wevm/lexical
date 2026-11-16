//! Ports of `packages/lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts`:
//! the samples and the `describe.each([false, true])` tables, with their
//! markers generated from upstream, as three table-driven tests per
//! `incremental`. Vitest cuts a name past 40 characters to 39 and `…`,
//! and the markers carry the ids as it reports them.

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::link::{
    AutoLinkConfigOverrides, auto_link_email_matcher, auto_link_extension, auto_link_url_matcher,
    is_auto_link_node,
};
use crate::{
    InitialEditorStateType, LexicalExtension, UpdateOptions, config_extension,
    create_paragraph_node, define_extension, get_root, get_selection,
};
use std::rc::Rc;

/// `links`: `(prefix, text, url, suffix)`.
const LINKS: [(&str, &str, &str, &str); 43] = [
    ("", "http://example.com", "http://example.com", " ltr"),
    ("", "https://example.com", "https://example.com", " ltr"),
    (
        "",
        "http://www.example.com",
        "http://www.example.com",
        " ltr",
    ),
    (
        "",
        "https://www.example.com",
        "https://www.example.com",
        " ltr",
    ),
    ("", "www.example.com", "https://www.example.com", " ltr"),
    ("", "http://example.org", "http://example.org", " ltr"),
    ("", "https://example.net", "https://example.net", " ltr"),
    ("", "http://example.co.uk", "http://example.co.uk", " ltr"),
    ("", "https://example.xyz", "https://example.xyz", " ltr"),
    (
        "",
        "http://example.com/path/to/resource",
        "http://example.com/path/to/resource",
        " ltr",
    ),
    (
        "",
        "https://www.example.com/path/to/resource",
        "https://www.example.com/path/to/resource",
        " ltr",
    ),
    (
        "",
        "http://example.com/path?name=value",
        "http://example.com/path?name=value",
        " ltr",
    ),
    (
        "",
        "https://www.example.com/path?name=value&another=value2",
        "https://www.example.com/path?name=value&another=value2",
        " ltr",
    ),
    (
        "",
        "http://example.com/path#section",
        "http://example.com/path#section",
        " ltr",
    ),
    (
        "",
        "https://www.example.com/path/to/resource#fragment",
        "https://www.example.com/path/to/resource#fragment",
        " ltr",
    ),
    (
        "",
        "http://example.com:8080",
        "http://example.com:8080",
        " ltr",
    ),
    (
        "",
        "https://www.example.com:443/path",
        "https://www.example.com:443/path",
        " ltr",
    ),
    ("", "http://192.168.0.1", "http://192.168.0.1", " ltr"),
    ("", "https://127.0.0.1", "https://127.0.0.1", " ltr"),
    (
        "",
        "http://example.com/path/to/res+ource",
        "http://example.com/path/to/res+ource",
        " ltr",
    ),
    (
        "",
        "https://example.com/path/to/res%20ource",
        "https://example.com/path/to/res%20ource",
        " ltr",
    ),
    (
        "",
        "http://example.com/path?name=va@lue",
        "http://example.com/path?name=va@lue",
        " ltr",
    ),
    (
        "",
        "https://example.com/path?name=value&another=val%20ue",
        "https://example.com/path?name=value&another=val%20ue",
        " ltr",
    ),
    (
        "",
        "http://subdomain.example.com",
        "http://subdomain.example.com",
        " ltr",
    ),
    (
        "",
        "https://sub.subdomain.example.com",
        "https://sub.subdomain.example.com",
        " ltr",
    ),
    ("", "http://example.museum", "http://example.museum", " ltr"),
    (
        "",
        "https://example.travel",
        "https://example.travel",
        " ltr",
    ),
    ("", "http://foo.bar", "http://foo.bar", " ltr"),
    ("", "https://foo.bar", "https://foo.bar", " ltr"),
    ("", "email@domain.com", "mailto:email@domain.com", " ltr"),
    (
        "",
        "firstname.lastname@domain.com",
        "mailto:firstname.lastname@domain.com",
        " ltr",
    ),
    (
        "",
        "email@subdomain.domain.com",
        "mailto:email@subdomain.domain.com",
        " ltr",
    ),
    (
        "",
        "firstname+lastname@domain.com",
        "mailto:firstname+lastname@domain.com",
        " ltr",
    ),
    (
        "",
        "email@[123.123.123.123]",
        "mailto:email@[123.123.123.123]",
        " ltr",
    ),
    (
        "",
        "\"email\"@domain.com",
        "mailto:\"email\"@domain.com",
        " ltr",
    ),
    (
        "",
        "1234567890@domain.com",
        "mailto:1234567890@domain.com",
        " ltr",
    ),
    (
        "",
        "email@domain-one.com",
        "mailto:email@domain-one.com",
        " ltr",
    ),
    (
        "",
        "_______@domain.com",
        "mailto:_______@domain.com",
        " ltr",
    ),
    ("", "email@domain.name", "mailto:email@domain.name", " ltr"),
    (
        "",
        "email@domain.co.uk",
        "mailto:email@domain.co.uk",
        " ltr",
    ),
    (
        "",
        "firstname-lastname@domain.com",
        "mailto:firstname-lastname@domain.com",
        " ltr",
    ),
    (
        "مرحبا ",
        "https://qabilah.com/posts/عربي",
        "https://qabilah.com/posts/عربي",
        " end",
    ),
    (
        "go ",
        "http://예시.한국/경로?키=값#부분",
        "http://예시.한국/경로?키=값#부분",
        " done",
    ),
];

const INVALID_URLS: [&str; 14] = [
    "example.com",
    "htp://example.com",
    "htps://example.com",
    "http://exa mple.com",
    "https://example .com",
    "http://example!.com",
    "http://.com",
    "https://.org",
    "http://",
    "https://",
    "not_a_url",
    "this is not a url",
    "example",
    "ftp://example.com",
];

const INVALID_EMAILS: [&str; 8] = [
    "@domain.com",
    "@subdomain.domain.com",
    "email@domain!.com",
    "email@domain.c",
    "email@.com",
    "email@.org",
    "email@",
    "not_an_email",
];

fn build_editor() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(|_| {
            get_root().append(&[create_paragraph_node()])?;
            get_root().select_end().map(|_| ())
        }))),
        dependencies: vec![config_extension(
            &auto_link_extension(),
            [AutoLinkConfigOverrides {
                matchers: Some(vec![
                    Rc::new(auto_link_url_matcher),
                    Rc::new(auto_link_email_matcher),
                ]),
                ..AutoLinkConfigOverrides::default()
            }],
        )],
        ..LexicalExtension::<(), (), ()>::new("[test autolink samples]", ())
    })
    .into()])
    .unwrap()
}

/// `insertText(editor, text, incremental)`: one discrete update per
/// character when `incremental`, so matching runs at every boundary.
fn insert_text(editor: &LexicalEditorWithDispose, text: &str, incremental: bool) {
    let chunks: Vec<String> = if incremental {
        text.chars().map(String::from).collect()
    } else {
        vec![text.to_owned()]
    };
    for chunk in chunks {
        editor
            .update(
                move || {
                    let selection = get_selection().unwrap();
                    assert!(selection.is_range());
                    selection.insert_text(&chunk)
                },
                UpdateOptions {
                    discrete: true,
                    ..UpdateOptions::default()
                },
            )
            .unwrap();
    }
}

fn check_link(incremental: bool, prefix: &str, text: &str, url: &str, suffix: &str) {
    let editor = build_editor();
    let input = format!("{prefix}{text}{suffix}");
    insert_text(&editor, &input, incremental);
    editor
        .read(|| {
            let paragraph = get_root().get_first_child_or_throw().unwrap();
            assert_eq!(paragraph.get_type(), "paragraph");
            assert_eq!(get_root().get_children_size(), 1);
            assert_eq!(paragraph.get_text_content(), input);
            let has_prefix = !prefix.is_empty();
            assert_eq!(
                paragraph.get_children_size(),
                if has_prefix { 3 } else { 2 },
                "{input}"
            );
            let link = paragraph
                .get_child_at_index(usize::from(has_prefix))
                .unwrap();
            assert!(is_auto_link_node(&link), "{input}");
            assert_eq!(link.get_url().as_deref(), Some(url));
            assert_eq!(link.get_is_unlinked(), Some(false));
            assert_eq!(link.get_children_size(), 1);
            assert_eq!(
                link.get_first_child_or_throw().unwrap().get_text_content(),
                text
            );
            if has_prefix {
                let before = paragraph.get_first_child_or_throw().unwrap();
                assert!(before.is_text());
                assert_eq!(before.get_text_content(), prefix);
            }
            let after = paragraph.get_last_child_or_throw().unwrap();
            assert!(after.is_text());
            assert_eq!(after.get_text_content(), suffix);
        })
        .unwrap();
}

fn leaves_unchanged(incremental: bool, samples: &[&str]) {
    let editor = build_editor();
    let text = samples.join(" ");
    insert_text(&editor, &text, incremental);
    editor
        .read(|| {
            let paragraph = get_root().get_first_child_or_throw().unwrap();
            assert_eq!(paragraph.get_type(), "paragraph");
            assert_eq!(get_root().get_children_size(), 1);
            assert_eq!(paragraph.get_children_size(), 1);
            let child = paragraph.get_first_child_or_throw().unwrap();
            assert!(child.is_text());
            assert_eq!(child.get_text_content(), text);
        })
        .unwrap();
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://example.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://example.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://www.example.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://www.example.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > www.example.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://example.org
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://example.net
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://example.co.uk
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://example.xyz
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://example.com/path/to/resource
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://www.example.com/path/to/resource
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://example.com/path?name=value
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://www.example.com/path?name=value…
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://example.com/path#section
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://www.example.com/path/to/resourc…
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://example.com:8080
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://www.example.com:443/path
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://192.168.0.1
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://127.0.0.1
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://example.com/path/to/res+ource
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://example.com/path/to/res%20ource
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://example.com/path?name=va@lue
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://example.com/path?name=value&ano…
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://subdomain.example.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://sub.subdomain.example.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://example.museum
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://example.travel
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://foo.bar
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://foo.bar
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > email@domain.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > firstname.lastname@domain.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > email@subdomain.domain.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > firstname+lastname@domain.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > email@[123.123.123.123]
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > "email"@domain.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > 1234567890@domain.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > email@domain-one.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > _______@domain.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > email@domain.name
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > email@domain.co.uk
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > firstname-lastname@domain.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > https://qabilah.com/posts/عربي
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > http://예시.한국/경로?키=값#부분
#[test]
fn autolink_transform_links_incremental_false() {
    for (prefix, text, url, suffix) in LINKS {
        check_link(false, prefix, text, url, suffix);
    }
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > leaves invalid URLs unchanged
#[test]
fn leaves_invalid_urls_unchanged_incremental_false() {
    leaves_unchanged(false, &INVALID_URLS);
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: false) > leaves invalid emails unchanged
#[test]
fn leaves_invalid_emails_unchanged_incremental_false() {
    leaves_unchanged(false, &INVALID_EMAILS);
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://example.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://example.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://www.example.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://www.example.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > www.example.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://example.org
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://example.net
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://example.co.uk
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://example.xyz
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://example.com/path/to/resource
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://www.example.com/path/to/resource
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://example.com/path?name=value
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://www.example.com/path?name=value…
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://example.com/path#section
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://www.example.com/path/to/resourc…
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://example.com:8080
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://www.example.com:443/path
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://192.168.0.1
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://127.0.0.1
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://example.com/path/to/res+ource
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://example.com/path/to/res%20ource
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://example.com/path?name=va@lue
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://example.com/path?name=value&ano…
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://subdomain.example.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://sub.subdomain.example.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://example.museum
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://example.travel
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://foo.bar
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://foo.bar
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > email@domain.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > firstname.lastname@domain.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > email@subdomain.domain.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > firstname+lastname@domain.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > email@[123.123.123.123]
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > "email"@domain.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > 1234567890@domain.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > email@domain-one.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > _______@domain.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > email@domain.name
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > email@domain.co.uk
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > firstname-lastname@domain.com
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > https://qabilah.com/posts/عربي
// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > http://예시.한국/경로?키=값#부분
#[test]
fn autolink_transform_links_incremental_true() {
    for (prefix, text, url, suffix) in LINKS {
        check_link(true, prefix, text, url, suffix);
    }
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > leaves invalid URLs unchanged
#[test]
fn leaves_invalid_urls_unchanged_incremental_true() {
    leaves_unchanged(true, &INVALID_URLS);
}

// upstream: lexical-link/src/__tests__/unit/LexicalAutoLinkSamples.test.ts::AutoLink transform (incremental: true) > leaves invalid emails unchanged
#[test]
fn leaves_invalid_emails_unchanged_incremental_true() {
    leaves_unchanged(true, &INVALID_EMAILS);
}
