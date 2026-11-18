#!/usr/bin/env python3
"""Exercise the game-files migration between app storage and shared storage without Android."""
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def cargo_tests():
    harness = ROOT / '.local/android/shared-storage-tests'
    (harness / 'src').mkdir(parents=True, exist_ok=True)
    (harness / 'Cargo.toml').write_text('''[package]
name = "android-shared-storage-tests"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
serde_json = "1"
rustix = { version = "1", features = ["fs"] }
''', encoding='utf-8')
    (harness / 'src/lib.rs').write_text('''#![allow(dead_code)]
#[path = "../../../../crates/horizon-client/src/android/shared_storage.rs"]
pub mod shared_storage;
''', encoding='utf-8')
    subprocess.run(['cargo', 'test', '--manifest-path', str(harness / 'Cargo.toml')],
                   cwd=ROOT, check=True)


def contract():
    manifest = (ROOT / 'packaging/android/AndroidManifest.xml').read_text(encoding='utf-8')
    assert 'android.permission.MANAGE_EXTERNAL_STORAGE' in manifest, 'shared storage needs all-files access'
    assert 'android:requestLegacyExternalStorage="true"' in manifest, 'Android 10 needs the legacy opt-out'
    assert 'android.permission.WRITE_EXTERNAL_STORAGE' in manifest, 'Android 9 and 10 use the runtime permission'


def main():
    cargo_tests()
    contract()


if __name__ == '__main__':
    main()
