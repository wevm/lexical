//! `extension-core/shallowMergeConfig.ts`.

use super::types::ExtensionConfigBase;

/// `shallowMergeConfig(config, overrides)`: the default merge of config
/// overrides, `config` without them, otherwise
/// [`ExtensionConfigBase::shallow_merge`] (`{...config, ...overrides}`).
pub fn shallow_merge_config<C: ExtensionConfigBase>(
    config: &C,
    overrides: Option<&C::Partial>,
) -> C {
    match overrides {
        None => config.clone(),
        Some(overrides) => config.shallow_merge(overrides),
    }
}
