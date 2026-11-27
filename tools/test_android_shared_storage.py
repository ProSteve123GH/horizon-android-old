#!/usr/bin/env python3
"""Exercise the game-files migration between app storage and shared storage without Android."""
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def contract():
    source = (ROOT / 'crates/horizon-client/src/android/shared_storage.rs').read_text(
        encoding='utf-8'
    )
    # Keep the on-disk layout explicit.  In particular, IDs must survive a
    # folder rename and old profile-* folders must remain readable/migratable.
    for line in (
        'pub const INSTANCES_DIR: &str = "Instances";',
        'pub const DEFAULT_INSTANCE_NAME: &str = "Default";',
        'pub const PROFILE_ID_FILE: &str = "android-profile-id.txt";',
        'pub const PROFILE_NAME_FILE: &str = "android-profile-name.txt";',
        'pub const CONTROLS_DIR: &str = "Controls";',
        'const LEGACY_CONTROLS_FILE: &str = "android-touch-layouts.json";',
    ):
        assert line in source, f'missing storage contract: {line}'
    for symbol in (
        'pub fn discover_profile_path(root: &Path, id: u32)',
        'pub fn read_profile_id(dir: &Path) -> Option<u32>',
        'pub fn write_profile_id(dir: &Path, id: u32)',
        'pub fn migrate_layout(home: &Path) -> Result<String, String>',
    ):
        assert symbol in source, f'missing storage API: {symbol}'
    assert 'Keep old installs readable until migrate_layout is called.' in source
    assert 'let legacy = root.join("profiles");' in source
    assert 'folder.strip_prefix("profile-")' in source
    assert 'write_profile_id(&default, 0)' in source
    assert 'format!("p{stable_id}")' in source


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
    subprocess.run(['cargo', 'test', '--manifest-path', str(harness / 'Cargo.toml'),
                    '--', '--test-threads=1'],
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
