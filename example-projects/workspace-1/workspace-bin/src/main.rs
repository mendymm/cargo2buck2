use heck::ToKebabCase as _;

fn main() {
    println!("{}", workspace_dep::run_from_dep());
    println!("{}", workspace_dep::run_from_dep().to_kebab_case());
}
