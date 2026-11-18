#!/usr/bin/env python3
"""Test the real Android recorder/timers in a host harness, without player data."""
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    harness = ROOT / '.local/android/performance-tests'
    (harness / 'src').mkdir(parents=True, exist_ok=True)
    (harness / 'Cargo.toml').write_text('''[package]
name = "android-performance-tests"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
serde_json = "1"
log = "0.4"
rustix = { version = "1", features = ["time"] }
horizon-threads = { path = "../../../crates/horizon-threads" }
''', encoding='utf-8')
    (harness / 'src/lib.rs').write_text('''#![allow(dead_code)]
extern crate self as horizon_client;
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
// Stubs for engine modules that only exist in the full game crate.
pub mod block_entity_render {
    #[derive(Default)]
    pub struct StaticCacheFrameStats {
        pub snapshot_rebuilds: u64,
        pub snapshot_bytes_built: u64,
        pub snapshot_revision: u64,
        pub static_bytes_uploaded: u64,
        pub static_entities_drawn: u64,
        pub dynamic_entities_drawn: u64,
    }
}
mod getrandom {
    pub fn fill(dest: &mut [u8]) -> std::io::Result<()> {
        use std::io::Read;
        std::fs::File::open("/dev/urandom")?.read_exact(dest)
    }
}
mod horizon_server_files {
    pub mod private_file {
        pub fn write_private(path: &std::path::Path, content: String) -> std::io::Result<()> {
            std::fs::write(path, content)
        }
    }
}
mod shared_storage {
    pub fn reports_dir(home: &std::path::Path) -> std::path::PathBuf {
        home.join("reports")
    }
}
#[path = "../../../../crates/horizon-client/src/control.rs"] pub mod control;
#[path = "../../../../crates/horizon-client-render/src/spans.rs"] pub mod spans;
mod backend { pub const VERSION: &str = "0.7.2-test"; }
mod platform {
    pub fn open_documents(_: &str) -> bool { true }
}
#[path = "../../../../crates/horizon-client/src/android/performance.rs"] mod performance;
#[path = "../../../../crates/horizon-client/src/android/device_math.rs"] mod device_math;
''', encoding='utf-8')
    subprocess.run(['cargo', 'test', '--manifest-path', str(harness / 'Cargo.toml'),
                    '--', '--test-threads=1'], cwd=ROOT, check=True)


if __name__ == '__main__':
    main()
