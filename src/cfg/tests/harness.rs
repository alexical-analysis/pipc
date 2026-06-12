use std::path::PathBuf;

use crate::backend;
use crate::ctx::ctx::GlobalCtx;

pub fn run_pipeline(ctx: &GlobalCtx) -> Vec<u16> {
    backend::compile(ctx)
}

pub fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/cfg/tests/golden")
        .join(format!("{}.hex", name))
}

pub fn render_hex(rom: &[u16]) -> String {
    let mut out = String::new();
    for (i, chunk) in rom.chunks(8).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|w| format!("{:04X}", w)).collect();
        out.push_str(&format!("{:04X}: {}\n", i * 8, hex.join(" ")));
    }
    out
}

pub fn compare_or_update(name: &str, rom: &[u16]) {
    let path = golden_path(name);
    let actual = render_hex(rom);

    if std::env::var("UPDATE_GOLDEN").is_ok() {
        std::fs::write(&path, &actual).expect("failed to write golden");
        return;
    }

    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "golden file missing: {}. Run UPDATE_GOLDEN=1 cargo test to create it.",
            path.display()
        )
    });

    assert_eq!(actual, expected, "ROM mismatch for test '{}'", name);
}
