use std::collections::{BTreeMap, BTreeSet};

use cargo::core::PackageId;
use serde::Serialize;

pub struct BuckFile {
    pkg_id_to_rules: BTreeMap<PackageId, BTreeSet<InternalRule>>,
    add_buildscript_run_import: bool,
}

pub trait StarlarkRule: Serialize + PartialEq + Eq + PartialOrd + Ord {
    fn into_starlark(self) -> Result<String, serde_starlark::Error>;
    fn into_internal_rule(self) -> InternalRule;
}

impl BuckFile {
    pub fn new() -> Self {
        Self {
            pkg_id_to_rules: BTreeMap::new(),
            add_buildscript_run_import: false,
        }
    }

    pub fn add_rule(&mut self, pkg_id: &PackageId, rule: impl StarlarkRule) {
        let pkg_set = match self.pkg_id_to_rules.get_mut(pkg_id) {
            Some(s) => s,
            None => {
                self.pkg_id_to_rules.insert(*pkg_id, BTreeSet::new());
                self.pkg_id_to_rules.get_mut(pkg_id).unwrap()
            }
        };
        let internal_rule = rule.into_internal_rule();
        if matches!(internal_rule, InternalRule::BuildScriptRun(_)) {
            self.add_buildscript_run_import = true;
        }
        pkg_set.insert(internal_rule);
    }

    pub fn into_starlark_vec(self) -> Vec<u8> {
        let mut vec = vec![];
        if self.add_buildscript_run_import {
            vec.extend_from_slice(
                Load(
                    "@prelude//rust:cargo_buildscript.bzl".to_string(),
                    "buildscript_run".to_string(),
                )
                .into_starlark()
                .unwrap()
                .as_bytes(),
            );
        }

        for (_, val) in self.pkg_id_to_rules {
            for elm in val {
                vec.extend_from_slice(elm.into_starlark().unwrap().as_bytes());
            }
        }
        vec
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum InternalRule {
    RustBinary(RustBinary),
    Glob(Glob),
    Load(Load),
    RustLibrary(RustLibrary),
    HttpArchive(HttpArchive),
    BuildScriptRun(BuildScriptRun),
}
impl InternalRule {
    fn into_starlark(self) -> Result<String, serde_starlark::Error> {
        match self {
            InternalRule::RustBinary(v) => v.into_starlark(),
            InternalRule::Glob(v) => v.into_starlark(),
            InternalRule::Load(v) => v.into_starlark(),
            InternalRule::RustLibrary(v) => v.into_starlark(),
            InternalRule::HttpArchive(v) => v.into_starlark(),
            InternalRule::BuildScriptRun(v) => v.into_starlark(),
        }
    }
}

#[derive(Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename = "rust_binary")]
pub struct RustBinary {
    pub name: String,
    pub visibility: Vec<String>,

    pub srcs: Srcs,
    pub edition: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub named_deps: Option<BTreeMap<String, String>>,

    pub deps: Vec<String>,
    pub crate_root: String,
    #[serde(rename = "crate")]
    pub crate_name: String,
    pub features: Vec<String>,
    pub env: BTreeMap<String, String>,
}

#[derive(Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename = "glob")]
pub struct Glob(pub BTreeSet<String>);

#[derive(Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename = "load")]
pub struct Load(pub String, pub String);

#[derive(Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(untagged)]
pub enum Srcs {
    Glob(Glob),
    Plain(Vec<String>),
}

#[derive(Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename = "rust_library")]
pub struct RustLibrary {
    pub name: String,
    pub visibility: Vec<String>,
    pub srcs: Vec<String>,
    pub edition: String,
    pub crate_root: String,
    #[serde(rename = "crate")]
    pub crate_name: String,
    #[serde(skip_serializing_if = "is_false")]
    pub proc_macro: bool,
    pub deps: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub named_deps: Option<BTreeMap<String, String>>,
    pub features: Vec<String>,
    pub env: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rustc_flags: Option<Vec<String>>,
}

fn is_false(b: &bool) -> bool {
    !b
}

#[derive(Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename = "http_archive")]
pub struct HttpArchive {
    pub name: String,
    pub sha256: String,
    pub strip_prefix: String,
    pub urls: Vec<String>,
    pub visibility: Vec<String>,
}

#[derive(Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename = "buildscript_run")]
pub struct BuildScriptRun {
    pub name: String,
    pub package_name: String,
    pub buildscript_rule: String,
    pub env: BTreeMap<String, String>,
    pub features: Vec<String>,
    pub version: String,
}

macro_rules! impl_starlark_rule {
    ($Type:ident) => {
        impl StarlarkRule for $Type {
            fn into_internal_rule(self) -> InternalRule {
                InternalRule::$Type(self)
            }
            fn into_starlark(self) -> Result<String, serde_starlark::Error> {
                serde_starlark::to_string(&self)
            }
        }
    };
}
impl_starlark_rule!(RustBinary);
impl_starlark_rule!(Glob);
impl_starlark_rule!(Load);
impl_starlark_rule!(RustLibrary);
impl_starlark_rule!(HttpArchive);
impl_starlark_rule!(BuildScriptRun);
