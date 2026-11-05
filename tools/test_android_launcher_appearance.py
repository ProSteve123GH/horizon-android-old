#!/usr/bin/env python3
"""Exercise the Android launcher's appearance store without Android or Compose."""
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    harness = ROOT / '.local/android/launcher-appearance-tests'
    (harness / 'src').mkdir(parents=True, exist_ok=True)
    (harness / 'Cargo.toml').write_text('''[package]
name = "android-launcher-appearance-tests"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
''', encoding='utf-8')
    (harness / 'src/lib.rs').write_text('''#![allow(dead_code)]
#[path = "../../../../crates/horizon-client/src/android/appearance.rs"]
mod appearance;
''', encoding='utf-8')
    subprocess.run(
        ['cargo', 'test', '--manifest-path', str(harness / 'Cargo.toml')],
        cwd=ROOT,
        check=True,
    )


if __name__ == '__main__':
    main()
