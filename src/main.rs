use std::{collections::BTreeSet, path::Path};

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

fn main() {
    for path in [
        "example-projects/simple-no-deps-bin",
        "example-projects/simple-single-dep-bin",
        "example-projects/basic-build-script",
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

    let cli_features = CliFeatures::from_command_line(&[], false, true).unwrap();

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

    for pkg in resolved.pkg_set.packages() {
        // assert_eq!(pkg.targets().len(), 1);
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

            match target.kind() {
                TargetKind::Lib(crate_types) => {
                    assert_eq!(crate_types.len(), 1);
                    let crate_type = &crate_types[0];
                    let lib_name = format!(
                        "{}-{}-{crate_type}",
                        pkg.package_id().name(),
                        pkg.package_id().version()
                    );

                    buck_file.add_rule(HttpArchive {
                        name: pkg.package_id().tarball_name(),
                        sha256: pkg.summary().checksum().unwrap().to_string(),
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

                    match crate_type {
                        CrateType::Bin => todo!(),
                        CrateType::Rlib => todo!(),
                        CrateType::Dylib => todo!(),
                        CrateType::Cdylib => todo!(),
                        CrateType::Staticlib => todo!(),
                        CrateType::ProcMacro => {
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
                                deps: resolved_workspace
                                    .deps(pkg.package_id())
                                    .map(|(dep_id, _dep)| {
                                        format!(":{}-{}", dep_id.name(), dep_id.version())
                                    })
                                    .collect::<Vec<_>>(),
                            });
                        }
                        CrateType::Other(_) => todo!(),
                        CrateType::Lib => {
                            // for dep in resolved_workspace.deps(pkg.package_id()){
                            //     dbg!(dep);
                            // }
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
                                deps: resolved_workspace
                                    .deps(pkg.package_id())
                                    .map(|(dep_id, _dep)| {
                                        format!(":{}-{}", dep_id.name(), dep_id.version())
                                    })
                                    .collect::<Vec<_>>(),
                            });
                        }
                    }
                }
                TargetKind::Bin => {
                    buck_file.add_rule(RustBinary {
                        name: target.name().to_string(),
                        edition: target.edition().to_string(),
                        visibility: vec!["PUBLIC".to_string()],
                        srcs: Glob(BTreeSet::from_iter(["src/*.rs".to_string()])),
                        deps: resolved_workspace
                            .deps(pkg.package_id())
                            .map(|(dep_id, _dep)| {
                                format!(":{}-{}", dep_id.name(), dep_id.version())
                            })
                            .collect::<Vec<_>>(),
                        crate_root,
                        crate_name: pkg.name().to_string(),
                    });
                }
                // TODO: impl these
                TargetKind::ExampleBin | TargetKind::Bench | TargetKind::Test => (),
                TargetKind::ExampleLib(_crate_types) => todo!(),
                TargetKind::CustomBuild => {}
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
    pub srcs: Glob,
    pub edition: String,
    pub deps: Vec<String>,
    pub crate_root: String,
    #[serde(rename = "crate")]
    pub crate_name: String,
}
#[derive(Serialize)]
#[serde(rename = "glob")]
pub struct Glob(pub BTreeSet<String>);

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
