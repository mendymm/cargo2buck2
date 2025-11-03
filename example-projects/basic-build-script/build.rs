fn main() {
    // Pick a value (from your shell env or a default)
    let value = env!("CARGO_CRATE_NAME");

    // Set the compile-time env var
    println!(
        "cargo:rustc-env=MY_VAR={}",
        format!("{value}-{}", env!("CARGO_PKG_VERSION"))
    );
}
