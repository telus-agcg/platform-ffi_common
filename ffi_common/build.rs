use cbindgen::{Builder, Language};
use std::env;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    Builder::new()
        .with_crate(crate_dir)
        .with_language(Language::C)
        .with_parse_expand(&["ffi_common"])
        .generate()
        .map(|bindings| bindings.write_to_file("bindings.h"))
        .unwrap_or_else(|_| {
            eprintln!("Unable to generate bindings");
            false
        });
}
