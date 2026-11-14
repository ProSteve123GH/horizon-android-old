#!/usr/bin/env python3
"""Exercise instance naming, sizing and duplication without Android or JNI."""
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    root = ROOT / '.local/android/instance-tests'
    (root / 'src').mkdir(parents=True, exist_ok=True)
    (root / 'Cargo.toml').write_text('''[package]
name = "android-instance-tests"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
''', encoding='utf-8')
    (root / 'src/lib.rs').write_text('''#![allow(dead_code)]
#[path = "../../../../crates/horizon-client/src/android/instances.rs"]
mod instances;
''', encoding='utf-8')
    subprocess.run(['cargo', 'test', '--manifest-path', str(root / 'Cargo.toml')],
                   cwd=ROOT, check=True)


if __name__ == '__main__':
    main()
