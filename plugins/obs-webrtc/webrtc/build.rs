use std::{env, path::PathBuf};

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let bindings_dir = env::var("OBS_WEBRTC_GENERATED_DIR").ok();

    let config = cbindgen::Config {
        language: cbindgen::Language::C,
        ..Default::default()
    };

    match cbindgen::generate_with_config(crate_dir, config) {
        Ok(bindings) => {
            if let Some(bindings_dir) = bindings_dir {
                let bindings_dir: PathBuf = bindings_dir.into();
                bindings.write_to_file(bindings_dir.join("bindings.h"));
            }
        }
        Err(cbindgen::Error::ParseSyntaxError { .. }) => (), // ignore in favor of cargo's syntax check
        Err(err) => panic!("{:?}", err),
    };
}
