//! Helpers shared by the ports of `packages/lexical-text`'s unit tests.

#![allow(dead_code)]

use crate::markdown::RegExp;
use crate::text::EntityMatch;
use crate::{Editor, LexicalError, UpdateOptions};

/// `{discrete: true}`.
pub(crate) fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

/// `editor.update(fn, {discrete: true})`, on any editor.
pub(crate) fn update_discrete(
    editor: &Editor,
    f: impl FnOnce() -> Result<(), LexicalError> + 'static,
) {
    editor.update(f, discrete()).unwrap();
}

/// A `getMatch` that runs the JS regular expression `/source/flags` and
/// returns its first match: `{start: m.index, end: m.index + m[0].length}`.
pub(crate) fn regex_match(
    source: &str,
    flags: &str,
) -> impl Fn(&str) -> Option<EntityMatch> + 'static {
    let regex = RegExp::new(source, flags).unwrap();
    move |text: &str| {
        regex.exec(text).map(|m| EntityMatch {
            start: m.index,
            end: m.index + crate::utf16::len(m.text()),
        })
    }
}

/// A `TextNode` subclass that is a text entity (`isTextEntity(): true`),
/// with `canInsertTextBefore` and `canInsertTextAfter` as given, and
/// `new_node(text)` for `new Klass(text)`.
macro_rules! entity_class {
    ($name:ident, $type:literal) => {
        entity_class!($name, $type, true, true);
    };
    ($name:ident, $type:literal, $before:expr, $after:expr) => {
        #[derive(Clone, Debug, Default)]
        pub(crate) struct $name {
            base: crate::NodeBase,
            text: crate::nodes::text::TextData,
        }
        impl crate::LexicalNode for $name {
            fn get_type(&self) -> &'static str {
                $type
            }
            fn base(&self) -> &crate::NodeBase {
                &self.base
            }
            fn base_mut(&mut self) -> &mut crate::NodeBase {
                &mut self.base
            }
            fn text(&self) -> Option<&crate::nodes::text::TextData> {
                Some(&self.text)
            }
            fn text_mut(&mut self) -> Option<&mut crate::nodes::text::TextData> {
                Some(&mut self.text)
            }
            fn is_text_entity(&self) -> bool {
                true
            }
            fn can_insert_text_before(&self) -> bool {
                $before
            }
            fn can_insert_text_after(&self) -> bool {
                $after
            }
        }
        impl $name {
            /// `new Klass(text)`.
            #[allow(dead_code)]
            pub(crate) fn new_node(text: &str) -> crate::NodeRef {
                crate::lexical_utils::set_node_key(Box::new(Self {
                    text: crate::nodes::text::TextData {
                        text: text.to_owned(),
                        ..crate::nodes::text::TextData::default()
                    },
                    ..Self::default()
                }))
            }
        }
    };
}
pub(crate) use entity_class;
