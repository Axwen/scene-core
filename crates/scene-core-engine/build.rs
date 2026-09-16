fn main() {
    let target = std::env::var("TARGET").expect("TARGET is set by cargo");
    println!("cargo:rustc-env=SCENE_CORE_TARGET={target}");
    println!("cargo:rerun-if-changed=build.rs");
}
