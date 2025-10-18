use std::{collections::BTreeSet, path::Path};

use camino::Utf8PathBuf;
use cargo::{
    GlobalContext,
    core::{
        PackageIdSpec, SourceId, Workspace,
        compiler::{CompileKind, RustcTargetData},
        resolver::CliFeatures,
    },
    ops::{OutputMetadataOptions, output_metadata, resolve_ws, resolve_ws_with_opts},
};
use serde::{Serialize, de};

fn main() {
    // buckify_workspace(Path::new(
    //     "/home/mendy/code/cargo2buck2/example-projects/simple-no-deps-bin",
    // ));
    buckify_workspace(Path::new(
        "/home/mendy/code/cargo2buck2/example-projects/simple-single-dep-bin",
    ));
}

fn buckify_workspace(ws_path: &Path) {
    let gctx = GlobalContext::default().unwrap();

    let ws = Workspace::new(&ws_path.join("Cargo.toml"), &gctx).unwrap();

    let mut target_data = RustcTargetData::new(&ws, &[CompileKind::Host]).unwrap();

    let cli_features = CliFeatures::from_command_line(&[], false, true).unwrap();

    let resolved = resolve_ws_with_opts(
        &ws,
        &mut target_data,
        &[CompileKind::Host],
        &cli_features,
        &[PackageIdSpec::new("simple-single-dep-bin".to_string())],
        cargo::core::resolver::HasDevUnits::Yes,
        cargo::core::resolver::ForceAllTargets::No,
        false,
    )
    .unwrap();

    let mut buck_file = BuckFile::new();

    let resolved_workspace = resolved.workspace_resolve.unwrap();

    for pkg in resolved.pkg_set.packages() {
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
                cargo::core::TargetKind::Lib(crate_types) => {
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
                    buck_file.add_rule(RustLibrary {
                        name: format!("{}-{}", pkg.name(), pkg.version()),
                        edition: target.edition().to_string(),
                        visibility: vec!["PUBLIC".to_string()],
                        srcs: vec![format!(":{}", pkg.package_id().tarball_name())],
                        crate_root: format!("{}/{}", pkg.package_id().tarball_name(), crate_root),
                        crate_name: pkg.name().to_string(),
                    });
                }
                cargo::core::TargetKind::Bin => {
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
                cargo::core::TargetKind::Test => todo!(),
                cargo::core::TargetKind::Bench => todo!(),
                cargo::core::TargetKind::ExampleLib(crate_types) => todo!(),
                cargo::core::TargetKind::ExampleBin => todo!(),
                cargo::core::TargetKind::CustomBuild => todo!(),
            }
        }

        // for dep in pkg.dependencies(){
        //     dep.
        // }
        // dbg!(i.summary().checksum());
        // dbg!(i.package_id().source_id());
        // // let source_id = i.source_id();
        // dbg!(i);
    }
    // dbg!(resolved);

    // dbg!();

    // let mut needed_deps: BTreeSet<SourceId> = BTreeSet::new();

    // for package in ws.members() {
    //     for target in package.targets() {
    //
    //     }
    // }

    // for dep in resolved_ws.iter() {
    //     dbg!(dep.source_id());
    // }

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
#[serde(untagged)]
enum BuckRule {
    RustBinary(RustBinary),
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
