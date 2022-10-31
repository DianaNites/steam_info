use std::{env, fs::File, path::PathBuf};

use gl_generator::{Api, Fallbacks, Profile, Registry, StructGenerator};

fn main() {
    let dest = PathBuf::from(&env::var("OUT_DIR").unwrap());

    println!("cargo:rerun-if-changed=build.rs");

    let mut file = File::create(&dest.join("gl_bindings.rs")).unwrap();
    Registry::new(
        Api::Gles2,
        (3, 0),
        Profile::Compatibility,
        Fallbacks::All,
        [],
    )
    .write_bindings(StructGenerator, &mut file)
    .unwrap();
}