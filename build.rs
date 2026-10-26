fn main() {
    kindle_parser();
    #[cfg(feature = "native-ui")]
    {
        slint_build::compile("ui/reader.slint").expect("Slint UI compilation failed");
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            windows_resources();
        }
    }
}

fn kindle_parser() {
    // Reuse MuPDF's linked zlib. No converter process, DLL or second inflater.
    let mut parser = cc::Build::new();
    parser
        .std("c11")
        .include("vendor/libmobi/src")
        .include("native")
        .include("vendor/mupdf-sys-0.8.0/mupdf/thirdparty/zlib")
        .define("USE_XMLWRITER", "1")
        .define("USE_ZLIB", "1")
        .define("PACKAGE_VERSION", "\"0.12\"")
        .define("MOBI_INLINE", "inline")
        .define("_CRT_SECURE_NO_WARNINGS", None)
        .warnings(false);
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        parser.flag("/FIkindle_alloc.h");
    } else {
        parser.flag("-include").flag("kindle_alloc.h");
    }
    for source in [
        "buffer",
        "compression",
        "index",
        "memory",
        "meta",
        "parse_rawml",
        "read",
        "structure",
        "util",
        "opf",
        "xmlwriter",
    ] {
        parser.file(format!("vendor/libmobi/src/{source}.c"));
    }
    parser.file("native/kindle.c").compile("sae_kindle");
    cc::Build::new()
        .std("c11")
        .include("native")
        .file("native/kindle_alloc.c")
        .compile("sae_kindle_alloc");
    println!("cargo:rerun-if-changed=vendor/libmobi/src");
    println!("cargo:rerun-if-changed=native");
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
