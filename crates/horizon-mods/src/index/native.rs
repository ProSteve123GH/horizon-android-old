//! Source-only native mixin bundles. Reading a bundle never executes its code.
use super::{Facts, valid_id};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::{collections::BTreeMap, io::Read, path::Path};

pub const MANIFEST: &str = "horizon-native.json";

/// A `.hmod` that still carries a WebAssembly mod: the registry kept publishing those until the
/// native reader could read a `.hmod`'s content beside its sources.
pub const WASM_GONE: &str = "mod.wasm: WebAssembly mods don't run any more, port the code to a native mod (docs/native-mods.md)";

/// Whether the archive at `path` is a native bundle: a `horizon-native.json` at its root. The
/// cheap test that routes a `.hmod` carrying sources beside its content.
pub fn holds(path: &Path) -> bool {
    std::fs::File::open(path)
        .ok()
        .and_then(|f| zip::ZipArchive::new(f).ok())
        .is_some_and(|mut zip| zip.by_name(MANIFEST).is_ok())
}

/// Whether an entry `name` belongs to a `.hmod`'s content (`horizon-mod.toml`, `assets/`,
/// `data/`), which [`crate::Package`] reads, rather than to its native sources.
fn content(name: &str) -> bool {
    name == crate::manifest::MANIFEST || name.starts_with("assets/") || name.starts_with("data/")
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub id: String,
    pub version: String,
    pub game: String,
    /// Exact source revisions tested together, never mutable tags.
    pub horizon: String,
    pub mixins: String,
    /// The version of Horizon's native hook points (`game/native_hooks.rs`,
    /// `NATIVE_HOOKS_API`) a mod that hooks nothing else needs: the launcher
    /// builds it on any Horizon with that version or a later one, and
    /// `horizon` is only the revision it was tested on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks: Option<u32>,
}

pub fn revision(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Manifest {
    pub fn check(&self) -> Result<(), String> {
        if self.schema != 1
            || !valid_id(&self.id)
            || self.version.is_empty()
            || self.game.is_empty()
            || !revision(&self.horizon)
            || !revision(&self.mixins)
            || self.hooks == Some(0)
        {
            return Err(
                "invalid native manifest: schema 1, id, version, game and exact revisions required"
                    .into(),
            );
        }
        Ok(())
    }

    /// Carries the exact native compatibility contract in the index's API field.
    pub fn api(&self) -> String {
        format!("native-1/{}/{}", self.horizon, self.mixins)
    }

    pub fn facts(&self) -> Facts {
        Facts {
            id: Some(self.id.clone()),
            version: Some(self.version.clone()),
            api: self.api(),
            game: Some(self.game.clone()),
            ..Facts::default()
        }
    }
}

pub struct Bundle {
    pub manifest: Manifest,
    /// Validated relative paths: Rust and TOML, plus files under `mixins/<id>/data/` or a
    /// `data/` folder deeper in (a crate's).
    pub files: BTreeMap<String, Vec<u8>>,
}

/// Whether `rest` (a path inside `mixins/<id>/`) is a data file: under the bundle's `data/`, or
/// under the top-level `data/` of a folder of it (a crate's, `engine/data/`), not any folder
/// named `data` deeper in.
fn in_data(rest: &str) -> bool {
    let parts: Vec<&str> = rest.split('/').collect();
    parts.len() >= 2 && (parts[0] == "data" || (parts.len() >= 3 && parts[1] == "data"))
}

/// Whether `rest` is one of the text files a crate folder usually carries, which a crate may need
/// to compile (`#![doc = include_str!("../README.md")]`): `README*`, `LICENSE*`, `COPYING*`,
/// `NOTICE*` and `.gitignore`, in the bundle's folder or a top-level folder of it.
fn text(rest: &str) -> bool {
    let parts: Vec<&str> = rest.split('/').collect();
    let name = parts[parts.len() - 1];
    parts.len() <= 2
        && (["README", "LICENSE", "LICENCE", "COPYING", "NOTICE"]
            .iter()
            .any(|p| name.starts_with(p))
            || name == ".gitignore")
}

/// Whether a bundle may carry the file `rest` (its path under `mixins/<id>/`): sources and
/// manifests, data, and the text files of a crate folder.
pub fn allowed(rest: &str) -> bool {
    rest.ends_with(".rs") || rest.ends_with(".toml") || in_data(rest) || text(rest)
}

impl Bundle {
    pub fn read(path: &Path) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
        // Content entries are skipped below; the archive's whole list is still bounded, so a
        // bundle with a hundred thousand empty `assets/` entries cannot make reading it slow.
        if zip.len() > 4096 {
            return Err("native bundle has too many entries".into());
        }
        let mut files = BTreeMap::new();
        let mut content_id = None;
        let mut total = 0u64;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            let name = entry.name().map_err(|e| e.to_string())?.into_owned();
            if entry.is_dir() {
                continue;
            }
            if entry
                .unix_mode()
                .is_some_and(|m| m & 0o170000 != 0 && m & 0o170000 != 0o100000)
                || name.contains('\\')
                || name
                    .split('/')
                    .any(|p| p.is_empty() || p == "." || p == "..")
                || name.starts_with('/')
                || name.contains(':')
            {
                return Err(format!("invalid native bundle path: {name}"));
            }
            if name == crate::manifest::WASM {
                return Err(WASM_GONE.into());
            }
            // A `.hmod` may carry its content beside its sources: the content is the content
            // reader's, and only its id is checked here.
            if content(&name) {
                if name == crate::manifest::MANIFEST {
                    let mut text = String::new();
                    (&mut entry)
                        .take(1 << 20)
                        .read_to_string(&mut text)
                        .map_err(|e| format!("{name}: {e}"))?;
                    content_id = Some(crate::Manifest::parse(&text)?.id);
                }
                continue;
            }
            if files.len() >= 512 {
                return Err("native bundle has too many entries".into());
            }
            let mut bytes = Vec::new();
            (&mut entry)
                .take(super::MAX_FILE + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            total += bytes.len() as u64;
            if total > super::MAX_FILE {
                return Err("native bundle expands beyond 64 MiB".into());
            }
            if files.insert(name, bytes).is_some() {
                return Err("duplicate native bundle path".into());
            }
        }
        let manifest: Manifest =
            serde_json::from_slice(files.get(MANIFEST).ok_or("missing horizon-native.json")?)
                .map_err(|e| e.to_string())?;
        manifest.check()?;
        if let Some(id) = content_id.filter(|id| *id != manifest.id) {
            return Err(format!(
                "its {} is mod {id}, its {MANIFEST} {}: one file is one mod",
                crate::manifest::MANIFEST,
                manifest.id
            ));
        }
        let prefix = format!("mixins/{}/", manifest.id);
        for name in files.keys().filter(|n| n.as_str() != MANIFEST) {
            let Some(rest) = name.strip_prefix(&prefix) else {
                return Err(format!("unexpected native source: {name}"));
            };
            if !allowed(rest) {
                return Err(format!("unexpected native source: {name}"));
            }
        }
        let source = files
            .get(&format!("{prefix}horizon-mixin.toml"))
            .ok_or("missing horizon-mixin.toml")?;
        let source: toml::Value =
            toml::from_str(std::str::from_utf8(source).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if source
            .get("mod")
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())
            != Some(&manifest.id)
            || source
                .get("mod")
                .and_then(|v| v.get("version"))
                .and_then(|v| v.as_str())
                != Some(&manifest.version)
            || source
                .get("compatibility")
                .and_then(|v| v.get("horizon_revision"))
                .and_then(|v| v.as_str())
                != Some(&manifest.horizon)
        {
            return Err("native manifest and mixin identity/revision differ".into());
        }
        Ok(Self { manifest, files })
    }

    /// A digest of the files that go into a build: the same mod reinstalled has the same digest,
    /// a changed one another. The launcher compares it with the digest its build's `build.json`
    /// kept, so a mod whose source changed needs a new build even at the same version.
    pub fn digest(&self) -> String {
        let mut hash = sha2::Sha256::new();
        for (name, bytes) in &self.files {
            hash.update((name.len() as u64).to_be_bytes());
            hash.update(name.as_bytes());
            hash.update((bytes.len() as u64).to_be_bytes());
            hash.update(bytes);
        }
        hash.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    /// The bundle as the Android build service needs it: its manifest and sources, without the
    /// content only the game reads, so a mod's assets don't count against the request's size. The
    /// same files always make the same bytes, which is how the service knows it built them before.
    pub fn for_build(&self) -> Result<Vec<u8>, String> {
        use std::io::Write as _;
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default());
        for (name, bytes) in &self.files {
            zip.start_file(name.as_str(), options)
                .map_err(|e| e.to_string())?;
            zip.write_all(bytes).map_err(|e| e.to_string())?;
        }
        Ok(zip.finish().map_err(|e| e.to_string())?.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn manifest() -> Manifest {
        Manifest {
            schema: 1,
            id: "veinminer".into(),
            version: "0.3.0".into(),
            game: "26.3".into(),
            horizon: "a".repeat(40),
            mixins: "b".repeat(40),
            hooks: None,
        }
    }
    fn read(name: &str, extras: &[(&str, &[u8])], mixin: &str) -> Result<Bundle, String> {
        let path =
            std::env::temp_dir().join(format!("native-bundle-{name}-{}.hnmod", std::process::id()));
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file(MANIFEST, options).unwrap();
        zip.write_all(&serde_json::to_vec(&manifest()).unwrap())
            .unwrap();
        zip.start_file("mixins/veinminer/horizon-mixin.toml", options)
            .unwrap();
        zip.write_all(mixin.as_bytes()).unwrap();
        for (name, bytes) in extras {
            zip.start_file(*name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        let result = Bundle::read(&path);
        std::fs::remove_file(path).unwrap();
        result
    }
    fn mixin() -> String {
        format!(
            "[mod]\nid = \"veinminer\"\nversion = \"0.3.0\"\n[compatibility]\nhorizon_revision = \"{}\"\n",
            "a".repeat(40)
        )
    }
    #[test]
    fn source_bundle_carries_exact_compatibility_into_index_facts() {
        let bundle = read(
            "valid",
            &[("mixins/veinminer/hook.rs", b"fn hook() {}")],
            &mixin(),
        )
        .unwrap();
        assert_eq!(bundle.manifest, manifest());
        let facts = bundle.manifest.facts();
        assert_eq!(
            facts.api,
            format!("native-1/{}/{}", "a".repeat(40), "b".repeat(40))
        );
        assert_eq!(facts.id.as_deref(), Some("veinminer"));
        assert!(facts.capabilities.is_empty());
    }
    #[test]
    fn accepts_binary_data_in_the_bundle_data_directory() {
        let bytes = [0, 255, 42];
        let bundle = read(
            "data",
            &[
                ("mixins/veinminer/data/nested/table.bin", &bytes),
                ("mixins/veinminer/engine/data/model.bin", &bytes),
                ("mixins/veinminer/engine/README.md", b"# engine"),
                ("mixins/veinminer/engine/LICENSE-MIT", b"MIT"),
                ("mixins/veinminer/.gitignore", b"target/"),
            ],
            &mixin(),
        )
        .unwrap();
        assert_eq!(
            bundle.files["mixins/veinminer/data/nested/table.bin"],
            bytes
        );
    }
    #[test]
    fn rejects_path_traversal_and_non_source_payloads() {
        for (i, name) in [
            "../escape.rs",
            "/absolute.rs",
            "mixins/other/hook.rs",
            "mixins/veinminer/code.dll",
            "mixins/veinminer/data/../code.dll",
            "mixins/veinminer/database/file.bin",
            "mixins/veinminer/engine/data.bin",
            "mixins/veinminer/engine/src/data/blob.bin",
            "mixins/veinminer/engine/src/README.md",
            "mixins/veinminer/engine/README.dll/x.bin",
            "mixins/veinminer/engine/Cargo.lock",
            "mixins/other/data/file.bin",
            "mixins\\veinminer\\hook.rs",
        ]
        .iter()
        .enumerate()
        {
            assert!(read(&format!("bad-path-{i}"), &[(name, b"bad")], &mixin()).is_err());
        }
    }
    #[test]
    fn rejects_missing_or_mismatched_mixin_identity_without_panicking() {
        assert!(read("missing-mod", &[], "[unrelated]\nvalue = 1").is_err());
        assert!(
            read(
                "wrong-revision",
                &[],
                &mixin().replace(&"a".repeat(40), &"c".repeat(40))
            )
            .is_err()
        );
        assert!(read("wrong-version", &[], &mixin().replace("0.3.0", "0.2.0")).is_err());
        assert!(
            Manifest {
                horizon: "nightly".into(),
                ..manifest()
            }
            .check()
            .is_err()
        );
    }
    #[test]
    fn hooks_is_optional_and_kept() {
        let hooked = Manifest {
            hooks: Some(1),
            ..manifest()
        };
        assert!(hooked.check().is_ok());
        let text = serde_json::to_string(&hooked).unwrap();
        assert!(text.contains("\"hooks\":1"), "{text}");
        assert_eq!(serde_json::from_str::<Manifest>(&text).unwrap(), hooked);
        let plain = serde_json::to_string(&manifest()).unwrap();
        assert!(!plain.contains("hooks"), "{plain}");
        let zero = Manifest {
            hooks: Some(0),
            ..manifest()
        };
        assert!(zero.check().is_err());
    }

    #[test]
    fn what_goes_to_the_build_service_leaves_the_content_out_and_repeats() {
        let big = [7u8; 4096];
        let bundle = read(
            "for-build",
            &[
                ("mixins/veinminer/hook.rs", b"fn hook() {}"),
                ("assets/veinminer/big.bin", &big),
                ("data/veinminer/loot.json", b"{}"),
            ],
            &mixin(),
        )
        .unwrap();
        let sent = bundle.for_build().unwrap();
        assert_eq!(sent, bundle.for_build().unwrap());
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&sent)).unwrap();
        let names: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().unwrap().into_owned())
            .collect();
        assert_eq!(
            names,
            [
                "horizon-native.json",
                "mixins/veinminer/hook.rs",
                "mixins/veinminer/horizon-mixin.toml"
            ]
        );
        let path =
            std::env::temp_dir().join(format!("native-bundle-sent-{}.hnmod", std::process::id()));
        std::fs::write(&path, &sent).unwrap();
        let again = Bundle::read(&path);
        std::fs::remove_file(path).unwrap();
        assert_eq!(again.unwrap().files, bundle.files);
    }

    #[test]
    fn holds_is_the_cheap_test_for_a_native_manifest() {
        let dir = std::env::temp_dir();
        let native = dir.join(format!("native-holds-{}.hnmod", std::process::id()));
        let content = dir.join(format!("content-holds-{}.hmod", std::process::id()));
        for (path, entries) in [
            (&native, vec![("horizon-native.json", b"{}".as_slice())]),
            (
                &content,
                vec![("horizon-mod.toml", b"id = \"x\"".as_slice())],
            ),
        ] {
            let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
            for (name, bytes) in entries {
                zip.start_file(name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(bytes).unwrap();
            }
            zip.finish().unwrap();
        }
        assert!(holds(&native));
        assert!(!holds(&content));
        std::fs::remove_file(native).unwrap();
        std::fs::remove_file(content).unwrap();
    }

    #[test]
    fn one_hmod_carries_content_beside_its_native_code() {
        let toml = r#"id = "veinminer"
name = "Veinminer"
version = "0.3.0"
api = "0.2"
capabilities = ["registries"]
"#;
        let bundle = read(
            "unified",
            &[
                ("horizon-mod.toml", toml.as_bytes()),
                ("assets/veinminer/lang/en_us.json", b"{}"),
                ("data/veinminer/tags/item/ores.json", b"{}"),
                ("mixins/veinminer/hook.rs", b"fn hook() {}"),
            ],
            &mixin(),
        )
        .unwrap();
        assert!(bundle.files.contains_key("mixins/veinminer/hook.rs"));
        assert!(!bundle.files.contains_key("horizon-mod.toml"));
        assert!(!bundle.files.keys().any(|n| n.starts_with("assets/")));
        assert!(!bundle.files.keys().any(|n| n.starts_with("data/")));
    }

    #[test]
    fn one_file_is_one_mod() {
        let toml = r#"id = "other"
name = "Other"
version = "0.3.0"
api = "0.2"
capabilities = ["registries"]
"#;
        let error = read(
            "two-ids",
            &[("horizon-mod.toml", toml.as_bytes())],
            &mixin(),
        )
        .err()
        .unwrap();
        assert!(error.contains("one file is one mod"), "{error}");
    }
}
