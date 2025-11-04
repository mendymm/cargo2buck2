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

use crate::{
    buck_file::{BuckFile, BuildScriptRun, Glob, HttpArchive, RustBinary, RustLibrary, Srcs},
    custom_metadata::CustomMetadata,
};

mod buck_file;
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
    let gctx = GlobalContext::default().unwrap();

    let ws = Workspace::new(&ws_path.join("Cargo.toml"), &gctx).unwrap();
    let specs = ws
        .members()
        .map(|p| p.package_id().to_spec())
        .collect::<Vec<_>>();
    let mut target_data = RustcTargetData::new(&ws, &[CompileKind::Host]).unwrap();

    let cli_features = CliFeatures::from_command_line(&[], false, false).unwrap();

    let resolved = resolve_ws_with_opts(
        &ws,
        &mut target_data,
        &[CompileKind::Host],
        &cli_features,
        &specs,
        cargo::core::resolver::HasDevUnits::Yes,
        cargo::core::resolver::ForceAllTargets::No,
        false,
    )
    .unwrap();

    let resolved_workspace = resolved.workspace_resolve.unwrap();

    for pkg_id in resolved.targeted_resolve.iter() {
        let pkg = resolved.pkg_set.get_one(pkg_id).unwrap();

        let package_id = pkg.package_id();
        let _metadata: CustomMetadata = match pkg.manifest().custom_metadata() {
            Some(custom_meta) => {
                let cargo2buck2 = custom_meta.get("cargo2buck2");
                cargo2buck2
                    .map(|v| v.to_owned().try_into::<CustomMetadata>().unwrap())
                    .unwrap_or_default()
            }
            None => CustomMetadata::default(),
        };

        let deps = resolved
            .targeted_resolve
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
        let named_deps = resolved
            .targeted_resolve
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
            cargo_env.insert("CARGO_PKG_NAME".to_string(), pkg.name().to_string());

            match target.kind() {
                TargetKind::Lib(crate_types) => {
                    assert_eq!(crate_types.len(), 1);
                    let crate_type = &crate_types[0];

                    if let Some(sha256) = pkg.summary().checksum() {
                        buck_file.add_rule(
                            &package_id,
                            HttpArchive {
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
                            },
                        );
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

                            buck_file.add_rule(
                                &package_id,
                                RustLibrary {
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
                                },
                            );
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

                            buck_file.add_rule(
                                &package_id,
                                RustLibrary {
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
                                },
                            );
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
                    buck_file.add_rule(
                        &package_id,
                        RustBinary {
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
                        },
                    );
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
                    buck_file.add_rule(
                        &package_id,
                        RustBinary {
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
                        },
                    );
                    buck_file.add_rule(
                        &package_id,
                        BuildScriptRun {
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
                        },
                    );
                }
            }
        }
    }

    std::fs::write(ws_path.join("BUCK"), buck_file.into_starlark_vec()).unwrap();
}
