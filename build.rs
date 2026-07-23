fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        // ndk-sys declares Camera2 NDK symbols but does not link the library itself.
        println!("cargo:rustc-link-lib=camera2ndk");
    }
}
