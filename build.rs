use cmake::Config;
use std::env;
use std::path::PathBuf;

fn main() {
    let out = Config::new("libisyntax/").build_target("isyntax").build();

    println!("cargo:rustc-link-search=native={}/build", out.display());
    println!("cargo:rustc-link-lib=static=isyntax");

    let bindings = bindgen::Builder::default()
        .header("libisyntax/src/libisyntax.h")
        .must_use_type("isyntax_error_t")
        .new_type_alias("isyntax_error_t")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
