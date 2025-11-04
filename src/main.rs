use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use cargo::{
    GlobalContext,
    core::{
        TargetKind, Workspace,
        compiler::{CompileKind, CrateType, RustcTargetData},
        resolver::CliFeatures,
    },
    ops::resolve_ws_with_opts,
};
use serde::Serialize;

use crate::custom_metadata::CustomMetadata;

mod custom_metadata;

fn main() {
    for path in [
        "example-projects/simple-no-deps-bin",
        "example-projects/proc-macro-dep",
        "example-projects/simple-single-dep-bin",
        "example-projects/bin-with-build-rs",
        "example-projects/renamed-dep",
    ] {
        buckify_workspace(&Path::new(path).canonicalize().unwrap());
    }
}

fn buckify_workspace(ws_path: &Path) {
    let mut buck_file = BuckFile::new();
    buck_file.add_rule(Load(
        "@prelude//rust:cargo_buildscript.bzl".to_string(),
        "buildscript_run".to_string(),
    ));

    let gctx = GlobalContext::default().unwrap();

    let ws = Workspace::new(&ws_path.join("Cargo.toml"), &gctx).unwrap();
    let specs = ws
        .members()
        .map(|p| p.package_id().to_spec())
        .collect::<Vec<_>>();
    let mut target_data = RustcTargetData::new(&ws, &[CompileKind::Host]).unwrap();

    let cli_features = CliFeatures::from_command_line(&[], false, true).unwrap();

    let resolved = resolve_ws_with_opts(
        &ws,
        &mut target_data,
        &[CompileKind::Host],
        &cli_features,
        &specs,
        cargo::core::resolver::HasDevUnits::Yes,
        cargo::core::resolver::ForceAllTargets::Yes,
        false,
    )
    .unwrap();

    let resolved_workspace = resolved.workspace_resolve.unwrap();

    for pkg in resolved.pkg_set.packages() {
        let _metadata: CustomMetadata = match pkg.manifest().custom_metadata() {
            Some(custom_meta) => {
                let cargo2buck2 = custom_meta.get("cargo2buck2");
                cargo2buck2
                    .map(|v| v.to_owned().try_into::<CustomMetadata>().unwrap())
                    .unwrap_or_default()
            }
            None => CustomMetadata::default(),
        };

        let deps = resolved_workspace
            .deps(pkg.package_id())
            .filter_map(|(dep_id, deps)| {
                let dep = deps.iter().next().unwrap();
                if dep.explicit_name_in_toml().is_none() {
                    Some(format!(":{}-{}", dep_id.name(), dep_id.version()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let named_deps = resolved_workspace
            .deps(pkg.package_id())
            .filter_map(|(dep_id, deps)| {
                let dep = deps.iter().next().unwrap();
                dep.explicit_name_in_toml().map(|explicit_name_in_toml| {
                    (
                        explicit_name_in_toml.to_string(),
                        format!(":{}-{}", dep_id.name(), dep_id.version()),
                    )
                })
            })
            .collect::<BTreeMap<_, _>>();
        let named_deps = match named_deps.is_empty() {
            true => None,
            false => Some(named_deps),
        };

        for target in pkg.targets() {
            let crate_root = target
                .src_path()
                .path()
                .unwrap()
                .strip_prefix(pkg.root())
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            let version = pkg.package_id().version();

            let mut cargo_env = BTreeMap::new();
            cargo_env.insert(
                "CARGO_PKG_VERSION_PATCH".to_string(),
                version.patch.to_string(),
            );
            cargo_env.insert("CARGO_PKG_VERSION".to_string(), version.to_string());

            match target.kind() {
                TargetKind::Lib(crate_types) => {
                    assert_eq!(crate_types.len(), 1);
                    let crate_type = &crate_types[0];

                    if let Some(sha256) = pkg.summary().checksum() {
                        buck_file.add_rule(HttpArchive {
                            name: pkg.package_id().tarball_name(),
                            sha256: sha256.to_string(),
                            strip_prefix: pkg
                                .package_id()
                                .tarball_name()
                                .strip_suffix(".crate")
                                .unwrap()
                                .to_string(),
                            urls: vec![format!(
                                "https://static.crates.io/crates/{}/{}/download",
                                pkg.package_id().name(),
                                pkg.package_id().version()
                            )],
                            visibility: vec!["PUBLIC".to_string()],
                        });
                    }

                    match crate_type {
                        CrateType::Bin => todo!(),
                        CrateType::Rlib => todo!(),
                        CrateType::Dylib => todo!(),
                        CrateType::Cdylib => todo!(),
                        CrateType::Staticlib => todo!(),
                        CrateType::ProcMacro => {
                            let mut env = cargo_env.clone();
                            if pkg.has_custom_build() {
                                env.insert(
                                    "OUT_DIR".to_string(),
                                    format!(
                                        "$(location :{}-{}-build-script-run[out_dir])",
                                        pkg.name(),
                                        pkg.version()
                                    ),
                                );
                            }
                            let rustc_flags = match pkg.has_custom_build() {
                                true => Some(vec![format!(
                                    "@$(location :{}[rustc_flags])",
                                    format!("{}-{}-build-script-run", pkg.name(), pkg.version()),
                                )]),
                                false => None,
                            };

                            buck_file.add_rule(RustLibrary {
                                name: format!("{}-{}", pkg.name(), pkg.version()),
                                edition: target.edition().to_string(),
                                visibility: vec!["PUBLIC".to_string()],
                                srcs: vec![format!(":{}", pkg.package_id().tarball_name())],
                                crate_root: format!(
                                    "{}/{}",
                                    pkg.package_id().tarball_name(),
                                    crate_root
                                ),
                                crate_name: pkg.name().to_string(),
                                proc_macro: true,
                                deps: deps.clone(),
                                named_deps: named_deps.clone(),
                                features: resolved_workspace
                                    .features(pkg.package_id())
                                    .iter()
                                    .map(|s| s.to_string())
                                    .collect(),
                                env,
                                rustc_flags,
                            });
                        }
                        CrateType::Other(_) => todo!(),
                        CrateType::Lib => {
                            let mut env = cargo_env.clone();
                            if pkg.has_custom_build() {
                                env.insert(
                                    "OUT_DIR".to_string(),
                                    format!(
                                        "$(location :{}-{}-build-script-run[out_dir])",
                                        pkg.name(),
                                        pkg.version()
                                    ),
                                );
                            }
                            let rustc_flags = match pkg.has_custom_build() {
                                true => Some(vec![format!(
                                    "@$(location :{}[rustc_flags])",
                                    format!("{}-{}-build-script-run", pkg.name(), pkg.version()),
                                )]),
                                false => None,
                            };

                            buck_file.add_rule(RustLibrary {
                                name: format!("{}-{}", pkg.name(), pkg.version()),
                                edition: target.edition().to_string(),
                                visibility: vec!["PUBLIC".to_string()],
                                srcs: vec![format!(":{}", pkg.package_id().tarball_name())],
                                crate_root: format!(
                                    "{}/{}",
                                    pkg.package_id().tarball_name(),
                                    crate_root
                                ),
                                crate_name: pkg.name().to_string(),
                                proc_macro: false,
                                deps: deps.clone(),
                                named_deps: named_deps.clone(),
                                features: resolved_workspace
                                    .features(pkg.package_id())
                                    .iter()
                                    .map(|s| s.to_string())
                                    .collect(),
                                env,
                                rustc_flags,
                            });
                        }
                    }
                }
                TargetKind::Bin => {
                    let mut env = cargo_env.clone();
                    if pkg.has_custom_build() {
                        env.insert(
                            "OUT_DIR".to_string(),
                            format!(
                                "$(location :{}-{}-build-script-run[out_dir])",
                                pkg.name(),
                                pkg.version()
                            ),
                        );
                    }
                    buck_file.add_rule(RustBinary {
                        name: target.name().to_string(),
                        edition: target.edition().to_string(),
                        visibility: vec!["PUBLIC".to_string()],
                        srcs: Srcs::Glob(Glob(BTreeSet::from_iter(["src/*.rs".to_string()]))),
                        deps: deps.clone(),
                        named_deps: named_deps.clone(),
                        crate_root,
                        crate_name: pkg.name().to_string(),
                        features: resolved_workspace
                            .features(pkg.package_id())
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                        env,
                    });
                }
                // TODO: impl these
                TargetKind::ExampleBin | TargetKind::Bench | TargetKind::Test => (),
                //  => todo!(),
                TargetKind::ExampleLib(_crate_types) => (),
                TargetKind::CustomBuild => {
                    let build_script_rule =
                        format!("{}-{}-build-script-build", pkg.name(), pkg.version());
                    let srcs = match pkg.package_id().source_id().is_path() {
                        true => Srcs::Plain(vec!["build.rs".to_string()]),
                        false => Srcs::Plain(vec![format!(":{}", pkg.package_id().tarball_name())]),
                    };
                    let crate_root = match pkg.package_id().source_id().is_path() {
                        true => "build.rs".to_string(),
                        false => format!("{}/{}", pkg.package_id().tarball_name(), crate_root),
                    };
                    buck_file.add_rule(RustBinary {
                        name: build_script_rule.clone(),
                        crate_name: "build_script_build".to_string(),
                        visibility: vec!["PUBLIC".to_string()],
                        edition: target.edition().to_string(),
                        srcs,
                        crate_root,
                        deps: deps.clone(),
                        named_deps: named_deps.clone(),
                        features: resolved_workspace
                            .features(pkg.package_id())
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                        env: cargo_env.clone(),
                    });
                    buck_file.add_rule(BuildScriptRun {
                        buildscript_rule: format!(":{build_script_rule}"),
                        name: format!("{}-{}-build-script-run", pkg.name(), pkg.version()),
                        env: cargo_env.clone(),
                        features: resolved_workspace
                            .features(pkg.package_id())
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                        package_name: pkg.name().to_string(),
                        version: pkg.version().to_string(),
                    });
                }
            }
        }
    }

    std::fs::write(ws_path.join("BUCK"), buck_file.internal_buf).unwrap();
}

struct BuckFile {
    internal_buf: Vec<u8>,
}

trait IntoStarlark {
    fn into_starlark(self) -> String;
}

impl<T> IntoStarlark for T
where
    T: Serialize,
{
    fn into_starlark(self) -> String {
        serde_starlark::to_string(&self).unwrap()
    }
}

impl BuckFile {
    pub fn new() -> Self {
        Self {
            internal_buf: vec![],
        }
    }
    pub fn add_rule(&mut self, rule: impl IntoStarlark) {
        self.internal_buf
            .extend_from_slice(rule.into_starlark().as_bytes());
        self.internal_buf.push(b'\n');
    }
}

#[derive(Serialize)]
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
#[derive(Serialize)]
#[serde(rename = "glob")]
pub struct Glob(pub BTreeSet<String>);

#[derive(Serialize)]
#[serde(rename = "load")]
pub struct Load(pub String, pub String);

#[derive(Serialize)]
#[serde(untagged)]
pub enum Srcs {
    Glob(Glob),
    Plain(Vec<String>),
}

#[derive(Serialize)]
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

#[derive(Serialize)]
#[serde(rename = "http_archive")]
pub struct HttpArchive {
    pub name: String,
    pub sha256: String,
    pub strip_prefix: String,
    pub urls: Vec<String>,
    pub visibility: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename = "buildscript_run")]
pub struct BuildScriptRun {
    pub name: String,
    pub package_name: String,
    pub buildscript_rule: String,
    pub env: BTreeMap<String, String>,
    pub features: Vec<String>,
    pub version: String,
}
