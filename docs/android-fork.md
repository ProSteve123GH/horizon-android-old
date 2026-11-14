# Horizon Android fork

This repository is an unofficial Android fork of
[Vasysik/horizon-android](https://github.com/Vasysik/horizon-android), which in
turn packages the Rust [Horizon engine](https://github.com/ludovic111/horizon).
The fork keeps upstream authorship and history. It does not replace the
upstream repository and does not publish GitHub Releases automatically.

## What this fork adds

| Area | Change |
| --- | --- |
| Launcher | Merged [upstream PR #56](https://github.com/Vasysik/horizon-android/pull/56), the Jetpack Compose launcher and its JNI bridge. The Rust launcher pages remain in the `:game` process. |
| Minecraft reuse | Import an existing Minecraft 26.3 folder or ZIP through Android's Storage Access Framework, verify it against the engine pins and download only missing files. |
| Repair | A **Repair missing files** action on the installed state verifies the prepared resources and redownloads missing or damaged Minecraft files. |
| Dev identity | CI builds use `dev.horizon.android.dev` and the label **Horizon Android Dev**, separate from a release install. |
| Dev builds | `.github/workflows/android-dev.yml` builds an arm64 APK on fork branches and can build all ABIs on dispatch. |
| Docs | Import, fork, upstream-sync and dev-build notes are in this directory. |

## PR #56 status

PR #56 (`compose-launcher`, by `maciasdav`) was still open upstream when this
fork merged it. The merge is local to this fork (merge commit `8667de85a`);
nothing was pushed to `Vasysik/horizon-android`. The conflict resolution kept
the later upstream Android fixes for accounts, wardrobe and input. The Compose
launcher and the Rust game still share the same controller bridge and prepared
installation.

If upstream changes PR #56, do not rebase published fork history. Merge the
updated PR branch, resolve the Android files again and record the result.

## Compose launcher coverage

The Compose launcher covers Play, install/import progress, Skins, settings,
profiles, saved accounts and Ely.by sign-in, content and diagnostics. The
skin/cape gallery and account controls use the same shared Rust wardrobe and
account APIs as the classic Rust launcher. **Settings → Open classic launcher**
remains a fallback and starts the `:game` process without a pending play request.
The game itself always runs as a client of the server thread or a remote server;
opening the classic launcher does not change that architecture.

## Package and signing identity

| Build | Application ID | Label |
| --- | --- | --- |
| Local debug | `dev.horizon.android.dev` | Horizon Android Dev |
| CI dev | `dev.horizon.android.dev` | Horizon Android Dev |
| Public/release | `dev.horizon.android` | Horizon Android |

CI dev builds use `ANDROID_DEV_KEYSTORE_BASE64` when configured. Without it, the
workflow restores or creates a cached fallback dev key. The key is never
committed, uploaded as an artifact or placed in a GitHub Release. Replacing the
key requires uninstalling the old dev app; see
[development APKs](android-dev-builds.md).

## Non-goals

- Running arbitrary Minecraft JAR versions or Java mods.
- Bundling Mojang jars, assets, Java runtimes or LWJGL.
- Publishing an APK automatically to Releases.
- Claiming emulator frame rates are phone performance.
- Merging experimental upstream branches automatically.

See [Minecraft installation import](android-minecraft-import.md) and
[upstream sync](android-upstream-sync.md).
