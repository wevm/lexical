//! Ports of `packages/lexical/src/__tests__/unit/LexicalWarning.test.ts`.

use crate::json::{Object, Value};
#[cfg(feature = "extension")]
use crate::{CreateEditorArgs, Editor, create_editor};
use crate::{LexicalError, LexicalWarning};
#[cfg(feature = "extension")]
use std::cell::RefCell;
#[cfg(feature = "extension")]
use std::rc::Rc;

// upstream: lexical/src/__tests__/unit/LexicalWarning.test.ts::LexicalWarning > is an Error with a code and data
#[test]
fn is_an_error_with_a_code_and_data() {
    let mut data = Object::new();
    data.insert("n", Value::from(1.0));
    let warning = LexicalWarning::new(
        "@example/pkg/thing",
        "a thing",
        Some(Value::Object(data.clone())),
    );
    // `toBeInstanceOf(Error)`: it is an error, and as the error `onWarn`
    // receives it is still a `LexicalWarning`.
    let _: &dyn std::error::Error = &warning;
    let error = LexicalError::from_warning(warning.clone());
    assert_eq!(error.warning(), Some(&warning));
    assert_eq!(LexicalWarning::NAME, "LexicalWarning");
    assert_eq!(warning.message, "a thing");
    assert_eq!(error.message(), "a thing");
    assert_eq!(warning.code, "@example/pkg/thing");
    assert_eq!(warning.data, Some(Value::Object(data)));
    assert_eq!(warning.to_string(), "LexicalWarning: a thing");
    assert_eq!(
        LexicalWarning::new("@example/pkg/other", "no data", None).data,
        None
    );
}

// upstream: lexical/src/__tests__/unit/LexicalWarning.test.ts::LexicalWarning > accepts any code (a switch needs a default branch)
#[test]
fn accepts_any_code() {
    let describe_code = |code: &str| -> &'static str {
        match code {
            "@example/pkg/known" => "known",
            _ => "other",
        }
    };
    assert_eq!(
        describe_code(&LexicalWarning::new("@example/pkg/known", "", None).code),
        "known"
    );
    assert_eq!(describe_code("@example/pkg/new-code"), "other");
}

// upstream: lexical/src/__tests__/unit/LexicalWarning.test.ts::LexicalWarning > is passed to the editor's onWarn as it is
#[cfg(feature = "extension")]
#[test]
fn is_passed_to_the_editors_on_warn_as_it_is() {
    use crate::extension::build_editor_from_extensions;
    use crate::{LexicalExtension, define_extension};

    let seen = Rc::new(RefCell::new(Vec::<LexicalError>::new()));
    let log = seen.clone();
    let editor = create_editor(CreateEditorArgs {
        on_warn: Some(Rc::new(move |error| {
            log.borrow_mut().push(error);
            Ok(())
        })),
        ..CreateEditorArgs::default()
    });
    let warning = LexicalWarning::new("@example/pkg/thing", "a thing", None);
    editor
        .on_warn(LexicalError::from_warning(warning.clone()))
        .unwrap();
    assert_eq!(seen.borrow().len(), 1);
    assert_eq!(seen.borrow()[0].warning(), Some(&warning));

    let from_extension = Rc::new(RefCell::new(Vec::<(LexicalError, Editor)>::new()));
    let log = from_extension.clone();
    let built = build_editor_from_extensions([define_extension(LexicalExtension {
        on_warn: Some(Rc::new(move |error, ed: &Editor| {
            log.borrow_mut().push((error, ed.clone()));
            Ok(())
        })),
        ..LexicalExtension::<(), (), ()>::new("@example/app", ())
    })
    .into()])
    .unwrap();
    built
        .on_warn(LexicalError::from_warning(warning.clone()))
        .unwrap();
    {
        let from_extension = from_extension.borrow();
        assert_eq!(from_extension.len(), 1);
        assert_eq!(from_extension[0].0.warning(), Some(&warning));
        assert!(from_extension[0].1 == *built);
    }
    built.dispose();
}
