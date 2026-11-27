#!/usr/bin/env python3
"""Exercise the launcher's touch layout profiles without Android, JNI or the engine."""
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def contract():
    storage = (ROOT / 'crates/horizon-client/src/android/shared_storage.rs').read_text(
        encoding='utf-8'
    )
    source = (ROOT / 'crates/horizon-client/src/android/touch_layouts.rs').read_text(
        encoding='utf-8'
    )
    # Layout profiles are launcher metadata, kept outside the renamed game
    # folder and partitioned by the stable instance ID.
    assert 'pub const CONTROLS_DIR: &str = "Controls";' in storage
    assert 'pub fn controls_profile_dir(home: &Path, stable_id: u32) -> PathBuf' in storage
    assert '.join(format!("p{stable_id}"))' in storage
    assert 'pub const STORE_FILE: &str = "android-touch-layouts.json";' in source
    assert 'pub const TOUCH_FILE: &str = "horizon-touch.json";' in source


def main():
    root = ROOT / '.local/android/touch-layout-tests'
    (root / 'src').mkdir(parents=True, exist_ok=True)
    (root / 'Cargo.toml').write_text('''[package]
name = "android-touch-layout-tests"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
''', encoding='utf-8')
    (root / 'src/lib.rs').write_text('''#![allow(dead_code)]
#[path = "../../../../crates/horizon-client/src/android/touch_layouts.rs"]
mod touch_layouts;
''', encoding='utf-8')
    # The profile store lives beside the instance's saves and touch settings.
    store = (ROOT / 'crates/horizon-client/src/android/touch_layouts.rs').read_text(encoding='utf-8')
    for line in ('pub const STORE_FILE: &str = "android-touch-layouts.json";',
                 'pub const TOUCH_FILE: &str = "horizon-touch.json";'):
        assert line in store, f'missing contract: {line}'
    subprocess.run(['cargo', 'test', '--manifest-path', str(root / 'Cargo.toml')],
                   cwd=ROOT, check=True)
    contract()


if __name__ == '__main__':
    main()
