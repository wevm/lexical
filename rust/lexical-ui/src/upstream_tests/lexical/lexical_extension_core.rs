//! Ports of `packages/lexical/src/__tests__/unit/LexicalExtensionCore.test.ts`.
//!
//! The upstream cases check TypeScript inference; here each binds the
//! result to a variable of the expected type, so the compiler checks it,
//! as Swift's port does. JS's `ExtensionConfigBase` (`{}`) config, and its
//! `undefined` output and `never` init, are `()`; a config object is an
//! `Object`.

#![cfg(feature = "extension")]

use crate::json::{Object, Value};
use crate::{
    LexicalExtension, NormalizedPeerDependency, declare_peer_dependency, define_extension,
};
use std::rc::Rc;

// upstream: lexical/src/__tests__/unit/LexicalExtensionCore.test.ts::defineExtension > does not change identity
#[test]
fn does_not_change_identity() {
    let extension_arg: LexicalExtension<(), (), ()> = LexicalExtension::new("test", ());
    let name_buffer = extension_arg.name.as_ptr();
    // `defineExtension` hands back the value it was given, moved behind
    // the `Rc` the builder shares, not a copy of it.
    let extension: Rc<LexicalExtension<(), (), ()>> = define_extension(extension_arg);
    assert_eq!(extension.name.as_ptr(), name_buffer);
    assert_eq!(extension.name, "test");
}

// upstream: lexical/src/__tests__/unit/LexicalExtensionCore.test.ts::defineExtension > infers the expected type (base case)
#[test]
fn infers_the_expected_type_base_case() {
    let extension: Rc<LexicalExtension<(), (), ()>> =
        define_extension(LexicalExtension::new("test", ()));
    assert_eq!(extension.name, "test");
}

// upstream: lexical/src/__tests__/unit/LexicalExtensionCore.test.ts::defineExtension > infers the expected type (config inference)
#[test]
fn infers_the_expected_type_config_inference() {
    let mut config = Object::new();
    config.insert("number", Value::from(123.0));
    let extension: Rc<LexicalExtension<Object, (), ()>> =
        define_extension(LexicalExtension::new("test", config));
    assert_eq!(extension.config.get("number"), Some(&Value::from(123.0)));
}

// upstream: lexical/src/__tests__/unit/LexicalExtensionCore.test.ts::defineExtension > infers the expected type (output inference)
#[cfg(feature = "extension")]
#[test]
fn infers_the_expected_type_output_inference() {
    use crate::extension::{build_editor_from_extensions, get_extension_dependency_from_editor};
    struct Output {
        output: i32,
    }
    let extension: Rc<LexicalExtension<(), Output, ()>> = define_extension(LexicalExtension {
        build: Some(Rc::new(|_, _, _| Ok(Output { output: 321 }))),
        ..LexicalExtension::new("test", ())
    });
    let editor = build_editor_from_extensions([extension.clone().into()]).unwrap();
    let dep = get_extension_dependency_from_editor(&editor, &extension).unwrap();
    assert_eq!(dep.output.unwrap().output, 321);
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalExtensionCore.test.ts::defineExtension > can define an extension without config
#[test]
fn can_define_an_extension_without_config() {
    let extension: Rc<LexicalExtension<(), (), ()>> =
        define_extension(LexicalExtension::new("test", ()));
    assert_eq!(extension.config, ());
}

// upstream: lexical/src/__tests__/unit/LexicalExtensionCore.test.ts::defineExtension > infers the correct init type
#[test]
fn infers_the_correct_init_type() {
    let extension: Rc<LexicalExtension<(), (), &'static str>> =
        define_extension(LexicalExtension {
            init: Some(Rc::new(|_, _, _| Ok("string"))),
            ..LexicalExtension::new("test", ())
        });
    assert!(extension.init.is_some());
}

// upstream: lexical/src/__tests__/unit/LexicalExtensionCore.test.ts::declarePeerDependency > validates the type argument
#[test]
fn declare_peer_dependency_validates_the_type_argument() {
    // The type argument is the peer's config: `Object` here.
    let dep: NormalizedPeerDependency = declare_peer_dependency::<Object>("other", None);
    // `['other']`: a name and no config.
    assert_eq!(dep.name(), "other");
    assert!(dep.config.is_none());
    let mut other = Object::new();
    other.insert("other", Value::Bool(false));
    let configured = declare_peer_dependency::<Object>("other", Some(other.clone()));
    // `['other', {other: false}]`.
    assert_eq!(configured.name(), "other");
    assert_eq!(
        configured
            .config
            .as_ref()
            .and_then(|c| c.downcast_ref::<Object>()),
        Some(&other)
    );
}
