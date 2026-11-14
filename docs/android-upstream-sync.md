# Keeping the Android fork in sync

This fork has two upstreams:

| Upstream | Remote | What it supplies |
| --- | --- | --- |
| [Vasysik/horizon-android](https://github.com/Vasysik/horizon-android) | `upstream` | Android packaging, JNI bridges, touch input, Android launcher and Android-specific fixes |
| [ludovic111/horizon](https://github.com/ludovic111/horizon) | usually reached through an Android merge | The Rust engine, protocol, renderer and game code |

Only `origin` (`ProSteve123GH/horizon-android`) is a push target. Never push to
either upstream, never force-push and never rewrite published fork history.

## Routine update

```sh
git switch android
git fetch upstream
git log --oneline android..upstream/android
git diff --stat android...upstream/android
git merge --no-ff upstream/android
```

Before merging, review the changes that touch this fork's surface:

- `crates/horizon-client/src/android/` and `android_main.rs`;
- `crates/horizon-client/src/official_install.rs`;
- `packaging/android/` and `.github/workflows/android-dev.yml`;
- `tools/android_*.py`;
- `docs/android*.md` and `docs/android-upstream.json`.

Resolve Android module paths and bridges explicitly. Preserve the Compose
launcher, the import/repair code, the dev application ID, the classic-launcher
entry point and the dev workflow. Do not copy files from upstream when a merge
can preserve authorship. Do not merge experimental upstream branches
automatically; review a stable `android` revision or a specific release branch.

## Updating the Compose launcher PR

PR #56 was merged as a normal merge commit. If the PR branch changes:

```sh
git fetch upstream pull/56/head:upstream-pr-56
git switch dev/android-compose-import
git merge --no-ff upstream-pr-56
```

Keep the later upstream account, wardrobe and input fixes. Re-check the Compose
screens against `LauncherActions`, `Snapshot` and the JNI declarations; a
renamed bridge field is a compile error in Kotlin, not a Rust error. Record the
new merge commit in [the fork notes](android-fork.md).

## Workflows

Upstream deployment workflows stay inactive in this fork. The only active
Android build workflow is `.github/workflows/android-dev.yml`; its source copy is
`packaging/android/dev-builds.yml`. Keep them identical:

```sh
cmp .github/workflows/android-dev.yml packaging/android/dev-builds.yml
```

The dev workflow must not publish GitHub Releases, send Discord messages or
upload signing keys. It prefers `ANDROID_DEV_KEYSTORE_BASE64`; the cached key is
only a fallback and is never uploaded as an artifact.

## Checks before pushing

Run the checks that match the change. A full Android merge usually needs:

```sh
cargo fmt --all --check
cargo check -p horizon-android
cargo test -p horizon-client --lib --no-default-features --features android-test minecraft_import
python3 -m unittest discover -s tests -p 'test_android*.py'
python3 tools/check_public_tree.py
python3 - <<'PY'
import yaml
for path in ['.github/workflows/android-dev.yml', 'packaging/android/dev-builds.yml']:
    yaml.safe_load(open(path))
PY
```

A local Android SDK/NDK is required for `tools/android_build.py`. When it is not
available, push the branch and let the dev workflow compile the Kotlin/Compose
side. Download the arm64 artifact, inspect `BUILD.json`, and state clearly in the
PR or report that the APK was compiled but not device-tested.

## Recording the merge

Update `docs/android-upstream.json` when the engine baseline changes, and update
[the fork notes](android-fork.md) when the Android upstream or PR #56 changes.
Record:

- the upstream commit or PR head;
- the merge commit on the fork;
- what conflicted and how it was resolved;
- which checks ran locally;
- which checks ran only in CI;
- any feature that remains on the classic Rust launcher.

Do not claim a device test, a frame-rate result or a gameplay validation that did
not happen.
