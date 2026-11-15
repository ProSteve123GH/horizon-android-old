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
getrandom = "0.3"
horizon-threads = { path = "../../../crates/horizon-threads" }
horizon-server-files = { path = "../../../crates/horizon-server-files" }
''', encoding='utf-8')
    (harness / 'src/lib.rs').write_text('''#![allow(dead_code)]
extern crate self as horizon_client;
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
// The real block_entity_render.rs transitively imports the entire engine; the
// harness needs only the small stats struct control.rs reads.
pub mod block_entity_render {
    #[derive(Clone, Copy, Debug, Default)]
    pub struct StaticCacheFrameStats {
        pub snapshot_rebuilds: u64,
        pub snapshot_bytes_built: u64,
        pub snapshot_revision: u64,
        pub static_bytes_uploaded: u64,
        pub static_entities_drawn: u64,
        pub dynamic_entities_drawn: u64,
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
