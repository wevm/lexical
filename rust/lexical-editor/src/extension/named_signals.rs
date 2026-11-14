//! `namedSignals.ts`.

use super::signals::{Signal, signal};
use crate::json::{Object, Value};

/// `NamedSignalsOutput`: a signal per key of an object, in its key order.
#[derive(Clone, Default)]
pub struct NamedSignalsOutput {
    signals: Vec<(String, Signal<Value>)>,
}

impl NamedSignalsOutput {
    /// The signal of `key`.
    pub fn get(&self, key: &str) -> Option<&Signal<Value>> {
        self.signals.iter().find(|(k, _)| k == key).map(|(_, s)| s)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Signal<Value>)> {
        self.signals.iter().map(|(k, s)| (k.as_str(), s))
    }
}

/// `namedSignals(defaults, opts)`: a [`Signal`] for each key of `defaults`,
/// holding the value of `opts` for the key unless that is absent or
/// `undefined`, otherwise the default. Typically a reactive copy of part
/// of an extension's config, so it can be reconfigured at runtime.
///
/// JS takes any object; this takes a JSON object. An extension with a
/// typed config makes its signals with [`signal`] instead, one field each.
pub fn named_signals(defaults: &Object, opts: Option<&Object>) -> NamedSignalsOutput {
    let signals = defaults
        .iter()
        .map(|(k, default)| {
            let value = match opts.and_then(|o| o.get(k)) {
                None | Some(Value::Undefined) => default.clone(),
                Some(v) => v.clone(),
            };
            (k.to_owned(), signal(value))
        })
        .collect();
    NamedSignalsOutput { signals }
}

/// A typed config whose output is `namedSignals(config)`: the config
/// struct, its `Partial` (each field an `Option`, merged as
/// `shallowMergeConfig` merges), and the output, a [`Signal`] per field
/// with an `of(config)` that makes them (JS's `namedSignals(config)`).
macro_rules! named_signals_config {
    (
        $(#[$config_meta:meta])*
        pub struct $config:ident;
        $(#[$partial_meta:meta])*
        pub struct $partial:ident;
        $(#[$output_meta:meta])*
        pub struct $output:ident {
            $($(#[$field_meta:meta])* $field:ident: $ty:ty,)*
        }
    ) => {
        $(#[$config_meta])*
        #[derive(Clone)]
        pub struct $config {
            $($(#[$field_meta])* pub $field: $ty,)*
        }

        $(#[$partial_meta])*
        #[derive(Clone, Default)]
        pub struct $partial {
            $(pub $field: Option<$ty>,)*
        }

        impl $crate::ExtensionConfigBase for $config {
            type Partial = $partial;

            fn shallow_merge(&self, overrides: &$partial) -> Self {
                $config {
                    $($field: overrides.$field.clone().unwrap_or_else(|| self.$field.clone()),)*
                }
            }
        }

        $(#[$output_meta])*
        #[derive(Clone)]
        pub struct $output {
            $(pub $field: $crate::extension::Signal<$ty>,)*
        }

        impl $output {
            /// `namedSignals(config)`.
            pub fn of(config: &$config) -> Self {
                $output {
                    $($field: $crate::extension::signal(config.$field.clone()),)*
                }
            }
        }
    };
}

pub(crate) use named_signals_config;
