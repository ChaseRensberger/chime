fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("native/permissions.m")
            .flag("-fblocks")
            .compile("chime_permissions");
        for framework in [
            "ApplicationServices",
            "CoreGraphics",
            "CoreServices",
            "AVFoundation",
            "Foundation",
        ] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
        println!("cargo:rerun-if-changed=native/permissions.m");
    }
    tauri_build::build();
}
