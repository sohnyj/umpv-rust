fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        println!("cargo:rerun-if-changed=res/umpv.rc");
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let output = format!("{}/umpv.res.o", out_dir);
        let status = std::process::Command::new("x86_64-w64-mingw32-windres")
            .args(["res/umpv.rc", "-o", &output])
            .status()
            .expect("failed to run windres");
        assert!(status.success(), "windres failed");
        println!("cargo:rustc-link-arg={}", output);
    }
}
