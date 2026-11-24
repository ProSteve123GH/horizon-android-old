use super::*;
use serde_json::{json, Value};

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
const SHA2: &str = "fedcba9876543210fedcba9876543210fedcba98";

fn link(text: &str) -> Link {
    parse(text).unwrap()
}

#[test]
fn links_name_a_repository_release_ref_or_commit() {
    let plain = link("https://github.com/acme/zoom");
    assert_eq!(
        (plain.owner.as_str(), plain.repo.as_str(), plain.target),
        ("acme", "zoom", Target::Latest)
    );
    assert_eq!(link("https://github.com/acme/zoom.git/").repo, "zoom");
    assert_eq!(
        link("https://github.com/acme/zoom/releases").target,
        Target::Latest
    );
    assert_eq!(
        link("https://github.com/acme/zoom/releases/tag/v1.2.0").target,
        Target::Tag("v1.2.0".into())
    );
    // A branch name keeps its slashes.
    assert_eq!(
        link("https://github.com/acme/zoom/tree/feature/x?tab=1").target,
        Target::Ref("feature/x".into())
    );
    assert_eq!(
        link("https://github.com/acme/zoom/commit/ABCDEF1").target,
        Target::Commit("abcdef1".into())
    );
}

#[test]
fn links_that_are_not_github_repositories_are_refused() {
    for bad in [
        "",
        "acme/zoom",
        "http://github.com/acme/zoom",
        "https://gitlab.com/acme/zoom",
        "https://github.com/acme",
        "https://github.com/acme/zoom/issues/3",
        "https://github.com/acme/zoom/commit/not-hex",
        "https://github.com/acme/zoom/tree/a..b",
        "https://github.com/../zoom",
        "https://github.com/acme/zo om",
    ] {
        assert!(parse(bad).is_err(), "{bad:?}");
    }
}

/// A recorded API: the JSON each path answers, 404 for the rest.
fn recorded(answers: Vec<(&'static str, Value)>) -> impl Fn(&str) -> Result<Option<Value>, String> {
    move |path| {
        Ok(answers
            .iter()
            .find(|(p, _)| *p == path)
            .map(|(_, v)| v.clone()))
    }
}

fn release(tag: &str, assets: Value) -> Value {
    json!({ "tag_name": tag, "assets": assets })
}

fn asset(name: &str, digest: Option<&str>) -> Value {
    json!({
        "name": name,
        "browser_download_url": format!("https://github.com/acme/zoom/releases/download/v1/{name}"),
        "digest": digest,
    })
}

#[test]
fn a_repository_resolves_to_its_latest_releases_hnmod_and_commit() {
    let digest = format!("sha256:{}", "AB".repeat(32));
    let api = recorded(vec![
        (
            "repos/acme/zoom/releases/latest",
            release(
                "v1",
                json!([
                    asset("zoom.hmod", None),
                    asset("zoom-1.hnmod", Some(&digest))
                ]),
            ),
        ),
        ("repos/acme/zoom/commits/v1", json!({ "sha": SHA })),
    ]);
    let found = resolve(&link("https://github.com/acme/zoom"), &api).unwrap();
    assert_eq!(
        (found.tag.as_deref(), found.commit.as_str()),
        (Some("v1"), SHA)
    );
    let file = found.asset.unwrap();
    assert_eq!(file.name, "zoom-1.hnmod");
    assert_eq!(file.sha256, Some("ab".repeat(32)));
    assert_eq!(found.content_only, None);
}

#[test]
fn a_release_with_no_hnmod_falls_back_to_its_commit_and_remembers_a_content_mod() {
    let api = recorded(vec![
        (
            "repos/acme/zoom/releases/latest",
            release("v1", json!([asset("zoom.hmod", None)])),
        ),
        ("repos/acme/zoom/commits/v1", json!({ "sha": SHA })),
    ]);
    let found = resolve(&link("https://github.com/acme/zoom"), &api).unwrap();
    assert_eq!(found.asset, None);
    assert_eq!(found.content_only.as_deref(), Some("zoom.hmod"));
    assert_eq!(found.commit, SHA);
}

#[test]
fn an_asset_from_another_host_is_ignored() {
    let evil = json!({ "name": "x.hnmod", "browser_download_url": "https://evil.example/x.hnmod" });
    let api = recorded(vec![
        (
            "repos/acme/zoom/releases/latest",
            release("v1", json!([evil])),
        ),
        ("repos/acme/zoom/commits/v1", json!({ "sha": SHA })),
    ]);
    assert_eq!(
        resolve(&link("https://github.com/acme/zoom"), &api)
            .unwrap()
            .asset,
        None
    );
}

#[test]
fn no_release_resolves_the_default_branch_to_a_commit() {
    let api = recorded(vec![
        ("repos/acme/zoom", json!({ "default_branch": "dev/main" })),
        ("repos/acme/zoom/commits/dev/main", json!({ "sha": SHA })),
    ]);
    let found = resolve(&link("https://github.com/acme/zoom"), &api).unwrap();
    assert_eq!(
        (&found.tag, &found.asset, found.commit.as_str()),
        (&None, &None, SHA)
    );
    assert_eq!(found.label(), "acme/zoom @ 0123456");
    assert_eq!(
        found.source_url(),
        format!("https://github.com/acme/zoom/tree/{SHA}")
    );
}

#[test]
fn refs_and_short_commits_become_full_commits_and_unknown_ones_say_so() {
    let api = recorded(vec![
        ("repos/acme/zoom/commits/feature/x", json!({ "sha": SHA })),
        ("repos/acme/zoom/commits/abcdef1", json!({ "sha": SHA2 })),
        ("repos/acme/zoom/releases/tags/v9", Value::Null),
    ]);
    let commit = |url: &str| resolve(&link(url), &api).map(|r| r.commit);
    assert_eq!(
        commit("https://github.com/acme/zoom/tree/feature/x"),
        Ok(SHA.into())
    );
    assert_eq!(
        commit("https://github.com/acme/zoom/commit/abcdef1"),
        Ok(SHA2.into())
    );
    let err = commit("https://github.com/acme/zoom/tree/gone").unwrap_err();
    assert!(err.contains("no commit, tag or branch gone"), "{err}");
    // A tag with no release is a commit.
    assert!(commit("https://github.com/acme/zoom/releases/tag/v9").is_err());
}

#[test]
fn a_rate_limit_is_said_plainly() {
    let limited = |_: &str| Err(RATE_LIMIT.to_string());
    let err = resolve(&link("https://github.com/acme/zoom"), &limited).unwrap_err();
    assert!(
        err.contains("rate limit") && err.contains("Try again"),
        "{err}"
    );
}

fn tree(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gh-mods-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// Bytes of an already-built native package, as an older launcher would have
/// shipped it as a release asset. Installing it must still work.
fn built_hnmod(id: &str) -> Vec<u8> {
    use std::io::Write;
    let manifest = json!({
        "schema": 1, "id": id, "version": "0.1.0", "game": "26.3",
        "horizon": SHA, "mixins": SHA2,
    });
    let source = format!("[mod]\nid = \"{id}\"\nversion = \"0.1.0\"\n[compatibility]\nhorizon_revision = \"{SHA}\"\n");
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("horizon-native.json", options).unwrap();
    zip.write_all(manifest.to_string().as_bytes()).unwrap();
    zip.start_file(format!("mixins/{id}/horizon-mixin.toml"), options)
        .unwrap();
    zip.write_all(source.as_bytes()).unwrap();
    zip.finish().unwrap().into_inner()
}

#[test]
fn a_commit_link_has_no_built_asset_to_install() {
    let found = Resolved {
        owner: "acme".into(),
        repo: "zoom".into(),
        tag: None,
        commit: SHA.into(),
        asset: None,
        content_only: None,
    };
    assert!(fetch(&found)
        .unwrap_err()
        .contains("no longer supported"));
}

fn packed(id: &str) -> Packed {
    Packed {
        id: id.into(),
        version: "0.1.0".into(),
        file: PathBuf::new(),
        sha256: String::new(),
    }
}

#[test]
fn several_mods_list_and_ask_which() {
    let two = [packed("keys"), packed("zoom")];
    let err = choose(&two, None, false).unwrap_err();
    assert!(err.contains("keys, zoom") && err.contains("--all"), "{err}");
    assert_eq!(choose(&two, Some("zoom"), false).unwrap()[0].id, "zoom");
    assert_eq!(choose(&two, None, true).unwrap().len(), 2);
    assert!(choose(&two, Some("map"), false).is_err());
    assert_eq!(choose(&two[..1], None, false).unwrap()[0].id, "keys");
}

#[test]
fn installing_records_the_origin_and_updates_follow_releases_or_the_head() {
    let home = tree("install");
    let source = tree("install-src");
    let bytes = built_hnmod("zoom");
    let ready = checked(&bytes, &source.join("out")).unwrap();
    let found = Resolved {
        owner: "acme".into(),
        repo: "zoom".into(),
        tag: None,
        commit: SHA.into(),
        asset: None,
        content_only: None,
    };
    let kept = ready.file.clone();
    let origin = install(&home, &found, &ready).unwrap();
    assert!(!kept.exists());
    assert_eq!(origin.label(), "acme/zoom @ 0123456");
    assert_eq!(origin.sha256, sha256(&bytes));
    assert_eq!(self::origin(&home, "zoom"), Some(origin.clone()));
    assert_eq!(origins(&home), vec![origin.clone()]);
    assert!(java::is_enabled(&home, "zoom"));
    // A commit's mod follows the default branch's head.
    assert_eq!(origin.follows().target, Target::Head);
    let newer = Resolved {
        commit: SHA2.into(),
        ..found.clone()
    };
    assert!(is_update(&origin, &newer));
    assert!(!is_update(&origin, &found));
    // A release's mod follows the latest release.
    let release = Origin {
        tag: Some("v1".into()),
        asset: Some("zoom.hnmod".into()),
        ..origin.clone()
    };
    assert_eq!(release.label(), "acme/zoom @ v1");
    assert_eq!(release.follows().target, Target::Latest);
    let v2 = Resolved {
        tag: Some("v2".into()),
        ..found.clone()
    };
    assert!(is_update(&release, &v2));
    assert!(!is_update(
        &release,
        &Resolved {
            tag: Some("v1".into()),
            ..found
        }
    ));
    // A file that isn't from GitHub is never replaced.
    write(&home.join(INSTALLED).join("keys.hnmod"), "x");
    let other = Packed {
        id: "keys".into(),
        ..packed("keys")
    };
    assert!(install(&home, &newer, &other)
        .unwrap_err()
        .contains("not from GitHub"));
    assert!(remove(&home, "zoom").unwrap());
    assert!(!remove(&home, "zoom").unwrap());
    assert!(!home.join(INSTALLED).join("zoom.hnmod").exists());
    assert!(!java::is_enabled(&home, "zoom"));
    let _ = fs::remove_dir_all(home);
    let _ = fs::remove_dir_all(source);
}
