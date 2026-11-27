#!/usr/bin/env python3
"""Run archive safety and terrain upload scheduling tests without a game."""
from pathlib import Path
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def main():
    root = ROOT / '.local/android/content-tests'
    (root / 'src').mkdir(parents=True, exist_ok=True)
    stub = root / 'stubs' / 'horizon_core'
    (stub / 'src').mkdir(parents=True, exist_ok=True)
    (stub / 'Cargo.toml').write_text('''[package]
name = "horizon_core"
version = "0.0.0"
edition = "2024"
''', encoding='utf-8')
    (stub / 'src' / 'lib.rs').write_text('''pub mod prepared_files {
    pub const FILE_NAME: &str = "horizon-data.bin";
}
''', encoding='utf-8')
    (root / 'Cargo.toml').write_text('''[package]
name = "android-content-tests"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
horizon-mods = { path = "../../../crates/horizon-mods", default-features = false }
serde_json = "1"
rustix = { version = "1", features = ["fs"] }
sha2 = "0.10"
ureq = { version = "2", default-features = false, features = ["tls"] }
miniz_oxide = "0.8"
flate2 = { version = "1.1.10", default-features = false, features = ["zlib-rs", "runtime_detection"] }
zip = { version = "9", default-features = false, features = ["deflate"] }
horizon_core = { path = "stubs/horizon_core" }
''', encoding='utf-8')
    lock=tomllib.loads((ROOT / 'Cargo.lock').read_text(encoding='utf-8'))
    version=next(p['version'] for p in lock['package'] if p['name']=='zip' and p['version'].startswith('9.'))
    manifest=root / 'Cargo.toml'
    manifest.write_text(manifest.read_text().replace('zip = { version = "9"', f'zip = {{ version = "={version}"'), encoding='utf-8')
    (root / 'src/lib.rs').write_text('''#![allow(dead_code)]
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
mod osk { pub enum Output { Text(String), Backspace, Enter, Close } }
#[path = "../../../../crates/horizon-client/src/ime_edit.rs"] mod ime_edit;
#[path = "../../../../crates/horizon-client/src/content_files.rs"] mod content_files;
#[path = "../../../../crates/horizon-client/src/official_install_cache.rs"] mod official_install_cache;
#[path = "../../../../crates/horizon-client-render/src/gfx/upload_budget.rs"] mod upload_budget;
#[path = "../../../../crates/horizon-client-render/src/zip.rs"] mod archive_reader;
#[path = "../../../../crates/horizon-client/src/android/search_queue.rs"] mod search_queue;
#[path = "../../../../crates/horizon-client/src/android/shared_storage.rs"] mod shared_storage;
#[path = "../../../../crates/horizon-client/src/android/document_store.rs"] mod document_store;
#[path = "../../../../crates/horizon-client-render/src/graphics_startup.rs"] mod graphics_startup;
#[path = "../../../../crates/horizon-client/src/android_resources.rs"] mod android_resources;
#[path = "../../../../crates/horizon-client/src/ui_text.rs"] mod ui_text;
''', encoding='utf-8')
    subprocess.run(['cargo', 'test', '--manifest-path', str(root / 'Cargo.toml')],
                   cwd=ROOT, check=True)


if __name__ == '__main__':
    main()
