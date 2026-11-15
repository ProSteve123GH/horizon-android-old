//! The native mods a build carries (`docs/android-native-mods.md`): the launcher's catalog calls a
//! native mod "Built" only when the chosen build's `build.json` lists it. A source `.hnmod` alone
//! is a download, not a build.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Where the builds are, under the launcher's home (`native_builds` reads the same folder).
pub(crate) const DIR: &str = "native";
/// The build chosen for the next start.
pub(crate) const CURRENT: &str = "current";
/// A build's folder starts with this; a new build takes the place of the last.
pub(crate) const COMPILED: &str = "compiled-";

pub(crate) fn dir(home: &Path) -> PathBuf {
    home.join(DIR)
}

pub(crate) fn chosen(home: &Path) -> Option<String> {
    let name = std::fs::read_to_string(dir(home).join(CURRENT)).ok()?;
    let name = name.trim();
    (!name.is_empty() && plain_build(name)).then(|| name.to_owned())
}

pub(crate) fn plain_build(name: &str) -> bool {
    name.starts_with(COMPILED)
        && name.len() <= 80
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_'))
}

/// The mods a build was made with, from its `build.json`.
pub(crate) fn build_mods(folder: &Path) -> Vec<(String, String)> {
    let Ok(text) = std::fs::read_to_string(folder.join("build.json")) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    value["mods"]
        .as_array()
        .map(|mods| {
            mods.iter()
                .filter_map(|m| {
                    Some((
                        m["id"].as_str()?.to_owned(),
                        m["version"].as_str().unwrap_or_default().to_owned(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The ids of the mods the chosen build carries: what the catalog's "Built" means. Empty until a
/// build is chosen, and a build that was never chosen does not count.
pub(crate) fn built_ids(home: &Path) -> BTreeSet<String> {
    chosen(home)
        .map(|name| build_mods(&dir(home).join(name)))
        .unwrap_or_default()
        .into_iter()
        .map(|(id, _version)| id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_chosen_builds_mods_are_built() {
        let home = std::env::temp_dir().join(format!(
            "horizon-native-built-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        let _ = std::fs::remove_dir_all(&home);
        let older = dir(&home).join("compiled-older");
        let newer = dir(&home).join("compiled-newer");
        std::fs::create_dir_all(&older).unwrap();
        std::fs::create_dir_all(&newer).unwrap();
        std::fs::write(
            older.join("build.json"),
            r#"{"mods":[{"id":"apple","version":"1.0"},{"id":"zoom","version":"2.0"}]}"#,
        )
        .unwrap();
        std::fs::write(
            newer.join("build.json"),
            r#"{"mods":[{"id":"beta","version":"3.0"}]}"#,
        )
        .unwrap();

        // A source `.hnmod` is not a build: nothing is built until one is chosen.
        assert!(built_ids(&home).is_empty());

        std::fs::write(dir(&home).join(CURRENT), "compiled-older\n").unwrap();
        let built = built_ids(&home);
        assert_eq!(
            built.iter().map(String::as_str).collect::<Vec<_>>(),
            ["apple", "zoom"]
        );

        // Choosing another build switches what counts: the older build's mods no longer do.
        std::fs::write(dir(&home).join(CURRENT), "compiled-newer\n").unwrap();
        let built = built_ids(&home);
        assert_eq!(
            built.iter().map(String::as_str).collect::<Vec<_>>(),
            ["beta"]
        );

        let _ = std::fs::remove_dir_all(&home);
    }
}
