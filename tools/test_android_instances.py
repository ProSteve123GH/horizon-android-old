#!/usr/bin/env python3
"""Exercise instance naming, sizing and duplication without Android or JNI."""
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def contract():
    source = (ROOT / 'crates/horizon-client/src/android/instances.rs').read_text(
        encoding='utf-8'
    )
    # These are storage contracts, rather than implementation details: the
    # launcher may rename an instance but must not change its identity.
    assert 'pub const NAME_FILE: &str = "android-profile-name.txt";' in source
    assert 'pub const ID_FILE: &str = crate::shared_storage::PROFILE_ID_FILE;' in source
    assert 'pub fn profile_id(dir: &Path) -> Option<u32>' in source
    assert 'pub fn write_profile_id(dir: &Path, id: u32)' in source
    assert 'if id == 0 {' in source and '"Default".into()' in source
    assert 'pub fn canonical_path(home: &Path, id: u32, name: &str)' in source
    assert 'crate::shared_storage::canonical_profile_dir(home, id, name)' in source
    assert '/// The stored name, or a stable fallback' in source


def main():
    root = ROOT / '.local/android/instance-tests'
    (root / 'src').mkdir(parents=True, exist_ok=True)
    (root / 'Cargo.toml').write_text('''[package]
name = "android-instance-tests"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
serde_json = "1"
rustix = { version = "1", features = ["fs"] }
''', encoding='utf-8')
    (root / 'src/lib.rs').write_text('''#![allow(dead_code)]
mod backend {
    pub fn game_home() -> std::path::PathBuf {
        std::env::temp_dir().join("horizon-instance-harness")
    }
    pub fn installed() -> Vec<String> { Vec::new() }
}
#[path = "../../../../crates/horizon-client/src/android/shared_storage.rs"]
mod shared_storage;
#[path = "../../../../crates/horizon-client/src/android/instances.rs"]
mod instances;
''', encoding='utf-8')
    subprocess.run(['cargo', 'test', '--manifest-path', str(root / 'Cargo.toml')],
                   cwd=ROOT, check=True)
    contract()


if __name__ == '__main__':
    main()
