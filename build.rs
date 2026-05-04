fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        println!("cargo:rerun-if-changed=res/mpv-icon.ico");
        println!("cargo:rerun-if-changed=res/umpv.rc");

        let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
        let out_dir = std::env::var("OUT_DIR").unwrap();

        if target_env == "msvc" {
            let res_output = format!("{}/umpv.res", out_dir);
            let status = std::process::Command::new("llvm-rc")
                .args(["/fo", &res_output, "res/umpv.rc"])
                .status()
                .expect("failed to run llvm-rc");
            assert!(status.success(), "llvm-rc failed");
            println!("cargo:rustc-link-arg={}", res_output);
        } else {
            let obj_output = format!("{}/umpv.res.o", out_dir);
            let status = std::process::Command::new("x86_64-w64-mingw32-windres")
                .args(["res/umpv.rc", "-o", &obj_output])
                .status()
                .expect("failed to run windres");
            assert!(status.success(), "windres failed");
            println!("cargo:rustc-link-arg={}", obj_output);
        }
    }
}
