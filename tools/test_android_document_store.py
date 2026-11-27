#!/usr/bin/env python3
"""Exercise the SAF document tree and its write flags without Android or JNI."""
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def contract():
    source = (ROOT / 'crates/horizon-client/src/android/document_store.rs').read_text(
        encoding='utf-8'
    )
    # SAF IDs are public, stable handles.  They are profile-number based, not
    # display-name based, so renaming an instance cannot orphan granted URIs.
    assert 'pub fn folder_id(profile: u32, folder: &str) -> String' in source
    assert 'format!("p{profile}/{folder}")' in source
    assert 'if folder == "reports"' in source
    assert 'format!("p{profile}") != first' in source
    assert 'shared_storage::profile_dir(home, profile)' in source
    assert 'IDs are canonical and remain tied to their profile after switching.' in source
    assert 'shared-storage.json' in source
    assert 'path.is_absolute().then_some(path)' in source


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
#[path = "../../../../crates/horizon-client/src/android/shared_storage.rs"]
mod shared_storage;
#[path = "../../../../crates/horizon-client/src/android/document_store.rs"]
pub mod document_store;
''', encoding='utf-8')
    subprocess.run(['cargo', 'test', '--manifest-path', str(root / 'Cargo.toml')],
                   cwd=ROOT, check=True)
    contract()


if __name__ == '__main__':
    main()
