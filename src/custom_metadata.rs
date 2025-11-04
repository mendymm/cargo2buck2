#![allow(dead_code)]
use std::collections::BTreeSet;

use serde::Deserialize;

/// The `cargo2buck2` section in a Cargo.toml file
///
/// example
///
/// [package.metadata.cargo2buck2]
/// read_env_vars_from_build_script = ["MY_VAR"]
///
#[derive(Debug, Deserialize, Default)]
pub struct CustomMetadata {
    /// List of environment variables to read from the output of the build script
    ///
    /// TODO(pre-alpha): find a better name for this
    pub read_env_vars_from_build_script: BTreeSet<String>,
}
