fn main() {
    let def = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("version.def");
    println!("cargo:rerun-if-changed={}", def.display());

    let target = std::env::var("TARGET").unwrap_or_default();
    if !target.contains("windows") {
        return;
    }

    let def = def.to_string_lossy();
    if std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default() == "msvc" {
        println!("cargo:rustc-link-arg=/DEF:{def}");
    } else {
        println!("cargo:rustc-link-arg=-Wl,/{def}");
    }
}
