use std::env;

fn main(){
    println!("cargo:rerun-if-env-changed=LIB_TYPE");
    let mut lib_type;

    match env::var("LIB_TYPE") {
        Ok(value) => lib_type = value,
        Err(_) => return 
    }

    let path = env::var("LIB_DIR").unwrap();
    let lib_name = env::var("LIB_NAME").unwrap();

    if lib_type != "static" {
        lib_type = "dylib".to_string()
    }

    println!("cargo:rustc-link-search={}", path);
    println!("cargo:rustc-link-lib={}={}",lib_type,lib_name);
    if lib_type == "dylib" {
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/../../{}",path);
    }
}
