# [WIP] cargo2buck2

A very much WIP tool that let's you use buck2 to build your rust projects without needing to write your own rules.

As a general rule, users should not need to care about the generated `BUCK` files, and their build cacheability should automagically improve!

The main goals for this project are
- Automatically generate BUILD files from existing `Cargo.toml`/`Cargo.lock` files.
- Treat `Cargo.toml` as the source of truth
- Resolve deps/features in the *same* way that cargo does (we use the `cargo` crate for this)
- Add new optional enhancements to the build process
- Provide many pre-build "fixups" to allow different crates to compile correctly (`zstd-sys`, `libsqlite3_sys`, `aws_lc_sys`, etc...)
- In general if a crate does not compile, we will try to add a "fixup" for it.


## Planed enhancments

- [ ] Ability to mark a proc-macros as "sandboxed", so we won't need to re-run it if the inputs did not change.
- [ ] Ability to explicitly name inputs to a build-script (files/environment variables) so we don't need to re-run it if the inputs did not change



## Progress


- [x] [Simple no dependency bin](./example-projects/simple-no-deps-bin/)
- [x] [Simple single dependency bin](./example-projects/simple-single-dep-bin/)
- [x] [Simple no dependency build-script](./example-projects/bin-with-build-rs/)
- [x] [Proc macro dependency bin](./example-projects/proc-macro-dep/)
- [ ] Simple workspace


## Acknowledgements
- dtolnay: for all your work on rust, the ecosystem, buck2 and reindeer. Thank you!❤️