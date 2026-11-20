#!/usr/bin/env python3
"""Host test for unified `.hmod` (content + native code) routing.

Includes the real `content_files.rs` archive rule in a tiny host crate with the
single crate-root alias it needs as a stub. `catalog.rs` is deliberately not
included: it drags in the network/browser modules. The launcher's native-vs-
content decision is instead exercised with an independent reference classifier
beside the real `horizon_mods::index::native::{holds, Bundle}`. No Android SDK
and no game binary: the test drives the decision from archive bytes alone.
"""
from pathlib import Path
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def main():
    root = ROOT / '.local/android/unified-hmod-tests'
    (root / 'src').mkdir(parents=True, exist_ok=True)
    (root / 'Cargo.toml').write_text('''[package]
name = "android-unified-hmod-tests"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
serde_json = "1"
horizon-mods = { path = "../../../crates/horizon-mods", default-features = false, features = ["index"] }
zip = { version = "9", default-features = false, features = ["deflate"] }
''', encoding='utf-8')
    lock = tomllib.loads((ROOT / 'Cargo.lock').read_text(encoding='utf-8'))
    zip_version = next(
        package['version'] for package in lock['package']
        if package['name'] == 'zip' and package['version'].startswith('9.')
    )
    manifest = root / 'Cargo.toml'
    manifest.write_text(
        manifest.read_text(encoding='utf-8').replace(
            'zip = { version = "9"',
            f'zip = {{ version = "={zip_version}"',
        ),
        encoding='utf-8',
    )

    source = (ROOT / 'crates/horizon-client/src/content_files.rs').as_posix()
    (root / 'src/lib.rs').write_text(f'''#![allow(dead_code)]
#![allow(unused_imports)]

use std::{{io::Read, path::Path}};

// The only `crate::` name the real archive rule uses. `?` on zip/JSON/IO
// errors needs the game's boxed-error alias, so stub that exact type.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[path = "{source}"]
pub mod content_files;

const NATIVE_MANIFEST: &str = "horizon-native.json";

/// Independent, deliberately tiny stand-in for the launcher's pure routing
/// question: does this ZIP carry native code? It opens the archive and reads
/// the manifest; the file suffix is never consulted. A native manifest that
/// does not parse is not a native mod either.
fn reference_is_native(path: &Path) -> bool {{
    let Ok(file) = std::fs::File::open(path) else {{ return false; }};
    let Ok(mut zip) = zip::ZipArchive::new(file) else {{ return false; }};
    let Ok(mut entry) = zip.by_name(NATIVE_MANIFEST) else {{ return false; }};
    let mut bytes = Vec::new();
    if entry.read_to_end(&mut bytes).is_err() {{
        return false;
    }}
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .and_then(|v| v.get("schema").and_then(serde_json::Value::as_u64))
        == Some(1)
}}

#[cfg(test)]
mod routing_tests {{
    use super::*;
    use horizon_mods::index::native::{{Bundle, holds}};
    use std::{{io::Write, path::PathBuf}};

    // Expected values independent of native.rs's `veinminer` fixtures.
    const ID: &str = "gravel_pick";
    const VERSION: &str = "2.1.0";
    const HORIZON: &str = "dddddddddddddddddddddddddddddddddddddddd";
    const MIXINS: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    const CONTENT_TOML: &[u8] = b"id = \\"gravel_pick\\"\\nname = \\"Gravel Pick\\"\\nversion = \\"2.1.0\\"\\napi = \\"0.2\\"\\ncapabilities = [\\"registries\\"]\\n";
    const LANG: &[u8] = br#"{{"item.gravel_pick":"Gravel Pick"}}"#;

    fn native_manifest() -> String {{
        format!(
            r#"{{{{"schema":1,"id":"{{ID}}","version":"{{VERSION}}","game":"26.3","horizon":"{{HORIZON}}","mixins":"{{MIXINS}}"}}}}"#
        )
    }}
    fn mixin() -> String {{
        format!(
            "[mod]\\nid = \\"{{ID}}\\"\\nversion = \\"{{VERSION}}\\"\\n[compatibility]\\nhorizon_revision = \\"{{HORIZON}}\\"\\n"
        )
    }}
    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {{
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (name, bytes) in entries {{
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }}
        writer.finish().unwrap().into_inner()
    }}
    // One directory per test: cargo runs tests in parallel, so a cleanup must
    // never remove another test's staged archive.
    fn dir(test: &str) -> PathBuf {{
        std::env::temp_dir().join(format!("unified-hmod-tests-{{}}-{{test}}", std::process::id()))
    }}
    fn stage(test: &str, label: &str, extension: &str, bytes: &[u8]) -> PathBuf {{
        let dir = dir(test);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{{label}}.{{extension}}"));
        std::fs::write(&path, bytes).unwrap();
        path
    }}
    fn clean(test: &str) {{
        let _ = std::fs::remove_dir_all(dir(test));
    }}

    #[test]
    fn unified_hmod_is_native_by_contents_not_by_suffix() {{
        let native = native_manifest();
        let mixin = mixin();
        let bytes = archive(&[
            (NATIVE_MANIFEST, native.as_bytes()),
            ("mixins/gravel_pick/horizon-mixin.toml", mixin.as_bytes()),
            ("mixins/gravel_pick/hook.rs", b"fn hook() {{}}"),
            ("horizon-mod.toml", CONTENT_TOML),
            ("assets/gravel_pick/lang/en_us.json", LANG),
        ]);
        let hmod = stage("unified", "unified", "hmod", &bytes);
        let bin = stage("unified", "copy", "bin", &bytes);
        assert!(reference_is_native(&hmod), "a v1 native manifest is native");
        assert!(holds(&hmod), "real holds: a unified .hmod is native");
        assert!(reference_is_native(&bin) && holds(&bin), "the same bytes under a non-mod suffix stay native");
        assert!(
            content_files::validate(&hmod, content_files::Kind::Mods, "unified.hmod", || false).is_ok(),
            "the same archive is also a well-formed mod package"
        );
        let bundle = Bundle::read(&hmod).unwrap();
        assert_eq!(bundle.manifest.id, ID);
        let expected: std::collections::BTreeMap<String, Vec<u8>> = [
            ("horizon-mod.toml".to_owned(), CONTENT_TOML.to_vec()),
            ("assets/gravel_pick/lang/en_us.json".to_owned(), LANG.to_vec()),
        ]
        .into_iter()
        .collect();
        assert_eq!(bundle.content, expected);
        let plain = Bundle::read(&bin).unwrap();
        assert_eq!(plain.manifest.id, ID);
        assert!(plain.content.is_empty(), "only a .hmod unpacks content");
        clean("unified");
    }}

    #[test]
    fn legacy_hnmod_is_still_native() {{
        let native = native_manifest();
        let mixin = mixin();
        let bytes = archive(&[
            (NATIVE_MANIFEST, native.as_bytes()),
            ("mixins/gravel_pick/horizon-mixin.toml", mixin.as_bytes()),
            ("mixins/gravel_pick/hook.rs", b"fn hook() {{}}"),
        ]);
        let path = stage("legacy", "legacy", "hnmod", &bytes);
        assert!(reference_is_native(&path), "legacy code is detected too");
        assert!(holds(&path), "real holds: a legacy .hnmod is native");
        let bundle = Bundle::read(&path).unwrap();
        assert_eq!(bundle.manifest.id, ID);
        assert!(bundle.content.is_empty(), "a legacy bundle stays code-only");
        clean("legacy");
    }}

    #[test]
    fn content_only_hmod_is_not_native() {{
        let bytes = archive(&[
            ("horizon-mod.toml", CONTENT_TOML),
            ("assets/gravel_pick/lang/en_us.json", LANG),
        ]);
        let path = stage("content", "content-only", "hmod", &bytes);
        assert!(!reference_is_native(&path), "no native manifest in the ZIP");
        assert!(!holds(&path), "a content-only .hmod is not native code");
        assert!(
            content_files::validate(&path, content_files::Kind::Mods, "content-only.hmod", || false).is_ok(),
            "it is still a valid content mod"
        );
        assert!(Bundle::read(&path).is_err(), "there is no native bundle to read");
        clean("content");
    }}

    #[test]
    fn malformed_missing_and_non_zip_manifests_are_rejected() {{
        let mixin = mixin();
        // Valid ZIP, broken native manifest: the reference classifier and the
        // reader both reject it. (`holds` is only a presence probe.)
        let broken = stage(
            "malformed",
            "malformed",
            "hmod",
            &archive(&[
                (NATIVE_MANIFEST, b"not json"),
                ("mixins/gravel_pick/horizon-mixin.toml", mixin.as_bytes()),
            ]),
        );
        assert!(!reference_is_native(&broken));
        assert!(Bundle::read(&broken).is_err(), "a broken native manifest is refused");
        clean("malformed");

        // Valid content ZIP with no native manifest at all.
        let missing = stage(
            "missing",
            "missing",
            "hmod",
            &archive(&[("horizon-mod.toml", CONTENT_TOML), ("assets/gravel_pick/lang/en_us.json", LANG)]),
        );
        assert!(!reference_is_native(&missing));
        assert!(!holds(&missing));
        assert!(Bundle::read(&missing).is_err());
        clean("missing");

        // Not a ZIP at all.
        let not_zip = stage("notzip", "not-a-zip", "hmod", b"plain text, not an archive");
        assert!(!reference_is_native(&not_zip));
        assert!(!holds(&not_zip));
        clean("notzip");
    }}
}}
''', encoding='utf-8')
    subprocess.run(
        ['cargo', 'test', '--offline', '--manifest-path', str(root / 'Cargo.toml')],
        cwd=ROOT,
        check=True,
    )


if __name__ == '__main__':
    main()
