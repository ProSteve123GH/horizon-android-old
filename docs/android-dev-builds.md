# Development APKs

Three Actions workflows build Android test APKs:

- **Android dev APKs** builds ARM64 on code pushes to `android`. Documentation-only
  pushes are skipped and older in-progress builds are cancelled when a newer
  commit arrives. **Run workflow → all_abis** builds all four ABIs. This is the
  signed, in-place-update build for the `android` branch.
- **Android PR APKs** builds a pull request or a manually selected branch without
  the signing secret. The build script generates a throwaway key, so uninstall
  an older dev app before installing one of these previews.
- **Android signed preview APK** is manual dispatch only. It restores the dev
  signing key, so it updates in place over earlier dev or signed preview builds.
  Use it to test a feature branch before it reaches `android`.

Pull requests never receive the signing key. The dev and signed preview
workflows restore it; the dev workflow only runs on `android`, and only a user
with write access can start a signed preview dispatch.

These APKs use the optimized Rust `release` profile (including ThinLTO), public
payload checks and official resource downloads. The package is
`dev.horizon.android.dev`, separate from the normal release, and the visible
version contains the run number and source commit. Test data is separate;
worlds and accounts are not automatically copied from the release app.
The workflow does not publish GitHub Releases or send Discord messages.

## One-time maintainer setup

Create a **new, dedicated dev key**, not the release signing key. In PowerShell:

```powershell
keytool -genkeypair -keystore "$env:TEMP/horizon-ci-dev.keystore" -storepass android -keypass android -alias androiddebugkey -keyalg RSA -validity 10000 -dname "CN=Horizon Android Dev CI"
[Convert]::ToBase64String([IO.File]::ReadAllBytes("$env:TEMP/horizon-ci-dev.keystore")) | Set-Clipboard
```

In repository Settings → Secrets and variables → Actions, add
`ANDROID_DEV_KEYSTORE_BASE64` with the clipboard contents. Keep a private backup
of this key outside the repository. The password/alias above match the existing
build configuration. Clear the clipboard afterwards. Never attach the key to an
issue, artifact, release or message.

The signed workflows fail early if the secret is absent. Keeping the same key
permits in-place updates between CI dev and signed preview builds; replacing it
requires reinstalling the dev app. Existing locally signed debug APKs may use a
different key as well. Back up their test data before uninstalling. Normal
release installations are unaffected. Version codes use epoch seconds, shared
by the signed workflows, so a later build updates an earlier one. Do not replace
the key without warning testers that they must reinstall once. A fork-specific
key will not update APKs signed with another fork's key.

## Sharing with testers

Open Actions → the successful workflow run → Artifacts. Share the run URL in
Discord and specify its commit, architecture and workflow name. Download and
unpack the artifact ZIP, then install its APK. GitHub requires sign-in to
download workflow artifacts; artifacts expire after 14 days. Each contains the
APK, checksums, license notices and `BUILD.json` identifying the exact source,
build profile and signing identity (`dev-key` or `throwaway`). Experimental
builds have not automatically passed gameplay or performance tests.

Dev and signed preview artifacts signed with the same key update in place.
PR artifacts use a new throwaway key on every run; uninstall the previous app
before installing a different PR artifact. Do not publish a signed preview APK
from an unreviewed branch.

Private signing keys, raw Minecraft inputs, prepared resource directories,
player data and local diagnostics are not uploaded as downloadable artifacts.
Only Cargo/Gradle build caches are retained separately for subsequent CI builds.
Cold builds also prepare the generated engine tables, so they may take
substantially longer than an incremental local build.
