load("@prelude//cxx:cxx_context.bzl", "get_cxx_toolchain_info")
load("@prelude//decls/toolchains_common.bzl", "toolchains_common")
load("@prelude//rust:rust_toolchain.bzl", "PanicRuntime", "RustToolchainInfo")
# load("@prelude//rust/tools:attrs.bzl", "internal_tool_attrs")

_DEFAULT_TRIPLE = select({
    "config//os:linux": select({
        "config//cpu:arm64": "aarch64-unknown-linux-gnu",
        "config//cpu:x86_64": "x86_64-unknown-linux-gnu",
    }),
    "config//os:macos": select({
        "config//cpu:arm64": "aarch64-apple-darwin",
        "config//cpu:x86_64": "x86_64-apple-darwin",
    }),
    "config//os:windows": select({
        "config//cpu:arm64": select({
            "DEFAULT": "aarch64-pc-windows-msvc",
            "config//abi:gnu": "aarch64-pc-windows-gnu",
            "config//abi:msvc": "aarch64-pc-windows-msvc",
        }),
        "config//cpu:x86_64": select({
            "DEFAULT": "x86_64-pc-windows-msvc",
            "config//abi:gnu": "x86_64-pc-windows-gnu",
            "config//abi:msvc": "x86_64-pc-windows-msvc",
        }),
    }),
})

def _vendored_rust_toolchain_impl(ctx):
    triple = ctx.attrs.rustc_target_triple
    rustc_download = ctx.attrs.toolchain[DefaultInfo].default_outputs[0]

    linker_info = get_cxx_toolchain_info(ctx).linker_info
    binary_extension = ".exe" if linker_info.binary_extension == "exe" else ""

    clippy_driver = rustc_download.project(f'clippy-preview/bin/clippy-driver{binary_extension}')
    rustc = rustc_download.project(f'rustc/bin/rustc{binary_extension}')
    rustdoc = rustc_download.project(f'rustc/bin/rustdoc{binary_extension}')
    cargo = rustc_download.project(f'cargo/bin/cargo{binary_extension}')
    sysroot_path = rustc_download.project(f'rust-std-{triple}')

    return [
        DefaultInfo(
            default_output = rustc_download,
            sub_targets = {
                'rustc': [ DefaultInfo( default_output = rustc ), RunInfo( args = cmd_args([rustc]) ) ],
                'rustdoc': [ DefaultInfo( default_output = rustdoc ), RunInfo( args = cmd_args([rustdoc]) ) ],
                'cargo': [ DefaultInfo( default_output = cargo ), RunInfo( args = cmd_args([cargo]) ) ],
            }
        ),
        RustToolchainInfo(
            allow_lints = ctx.attrs.allow_lints,
            clippy_driver = RunInfo(args = cmd_args([clippy_driver])),
            clippy_toml = ctx.attrs.clippy_toml[DefaultInfo].default_outputs[0] if ctx.attrs.clippy_toml else None,
            compiler = RunInfo(args = cmd_args([rustc])),
            default_edition = ctx.attrs.default_edition,
            panic_runtime = PanicRuntime(ctx.attrs.panic_runtime),
            deny_lints = ctx.attrs.deny_lints,
            doctests = ctx.attrs.doctests,
            # failure_filter_action = ctx.attrs.failure_filter_action[RunInfo],
            report_unused_deps = ctx.attrs.report_unused_deps,
            # rustc_action = ctx.attrs.rustc_action[RunInfo],
            rustc_binary_flags = ctx.attrs.rustc_binary_flags,
            rustc_flags = ctx.attrs.rustc_flags,
            rustc_target_triple = ctx.attrs.rustc_target_triple,
            rustc_test_flags = ctx.attrs.rustc_test_flags,
            rustdoc = RunInfo(args = cmd_args([rustdoc])),
            rustdoc_flags = ctx.attrs.rustdoc_flags,
            # rustdoc_test_with_resources = ctx.attrs.rustdoc_test_with_resources[RunInfo],
            # rustdoc_coverage = ctx.attrs.rustdoc_coverage[RunInfo],
            sysroot_path = sysroot_path,
            # transitive_dependency_symlinks_tool = ctx.attrs.transitive_dependency_symlinks_tool[RunInfo],
            warn_lints = ctx.attrs.warn_lints,
        ),
    ]

vendored_rust_toolchain = rule(
    impl = _vendored_rust_toolchain_impl,
    attrs =  {
        "toolchain": attrs.exec_dep(),
        "panic_runtime": attrs.enum(PanicRuntime.values(), default = "unwind"),
        "allow_lints": attrs.list(attrs.string(), default = []),
        "clippy_toml": attrs.option(attrs.dep(providers = [DefaultInfo]), default = None),
        "default_edition": attrs.option(attrs.string(), default = None),
        "deny_lints": attrs.list(attrs.string(), default = []),
        "doctests": attrs.bool(default = False),
        "report_unused_deps": attrs.bool(default = False),
        "rustc_binary_flags": attrs.list(attrs.string(), default = []),
        "rustc_flags": attrs.list(attrs.string(), default = []),
        "rustc_target_triple": attrs.string(default = _DEFAULT_TRIPLE),
        "rustc_test_flags": attrs.list(attrs.string(), default = []),
        "rustdoc_flags": attrs.list(attrs.string(), default = []),
        "warn_lints": attrs.list(attrs.string(), default = []),

        "_cxx_toolchain": toolchains_common.cxx(),
    },
    is_toolchain_rule = True,
)
