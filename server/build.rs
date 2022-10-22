use std::env;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=LIB_TYPE");
    let mut lib_type = env::var("LIB_TYPE").unwrap_or("dylib".to_string());

    let path = env::var("LIB_DIR").unwrap_or("target/debug".to_string());
    let lib_name = env::var("LIB_NAME").unwrap_or("jequi_go".to_string());

    if lib_type != "static" {
        lib_type = "dylib".to_string()
    }

    let output = Command::new("make")
        .arg("-C")
        .arg("../")
        .arg(&lib_type)
        .output()
        .expect("failed to execute process");

    if !output.status.success() {
        panic!("{}", String::from_utf8_lossy(&output.stderr))
    }

    println!("cargo:rustc-link-search={}", path);
    println!("cargo:rustc-link-lib={}={}", lib_type, lib_name);
    if lib_type == "dylib" {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", path);
    }
}
