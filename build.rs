fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let status = std::process::Command::new("x86_64-w64-mingw32-windres")
            .args(["res/umpv.rc", "-o"])
            .arg(format!("{}/umpv.res.o", std::env::var("OUT_DIR").unwrap()))
            .status()
            .expect("failed to run windres");
        assert!(status.success(), "windres failed");
        println!(
            "cargo:rustc-link-arg={}",
            format!("{}/umpv.res.o", std::env::var("OUT_DIR").unwrap())
        );
    }
}
