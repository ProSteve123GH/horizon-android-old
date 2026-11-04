#!/usr/bin/env python3
"""Exercise world listing, renaming, copying and deletion without Android or JNI."""
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    root = ROOT / '.local/android/world-tests'
    (root / 'src').mkdir(parents=True, exist_ok=True)
    (root / 'Cargo.toml').write_text('''[package]
name = "android-world-tests"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
horizon-world = { path = "../../../crates/horizon-world" }
chrono = { version = "0.4.42", default-features = false, features = ["clock", "std"] }
flate2 = { version = "1.1.10", default-features = false, features = ["zlib-rs", "runtime_detection"] }
zip = { version = "9.1.3", default-features = false, features = ["deflate"] }
''', encoding='utf-8')
    (root / 'src/lib.rs').write_text('''#![allow(dead_code)]
mod desktop_seams {
    pub fn to_trash(path: &std::path::Path) -> std::io::Result<()> {
        std::fs::remove_dir_all(path)
    }
}
#[path = "../../../../launcher/src/worlds.rs"]
mod worlds;
''', encoding='utf-8')
    subprocess.run(['cargo', 'test', '--manifest-path', str(root / 'Cargo.toml')],
                   cwd=ROOT, check=True)


if __name__ == '__main__':
    main()
