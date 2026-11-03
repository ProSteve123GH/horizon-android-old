# Development APKs

The **Android dev APKs** workflow builds installable APKs without publishing a
GitHub Release. It runs on pushes to `android`, `android-dev*` and
`dev/android-*` when build inputs change. **Run workflow** can build all four
ABIs and choose the `release` or `fast` Rust profile. Pull requests never receive
the signing secret and do not trigger this workflow.

The default push build is `arm64-v8a`. A dispatch with **all_abis** builds
`arm64-v8a`, `armeabi-v7a`, `x86` and `x86_64`. The APK uses the optimized Rust
`release` profile by default (ThinLTO), public payload checks and on-device
Mojang downloads.

The CI package is `dev.horizon.android.dev`, labelled **Horizon Android Dev**, so
it installs beside a release app. The version name contains the workflow run and
source commit; the version code is `1000 + run_number`. Test data is separate:
worlds and accounts are not copied from the release app.

The workflow never publishes GitHub Releases, sends Discord messages or uploads
raw Minecraft inputs, prepared resources, player data or signing keys.

## Getting an APK

1. Open **Actions → Android dev APKs** in the fork.
2. Open the successful run for the branch and commit you want.
3. Download the artifact for the ABI, for example
   `horizon-dev-123-arm64-v8a`.
4. Unpack the artifact ZIP and install the APK. Android may require allowing
   installs from the browser or file manager used to open it.

Each artifact contains the APK, `SHA256SUMS.txt`, `LICENSE.txt`,
`THIRD_PARTY_NOTICES.md` and `BUILD.json`. `BUILD.json` records the exact commit,
branch, ABI, Rust profile, package, version code, signing source and workflow run.
Artifacts expire after 14 days. GitHub requires sign-in to download them.

These are experimental builds. They have not automatically passed gameplay,
graphics-driver or performance testing. Emulator frame rates are not phone
results.

## Signing

The workflow prefers the repository secret `ANDROID_DEV_KEYSTORE_BASE64`. If it
is absent, the workflow restores a cached fallback dev key. If neither exists, it
generates a new fallback key and caches only that generated key. A key supplied
through the secret is never written to the Actions cache.

The key must use:

- alias `androiddebugkey`;
- store password `android`;
- key password `android`.

The same key allows in-place updates between CI dev builds. Replacing it requires
uninstalling the dev app first, which deletes its app-private test data. Normal
release installations are unaffected. Back up test worlds before changing keys.

Create a dedicated dev key, never the release key:

```powershell
keytool -genkeypair -keystore "$env:TEMP/horizon-ci-dev.keystore" `
  -storepass android -keypass android -alias androiddebugkey `
  -keyalg RSA -validity 10000 -dname "CN=Horizon Android Dev CI"
[Convert]::ToBase64String([IO.File]::ReadAllBytes("$env:TEMP/horizon-ci-dev.keystore")) |
  Set-Clipboard
```

Add the clipboard contents as `ANDROID_DEV_KEYSTORE_BASE64` in repository
Settings → Secrets and variables → Actions. Keep a private backup outside the
repository. Clear the clipboard afterwards. Never attach the key to an issue,
artifact, release, commit or message.

## Build environment

The workflow uses Ubuntu 24.04, JDK 25 for the Mojang exporters, JDK 17 for
Gradle/Android, Gradle 8.14, Android platform 36, build tools 35.0.0 and NDK
28.2.13676358. The split is deliberate: vanilla Minecraft 26.3 requires Java SE
25, but Horizon does not run the Java client, and Gradle 8.14 only runs on Java
24 or older. It caches Cargo dependencies, the prepared engine inputs and a
generated fallback signing key (never a key supplied through the secret). Cold
builds prepare the generated engine tables from official downloads and can take
substantially longer than incremental builds.

The source copy of the workflow is `packaging/android/dev-builds.yml`; the active
copy is `.github/workflows/android-dev.yml`. Keep them identical. Local builds
and the SDK/NDK setup are described in [the Android guide](android.md#build-developers-only).
