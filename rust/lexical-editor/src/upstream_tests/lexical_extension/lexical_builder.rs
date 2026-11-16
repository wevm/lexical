//! Ports of `packages/lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts`.
//!
//! `rep.getState().config` is read with `getExtensionDependencyFromEditor`,
//! which returns the same merged config.

use crate::extension::{
    LexicalBuilder, build_editor_from_extensions, get_extension_dependency_from_editor,
};
use crate::nodes::text::TextData;
use crate::{
    AnyLexicalExtensionArgument, ExtensionConfigBase, InitialEditorStateType, LexicalExtension,
    LexicalNode, NodeBase, NodeClass, NodeRef, config_extension, create_paragraph_node,
    declare_peer_dependency, define_extension, get_root,
};
use std::rc::Rc;

const INITIAL_STATE_EXTENSION_NAME: &str = "@lexical/extension/InitialState";

/// `{a: 1; b: string | null}`.
#[derive(Clone, Debug, PartialEq)]
struct Config {
    a: i32,
    b: Option<String>,
}

/// `Partial<Config>` (only `b` is ever overridden).
#[derive(Clone, Default)]
struct ConfigPartial {
    b: Option<Option<String>>,
}

impl ExtensionConfigBase for Config {
    type Partial = ConfigPartial;
    fn shallow_merge(&self, overrides: &ConfigPartial) -> Self {
        Config {
            a: self.a,
            b: overrides.b.clone().unwrap_or_else(|| self.b.clone()),
        }
    }
}

type ConfigExtensionType = LexicalExtension<Config, (), ()>;

thread_local! {
    static CONFIG_EXTENSION: Rc<ConfigExtensionType> = define_extension(LexicalExtension::new(
        "Config",
        Config {
            a: 1,
            b: Some("b".to_owned()),
        },
    ));
}

fn config_ext() -> Rc<ConfigExtensionType> {
    CONFIG_EXTENSION.with(Rc::clone)
}

fn b_null() -> AnyLexicalExtensionArgument {
    config_extension(&config_ext(), [ConfigPartial { b: Some(None) }])
}

fn named(
    name: &str,
    dependencies: Vec<AnyLexicalExtensionArgument>,
) -> AnyLexicalExtensionArgument {
    define_extension(LexicalExtension {
        dependencies,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()
}

/// The names of the sorted reps and the merged config of ConfigExtension.
fn reps_and_config(roots: Vec<AnyLexicalExtensionArgument>) -> (Vec<String>, Config) {
    let editor = build_editor_from_extensions(roots).unwrap();
    let names = LexicalBuilder::from_editor(&editor)
        .unwrap()
        .sorted_extension_reps()
        .unwrap()
        .iter()
        .map(|rep| rep.extension.name().to_owned())
        .collect();
    let config = (*get_extension_dependency_from_editor(&editor, &config_ext())
        .unwrap()
        .config)
        .clone();
    editor.dispose();
    (names, config)
}

fn a1_b_null() -> Config {
    Config { a: 1, b: None }
}

// upstream: lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts::LexicalBuilder > merges extension configs (siblings)
#[test]
fn merges_configs_siblings() {
    let (names, config) = reps_and_config(vec![config_ext().into(), b_null()]);
    assert_eq!(names.len(), 2);
    assert_eq!(names.last().map(String::as_str), Some("Config"));
    assert_eq!(config, a1_b_null());
}

// upstream: lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts::LexicalBuilder > merges extension configs (parent override child)
#[test]
fn merges_configs_parent_override_child() {
    let (names, config) = reps_and_config(vec![named("parent", vec![b_null()])]);
    assert_eq!(names.len(), 3);
    assert!(names.iter().any(|n| n == "Config"));
    assert_eq!(config, a1_b_null());
}

// upstream: lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts::LexicalBuilder > merges extension configs (grandparent override child) override after
#[test]
fn merges_configs_grandparent_override_after() {
    let (names, config) = reps_and_config(vec![named(
        "grandparent",
        vec![named("parent", vec![config_ext().into()]), b_null()],
    )]);
    assert_eq!(names.len(), 4);
    assert_eq!(config, a1_b_null());
}

// upstream: lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts::LexicalBuilder > merges extension configs (grandparent override child) override before
#[test]
fn merges_configs_grandparent_override_before() {
    let (names, config) = reps_and_config(vec![named(
        "grandparent",
        vec![b_null(), named("parent", vec![config_ext().into()])],
    )]);
    assert_eq!(names.len(), 4);
    assert_eq!(config, a1_b_null());
}

/// `{first: string; second: string}`.
#[derive(Clone, Debug, PartialEq)]
struct Pair {
    first: String,
    second: String,
}

#[derive(Clone, Default)]
struct PairPartial {
    first: Option<String>,
    second: Option<String>,
}

impl ExtensionConfigBase for Pair {
    type Partial = PairPartial;
    fn shallow_merge(&self, overrides: &PairPartial) -> Self {
        Pair {
            first: overrides
                .first
                .clone()
                .unwrap_or_else(|| self.first.clone()),
            second: overrides
                .second
                .clone()
                .unwrap_or_else(|| self.second.clone()),
        }
    }
}

thread_local! {
    static PAIR_EXTENSION: Rc<LexicalExtension<Pair, (), ()>> = define_extension(LexicalExtension::new(
        "Pair",
        Pair {
            first: "default".to_owned(),
            second: "default".to_owned(),
        },
    ));
}

fn pair_ext() -> Rc<LexicalExtension<Pair, (), ()>> {
    PAIR_EXTENSION.with(Rc::clone)
}

fn pair(first: Option<&str>, second: Option<&str>) -> AnyLexicalExtensionArgument {
    config_extension(
        &pair_ext(),
        [PairPartial {
            first: first.map(str::to_owned),
            second: second.map(str::to_owned),
        }],
    )
}

/// `configOf(...dependencies)`.
fn config_of(dependencies: Vec<AnyLexicalExtensionArgument>) -> Pair {
    let editor = build_editor_from_extensions([named("root", dependencies)]).unwrap();
    let config = (*get_extension_dependency_from_editor(&editor, &pair_ext())
        .unwrap()
        .config)
        .clone();
    editor.dispose();
    config
}

fn p(first: &str, second: &str) -> Pair {
    Pair {
        first: first.to_owned(),
        second: second.to_owned(),
    }
}

// upstream: lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts::LexicalBuilder > a dependency configured more than once by one extension > applies every config rather than only the last
#[test]
fn applies_every_config() {
    assert_eq!(
        config_of(vec![pair(Some("one"), None), pair(None, Some("two"))]),
        p("one", "two")
    );
}

// upstream: lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts::LexicalBuilder > a dependency configured more than once by one extension > applies them in order, so the last one wins a conflict
#[test]
fn applies_configs_in_order() {
    assert_eq!(
        config_of(vec![pair(Some("one"), None), pair(Some("two"), None)]),
        p("two", "default")
    );
}

// upstream: lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts::LexicalBuilder > a dependency configured more than once by one extension > keeps a peer dependency config alongside a direct one
#[test]
fn keeps_a_peer_dependency_config() {
    let peer: AnyLexicalExtensionArgument = define_extension(LexicalExtension {
        peer_dependencies: vec![declare_peer_dependency::<Pair>(
            "Pair",
            Some(PairPartial {
                second: Some("peer".to_owned()),
                ..Default::default()
            }),
        )],
        ..LexicalExtension::<(), (), ()>::new("Peer", ())
    })
    .into();
    assert_eq!(
        config_of(vec![pair(Some("direct"), None), peer]),
        p("direct", "peer")
    );
}

/// A `TextNode` subclass of the given type.
macro_rules! text_class {
    ($name:ident, $type:literal) => {
        #[derive(Clone, Debug, Default)]
        struct $name {
            base: NodeBase,
            text: TextData,
        }

        impl LexicalNode for $name {
            fn get_type(&self) -> &'static str {
                $type
            }
            fn base(&self) -> &NodeBase {
                &self.base
            }
            fn base_mut(&mut self) -> &mut NodeBase {
                &mut self.base
            }
            fn text(&self) -> Option<&TextData> {
                Some(&self.text)
            }
            fn text_mut(&mut self) -> Option<&mut TextData> {
                Some(&mut self.text)
            }
        }
    };
}

text_class!(NodeA, "node-a");
text_class!(NodeB, "node-b");

fn create<T: LexicalNode + Default>(text: &str) -> NodeRef {
    let node = crate::lexical_utils::set_node_key(Box::new(T::default()));
    node.set_text_content(text);
    node
}

// upstream: lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts::LexicalBuilder > nodes configuration > can mix deferred and direct node config
#[test]
fn can_mix_deferred_and_direct_node_config() {
    // JS's `nodes: () => [NodeA]` defers naming a class until the editor is
    // built; Rust's `nodes` is a list either way, so the two extensions
    // differ only in which class they register.
    let ext_defer = define_extension(LexicalExtension {
        nodes: vec![NodeClass::of::<NodeA>()],
        ..LexicalExtension::<(), (), ()>::new("A", ())
    });
    let ext_direct = define_extension(LexicalExtension {
        nodes: vec![NodeClass::of::<NodeB>()],
        ..LexicalExtension::<(), (), ()>::new("B", ())
    });
    let state = define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(|_| {
            let paragraph = create_paragraph_node();
            paragraph.append(&[create::<NodeA>("defer"), create::<NodeB>("direct")])?;
            get_root().append(&[paragraph])?;
            Ok(())
        }))),
        ..LexicalExtension::<(), (), ()>::new("state", ())
    });
    let editor =
        build_editor_from_extensions([ext_defer.into(), ext_direct.into(), state.into()]).unwrap();
    let texts = editor
        .read(|| {
            get_root()
                .get_all_text_nodes()
                .iter()
                .map(|node| (node.get_type().to_owned(), node.get_text_content()))
                .collect::<Vec<_>>()
        })
        .unwrap();
    assert_eq!(
        texts,
        [
            ("node-a".to_owned(), "defer".to_owned()),
            ("node-b".to_owned(), "direct".to_owned())
        ]
    );
    editor.dispose();
}

/// `ExtensionA`, with a peer dependency on Config setting `b: 'A'`.
fn extension_a() -> AnyLexicalExtensionArgument {
    define_extension(LexicalExtension {
        peer_dependencies: vec![declare_peer_dependency::<Config>(
            "Config",
            Some(ConfigPartial {
                b: Some(Some("A".to_owned())),
            }),
        )],
        ..LexicalExtension::<(), (), ()>::new("A", ())
    })
    .into()
}

fn peer_case() {
    let (names, config) = reps_and_config(vec![extension_a(), config_ext().into()]);
    assert_eq!(names, [INITIAL_STATE_EXTENSION_NAME, "Config", "A"]);
    assert_eq!(
        config,
        Config {
            a: 1,
            b: Some("A".to_owned())
        }
    );
}

// upstream: lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts::LexicalBuilder > handles peer dependency configuration > peer-first
#[test]
fn peer_first() {
    peer_case();
}

// upstream: lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts::LexicalBuilder > handles peer dependency configuration > peer-last
#[test]
fn peer_last() {
    // Upstream's two cases build the same editor.
    peer_case();
}
