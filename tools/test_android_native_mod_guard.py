#!/usr/bin/env python3
"""Host test for the Android native-mod Compile id guard.

Includes the real `native_mods.rs` logic in a tiny host crate with stubs for the
Android-only JNI/document/platform bridge. No Android SDK and no game: the test
drives the id-filtering rule that decides whether Compile starts a build.
"""
from pathlib import Path
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def main():
    root = ROOT / '.local/android/native-mod-guard-tests'
    (root / 'src').mkdir(parents=True, exist_ok=True)
    (root / 'Cargo.toml').write_text('''[package]
name = "android-native-mod-guard-tests"
version = "0.0.0"
edition = "2024"
[workspace]
exclude = ["jni-stub"]
[dependencies]
serde_json = "1"
horizon-mods = { path = "../../../crates/horizon-mods", default-features = false, features = ["index"] }
sha2 = "0.10"
ureq = { version = "2", default-features = false, features = ["tls"] }
zip = { version = "9", default-features = false, features = ["deflate"] }
jni = { path = "jni-stub" }
''', encoding='utf-8')
    # `jni` is an external crate in the real game; stub it as one so the
    # `jni::…` paths in the included file resolve exactly as they do in prod.
    stub = root / 'jni-stub'
    stub.mkdir(exist_ok=True)
    (stub / 'Cargo.toml').write_text('''[package]
name = "jni"
version = "0.0.0"
edition = "2024"
[workspace]
''', encoding='utf-8')
    (stub / 'src').mkdir(exist_ok=True)
    (stub / 'src/lib.rs').write_text('''#![allow(dead_code)]
pub mod objects {
    pub struct JObject<'a>(pub &'a ());
    pub struct GlobalRef(pub JObject<'static>);
    impl GlobalRef {
        pub fn as_obj(&self) -> &JObject<'_> { &self.0 }
    }
}
pub mod errors {
    pub type Result<T> = std::result::Result<T, String>;
}
pub struct JavaVM;
impl JavaVM {
    pub fn attach_current_thread(&self) -> errors::Result<JNIEnv<'_>> { Ok(JNIEnv(&())) }
}
pub struct JNIEnv<'a>(&'a ());
impl JNIEnv<'_> {
    pub fn new_global_ref(&mut self, _: &objects::JObject<'_>) -> errors::Result<objects::GlobalRef> {
        Ok(objects::GlobalRef(objects::JObject(&())))
    }
    pub fn get_java_vm(&mut self) -> errors::Result<JavaVM> { Ok(JavaVM) }
    pub fn exception_clear(&mut self) {}
}
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

    source = (ROOT / 'crates/horizon-client/src/android/native_mods.rs').as_posix()
    (root / 'src/lib.rs').write_text(f'''#![allow(dead_code)]
#![allow(unused_imports)]

// The Android-only surface the module reaches through `crate::`. None of it
// participates in the id rule under test; it only lets the real file compile.
pub mod documents {{
    pub fn busy() -> bool {{ false }}
    pub fn begin_import() {{}}
    pub fn note(_: &str, _: bool) {{}}
    pub fn set_working(_: bool) {{}}
    pub fn cancel_requested() -> bool {{ false }}
}}
pub mod document_bridge {{
    use jni::{{objects::JObject, JNIEnv}};
    pub fn open(
        _: &mut JNIEnv<'_>,
        _: &JObject<'_>,
        _: &str,
        _: &str,
    ) -> Result<(std::fs::File, String), String> {{
        Err("stub: no document".to_owned())
    }}
}}
pub mod platform {{
    pub fn cache_dir() -> Option<std::path::PathBuf> {{ None }}
}}

#[path = "{source}"]
pub mod native_mods;

#[cfg(test)]
mod guard_tests {{
    use super::native_mods::selected_ids;
    use serde_json::json;

    #[test]
    fn all_invalid_ids_are_rejected() {{
        // Independent expected values: every entry breaks `valid_id`'s
        // [a-z0-9]{{1,64}} rule (case, separators, empty, too long).
        let request = json!({{
            "enabled": ["Bad-ID", "has space", "a/b", "", "x".repeat(65)]
        }});
        assert_eq!(selected_ids(&request), None);
    }}

    #[test]
    fn valid_ids_are_still_accepted() {{
        let request = json!({{
            "enabled": ["vein_miner", "alpha", "BAD", "vein_miner", "z9"]
        }});
        assert_eq!(
            selected_ids(&request),
            Some(vec!["alpha".to_owned(), "vein_miner".to_owned(), "z9".to_owned()])
        );
    }}

    #[test]
    fn missing_enabled_array_is_rejected() {{
        assert_eq!(selected_ids(&json!({{}})), None);
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
