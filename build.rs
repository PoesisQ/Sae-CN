fn main() {
    #[cfg(feature = "native-ui")]
    {
        slint_build::compile("ui/reader.slint").expect("Slint UI compilation failed");
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            windows_resources();
        }
    }
}

#[cfg(feature = "native-ui")]
fn windows_resources() {
    use std::{env, path::PathBuf, process::Command};

    println!("cargo:rerun-if-changed=assets/sae-reader.rc");
    println!("cargo:rerun-if-changed=assets/app-icon.ico");
    println!("cargo:rerun-if-env-changed=RC");

    // RC is part of the Windows SDK already required for this MSVC project.
    // Locate it without adding a runtime or a new build dependency.
    let compiler = env::var_os("RC")
        .map(PathBuf::from)
        .or_else(|| {
            let sdk_bin =
                PathBuf::from(env::var_os("ProgramFiles(x86)")?).join("Windows Kits/10/bin");
            let mut versions: Vec<_> = std::fs::read_dir(sdk_bin)
                .ok()?
                .filter_map(Result::ok)
                .map(|entry| entry.path().join("x64/rc.exe"))
                .filter(|path| path.is_file())
                .collect();
            versions.sort();
            versions.pop()
        })
        .unwrap_or_else(|| PathBuf::from("rc.exe"));
    let output =
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR missing")).join("sae-reader.res");
    let status = Command::new(&compiler)
        .arg("/nologo")
        .arg("/fo")
        .arg(&output)
        .arg("assets/sae-reader.rc")
        .current_dir(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory missing"))
        .status()
        .unwrap_or_else(|error| panic!("Windows SDK RC failed ({}): {error}", compiler.display()));
    assert!(status.success(), "Windows icon/resource compilation failed");
    println!("cargo:rustc-link-arg-bin=sae-reader={}", output.display());
}
