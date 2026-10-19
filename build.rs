fn main() {
    println!("cargo:rerun-if-changed=assets/windows/questlog.rc");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let o = format!("{}/res.o", std::env::var("OUT_DIR").unwrap());
        for tool in ["x86_64-w64-mingw32-windres", "windres"] {
            if let Ok(s) = std::process::Command::new(tool).args(["-i", "assets/windows/questlog.rc", "-O", "coff", "-o", &o]).status() {
                if s.success() { println!("cargo:rustc-link-arg-bins={o}"); break; }
            }
        }
    }
}
