def _exec_test_impl(ctx):
    # Executable from the dep
    run = ctx.attrs.bin[RunInfo]

    # Build argv as a list; the first element is the executable, then extra args
    argv = [cmd_args(run)] + ctx.attrs.args

    return [
        DefaultInfo(),
        ExternalRunnerTestInfo(
            type = "custom",
            command = argv,        # must be list/tuple
            env = ctx.attrs.env,   # optional
        ),
    ]

exec_test = rule(
    impl = _exec_test_impl,
    attrs = {
        "bin": attrs.dep(providers = [RunInfo]),
        "args": attrs.list(attrs.arg(), default = []),
        "env": attrs.dict(attrs.string(), attrs.string(), default = {}),
    }
)