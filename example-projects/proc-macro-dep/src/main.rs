#[derive(serde::Serialize)]
struct Foo {
    bar: String,
}

fn main() {
    println!(
        "Hello json: {}",
        serde_json::to_string(&Foo {
            bar: "baz".to_string()
        })
        .unwrap()
    );
}
