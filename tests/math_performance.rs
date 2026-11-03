//! Explicit, windowless Windows core measurements, separate from UI acceptance.
#![cfg(windows)]
use sae_reader::{document::Book, settings::Typography};
use serde_json::json;
use std::{path::Path, time::Instant};

#[repr(C)]
#[derive(Default)]
struct Memory {
    cb: u32,
    faults: u32,
    peak_ws: usize,
    ws: usize,
    peak_paged: usize,
    paged: usize,
    peak_nonpaged: usize,
    nonpaged: usize,
    pagefile: usize,
    peak_pagefile: usize,
    private: usize,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> *mut std::ffi::c_void;
}
#[link(name = "psapi")]
unsafe extern "system" {
    fn GetProcessMemoryInfo(p: *mut std::ffi::c_void, m: *mut Memory, n: u32) -> i32;
}
fn private_bytes() -> usize {
    let mut memory = Memory {
        cb: std::mem::size_of::<Memory>() as u32,
        ..Default::default()
    };
    assert_ne!(
        unsafe {
            GetProcessMemoryInfo(
                GetCurrentProcess(),
                &mut memory,
                std::mem::size_of::<Memory>() as u32,
            )
        },
        0
    );
    memory.private
}

#[test]
#[ignore = "explicit native core benchmark; never creates a desktop window"]
fn headless_core_cost_and_repeated_open_release() {
    let output =
        std::env::var_os("SAE_MATH_BENCH_REPORT").expect("explicit report destination required");
    let mut records = Vec::new();
    for (alias, path) in [
        ("plain-markdown", "tests/fixtures/reading.md"),
        ("seven-formulas", "tests/fixtures/math.md"),
    ] {
        for run in 0..41 {
            let before = private_bytes();
            let start = Instant::now();
            let mut book = Book::open(Path::new(path)).unwrap();
            book.layout(&Typography::default(), 540.0, 700.0, 0)
                .unwrap();
            let opened = private_bytes();
            let page = book.engine.load_page(0).unwrap();
            let image = page
                .to_pixmap(
                    &mupdf::Matrix::new_scale(1.5, 1.5),
                    &mupdf::Colorspace::device_rgb(),
                    false,
                    true,
                )
                .unwrap();
            let ready_ms = start.elapsed().as_secs_f64() * 1000.0;
            let painted = private_bytes();
            assert_eq!(image.samples().len(), 810 * 1050 * 3);
            drop(image);
            drop(page);
            drop(book);
            records.push(json!({"sample":alias,"run":run,"open_layout_first_page_ms":ready_ms,"before_private":before,"opened_private":opened,"painted_private":painted,"closed_private":private_bytes()}));
        }
    }
    let report = json!({"measurement":"Windows native core, no Slint window; busy interactive host; warm iterations after run zero","records":&records});
    if let Some(parent) = Path::new(&output).parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    for sample in ["plain-markdown", "seven-formulas"] {
        let values = records
            .iter()
            .filter(|r| r["sample"] == sample && r["run"].as_u64().unwrap() >= 10)
            .map(|r| r["closed_private"].as_u64().unwrap())
            .collect::<Vec<_>>();
        assert!(
            values.last().unwrap().saturating_sub(values[0]) < 2 * 1024 * 1024,
            "closed documents retained native resources: {sample}"
        );
    }
}
