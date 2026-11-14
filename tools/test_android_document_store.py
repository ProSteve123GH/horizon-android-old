#!/usr/bin/env python3
"""Exercise the SAF document tree and its write flags without Android or JNI."""
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    root = ROOT / '.local/android/document-store-tests'
    (root / 'src').mkdir(parents=True, exist_ok=True)
    (root / 'Cargo.toml').write_text('''[package]
name = "android-document-store-tests"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
serde_json = "1"
rustix = { version = "1", features = ["fs"] }
''', encoding='utf-8')
    (root / 'src/lib.rs').write_text('''#![allow(dead_code)]
#[path = "../../../../crates/horizon-client/src/android/document_store.rs"]
pub mod document_store;
''', encoding='utf-8')
    subprocess.run(['cargo', 'test', '--manifest-path', str(root / 'Cargo.toml')],
                   cwd=ROOT, check=True)


if __name__ == '__main__':
    main()
