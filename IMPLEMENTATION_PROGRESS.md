# Android feature implementation progress

Last updated: 2026-11-05 UTC

Roadmap: `umm/01.md` … `umm/12.md`, in numerical order. The extra CI-speed and
Game Files storage requests are recorded separately below.

## Summary

| Feature | Status | Branch | Commit | Android build |
|---|---|---|---|---|
| 01 Instance Manager | COMPLETED | `dev/android-feature-01-instance-manager` | `480b15d5c` | Run 41845522596, success |
| 02 World Manager | COMPLETED | `dev/android-feature-02-world-manager` | `58b8aaeaf` | Run 41872293141, success; APK checked |
| 03 Per-instance Performance Presets | COMPLETED | `dev/android-feature-03-performance-presets` | `d096991e6` | Run 41957716637, success; APK checked |
| 04 Server Manager | COMPLETED | `dev/android-feature-04-server-manager` | `cca2ad531` | Run 41968664989, success; APK checked |
| 05 Customizable Launcher | COMPLETED | `dev/android-feature-05-customizable-launcher` | `70b36be4a` | Run 41975677227, success; APK checked |
| 06 Screenshot Gallery | COMPLETED | `dev/android-feature-06-screenshot-gallery` | `fc97dc614` | Run 41986881875, success; source run 41989318206; APK checked |
| 07 Advanced Touch Control Editor | PENDING | — | — | — |
| 08 Android Performance HUD | PENDING | — | — | — |
| 09 Input Device Profiles | PENDING | — | — | — |
| 10 Launcher File Manager | PENDING | — | — | — |
| 11 Mobile Inventory Improvements | PENDING | — | — | — |
| 12 Photo Mode | PENDING | — | — | — |

Stack order: `dev/android-feature-01-instance-manager` →
`dev/android-ci-speed` → `dev/android-feature-02-world-manager` →
`dev/android-game-files-write` → `dev/android-feature-03-performance-presets` →
`dev/android-feature-04-server-manager` → `dev/android-feature-05-customizable-launcher` →
`dev/android-feature-06-screenshot-gallery`.
Feature 07 starts from the Feature 06 branch.

## Feature 01: Instance Manager

- Status: COMPLETED.
- Branch: `dev/android-feature-01-instance-manager`, commit `480b15d5c`.
- Changes: per-instance home profiles with create/rename/delete/duplicate,
  the active-instance marker, instance metadata and last-played display,
  Compose profile sheet, and launch routing through the selected instance.
- Important files: `crates/horizon-client/src/android/instances.rs`,
  `bridge.rs`, `android_main.rs`, `packaging/android/kotlin/dev/horizon/android/launcher/Profiles.kt`.
- Tests: `python3 tools/test_android_instances.py` passed; local Android
  unittest discovery passed; public-tree and formatting checks passed.
- Android build: run 41845522596 succeeded (22m15s total).
- Limitations: none known for the delivered scope; no local Android SDK, so
  compilation is verified through CI. The user confirmed the installed build.

## Feature 02: World Manager

- Status: COMPLETED.
- Branch: `dev/android-feature-02-world-manager`, commit `58b8aaeaf`.
- Changes: the shared desktop `launcher/src/worlds.rs` model is reused by
  Android; lenient world listing, metadata fallbacks, rename, duplicate,
  delete-with-confirmation, direct world launch, instance labels, symlink and
  stray-file rejection, and session-lock checks. `--world` is passed to the
  game; the configured server is cleared for a direct world launch.
- Important files: `launcher/src/worlds.rs`, `launcher/src/worlds/tests.rs`,
  `crates/horizon-client/src/android/{bridge.rs,save.rs,game_launch.rs,launcher.rs}`,
  `packaging/android/kotlin/dev/horizon/android/launcher/Worlds.kt`,
  `LauncherApp.kt`, `tools/test_android_worlds.py`.
- Tests: `python3 tools/test_android_worlds.py` 5/5 passed; instance tests
  passed; `rustfmt`, `git diff --check`, public-tree check passed.
- Android build: run 41872293141 succeeded; APK dex contains `WorldsSheet`,
  `WorldRow`, `WorldsButton`, `Overlay$Worlds` and `world_launch`,
  `world_rename`, `world_duplicate`, `world_delete`.
- Limitations: the source-check workflow runs on `android`/PRs, not on feature
  pushes. Commit `8a9acd2c2` has literal `\n` sequences in its message; fixing
  it would require a history rewrite, which the rules forbid.

## Feature 03: Per-instance Performance Presets

- Status: COMPLETED; Android build and APK verified.
- Branch: `dev/android-feature-03-performance-presets`, commits `02123a675`
  (engine and bridge), `905dc49ee` (Compose screens), `6005bd4f2`
  (application timing), `d76da5ad6` (source-check workflow), `2f3d68c9f`
  (unavailable stored presets), `3228e5437` (full engine ranges),
  `9c6d0ea61` (selection integrity) and `d096991e6` (both engine preset
  labels, preview errors and the settings-row fallback).
- Changes:
  - `performance_presets.rs` owns the complete supported catalog: render and
    simulation distance, FPS cap, resolution scale, clouds, particles, entity
    shadows, smooth lighting, biome blend, mipmap levels, LOD distance, cave
    and entity culling, FXAA, TAA, worker count and battery mode. Keys, ranges
    and defaults come from the engine's `options_file.rs` and `graphics.rs`.
  - Battery Saver, Balanced and Performance built-ins; custom presets can be
    created from current values and edited, renamed, duplicated and deleted.
  - Presets are per instance (`android-performance.json` beside that instance's
    engine files). Applying one patches only the targeted keys in `options.txt`
    and `horizon-graphics.txt`, preserves every other line, handles CRLF and
    duplicate keys, and backs up a corrupt metadata file before replacing it.
  - Every apply goes through a preview that lists the old and new values and
    marks worker-thread changes as needing a game restart. Both of the game's
    preset labels are set to Custom: the vanilla `graphicsPreset` key in
    `options.txt` and the Horizon `preset` key in `horizon-graphics.txt`. The
    preview names each stale label as a separate change, so neither settings
    screen claims a preset the values no longer match.
  - The Settings performance row opens the preset sheet; custom values use a
    per-setting editor for every supported type.
- Important files: `crates/horizon-client/src/android/performance_presets.rs`,
  `crates/horizon-client/src/android/bridge.rs`,
  `crates/horizon-client/src/android_main.rs`,
  `packaging/android/kotlin/dev/horizon/android/launcher/PerformancePresets.kt`,
  `LauncherApp.kt`, `SettingsScreen.kt`,
  `tools/test_android_performance_presets.py`.
- Tests: `python3 tools/test_android_performance_presets.py` 14/14 passed;
  an additional local smoke program applied all three built-ins and checked
  their render distance, FPS cap, resolution scale and both preset labels.
  Instance, world and document-store harnesses passed; Android unittest
  discovery 15/15; `cargo fmt --check`, `git diff --check`, public-tree check
  passed. The Rust Android target compiles locally with
  `cargo check -p horizon-android --target aarch64-linux-android
  --no-default-features`.
- Android build: run 41957716637 succeeded at `d096991e6` in 17m5s (release
  profile, arm64-v8a). The APK checksum verified, and its dex contains
  `PerformanceSheet`, `PerformancePreviewDialog`, `PerformanceEditorDialog`,
  `PerformanceValueDialog`, `openPerformance`, the Settings-screen wiring and
  every bridge action (`performance_state`, `preview`, `apply`, `create`,
  `update`, `rename`, `duplicate`, `delete`, `reset`). The native library
  contains the store path, built-ins, both preset-label messages and the
  Custom labels.
- Limitations: Kotlin compilation still needs CI because this Termux host has
  no Android SDK. The presets only change the catalogued keys; they do not
  promise a frame rate, temperature or battery-life result. Close the game
  before applying, since a running game can save its own options on exit.
  A stored custom preset with an unknown key from a newer build is marked
  unavailable and can be deleted, but the current editor cannot remove the
  unknown key in place. No on-device UI test has been performed in this
  session; CI verifies Kotlin compilation and the APK contents.

## Feature 04: Server Manager

- Status: COMPLETED; Android build and APK verified.
- Branch: `dev/android-feature-04-server-manager`, commit `cca2ad531`.
- Changes:
  - Saved servers use the shared desktop `servers.dat` model in
    `launcher/src/server_list.rs`, extended with edit, duplicate, reorder, name
    copying and position lookup while preserving unknown NBT fields and icons.
  - Per-instance `android-servers.json` stores favorites and optional groups,
    reconciled by name and address so entries added elsewhere remain usable.
  - Add, edit, delete-with-confirmation, duplicate, reorder and favorite actions;
    separate addresses and ports are validated before saving or launching.
  - Icons are limited to a safe size and type. Invalid or oversized icons are
    reported without destroying the existing entry.
  - Status checks are manual only. Background threads call
    `horizon_client::net::ping`, the UI polls the local `server_status` action
    briefly, and no continuous network polling runs.
  - Launching validates the raw address, sets the existing quick-connect server
    setting and uses the normal `play(None)` multiplayer path.
  - A Compose servers sheet with filters, group editing, status rows and launch
    actions is wired into Settings and the Play screen.
- Important files: `launcher/src/server_list.rs`,
  `launcher/src/server_list/tests.rs`,
  `crates/horizon-client/src/android/servers.rs`,
  `crates/horizon-client/src/android/bridge.rs`,
  `packaging/android/kotlin/dev/horizon/android/launcher/Servers.kt`,
  `LauncherApp.kt`, `SettingsScreen.kt`, `PlayScreen.kt`,
  `tools/test_android_servers.py`.
- Tests: `python3 tools/test_android_servers.py` passed all seven groups;
  `cargo check -p horizon-android --target aarch64-linux-android
  --no-default-features`, `cargo fmt --check`, `git diff --check`,
  public-tree and workflow byte-identity checks passed. Clippy without
  `-D warnings` shows no new warning in the changed files; the repository's
  pre-existing `needless_borrow` in `horizon-client-input/src/keybinds.rs:260`
  still blocks a global `-D warnings` run.
- Android build: run 41968664989 succeeded in 15m24s (release profile,
  arm64-v8a), including Kotlin compilation, the Rust NDK build, signing, audit
  and artifact upload. APK artifact `horizon-dev-29-arm64-v8a`
  (id `13388179872`) downloaded and its SHA-256
  `758c8511e2835c9ec6a0ad626a84f2f546e70359f37dcde05b7e310d0904eef5`
  matched `SHA256SUMS.txt`. The dex contains `ServersSheet`,
  `ServerEditDialog`, `ServerRow`, `Overlay$Servers` and `openServers`; the
  native library contains the per-instance store path, validation messages and
  the server action/status messages.
- Limitations: no local Android SDK, so Kotlin compilation and APK packaging
  are verified through CI. No on-device UI test has been performed in this
  session. A server that is unreachable is shown as unchecked/offline without
  retrying automatically. Hidden `servers.dat` entries are preserved but not
  shown or managed.

## Feature 05: Customizable Launcher

- Status: COMPLETED; Android build and APK verified.
- Branch: `dev/android-feature-05-customizable-launcher`, commit `70b36be4a`.
- Changes:
  - Launcher-wide appearance is stored in `android-appearance.json` with
    System/Light/Dark themes, eight accent choices (system plus seven built-in
    palettes), and a custom-background marker.
  - The Rust palette owns both light and dark primary roles and
    primary-container roles, so text, selected navigation, buttons, chip and
    icon-container colours stay consistent across every Compose screen.
  - The Appearance sheet applies theme and accent changes immediately. Reset
    restores System defaults and removes the copied background.
  - Background images are chosen through Android Documents, decoded with a
    sample cap, recompressed as a bounded 1600 px JPEG and stored privately;
    the original can be moved or deleted afterwards. Missing, empty,
    oversized or unreadable copies fall back to the theme background.
  - The sheet is scrollable, uses full-width 48 dp actions on narrow screens,
    disables destructive actions while an import runs, and keeps errors in
    text as well as colour. Static image rendering is capped at 1600 px; no
    network or animation is used.
- Important files: `crates/horizon-client/src/android/appearance.rs`,
  `bridge.rs`, `android_main.rs`,
  `packaging/android/kotlin/dev/horizon/android/launcher/AppearanceModel.kt`,
  `AppearanceSheet.kt`, `Theme.kt`, `LauncherApp.kt`, `SettingsScreen.kt`,
  `tools/test_android_launcher_appearance.py`.
- Tests: `python3 tools/test_android_launcher_appearance.py` 11/11 passed,
  covering every accent's contrast, theme/accent/background persistence,
  corrupt-store fallback, missing/deleted copies, empty/oversized files and
  reset. `cargo check -p horizon-android --target aarch64-linux-android
  --no-default-features`, `cargo fmt --check`, `git diff --check`, public-tree
  and workflow byte-identity checks passed.
- Android build: run 41975677227 succeeded in 16m3s at `70b36be4a` (release
  profile, arm64-v8a), including Kotlin compilation, the Rust NDK build, signing,
  audit and artifact upload. The downloaded APK artifact
  `horizon-dev-30-arm64-v8a` (id `13392471525`) has SHA-256
  `2227fec07f34015e7d898cfbbbe49da028c71f96a8c8e378fa0c2771bca85b2b`,
  matching `SHA256SUMS.txt`. Its dex contains `AppearanceSheet`,
  `LauncherBackground`, `LauncherAppearance`, `ThemeChoice`, `AccentChoice`,
  `openAppearance` and every `appearance_*` bridge action; the native library
  contains the appearance store path, background file names, palette labels,
  status messages and fallback strings.
- Limitations: Kotlin compilation and APK packaging are verified through CI
  because this Termux host has no Android SDK. No on-device UI/orientation test
  has been performed in this session. Background orientation metadata is not
  applied, and a transparent PNG is flattened onto JPEG's black background.
  The new source-check harness is included in both workflow copies, but the
  source-check workflow runs on `android`/PRs, not on a feature-branch push; it
  was run locally.

## Feature 06: Screenshot Gallery

- Status: COMPLETED; Android build, source checks and APK verified.
- Branch: `dev/android-feature-06-screenshot-gallery`, commits `b5c8002ff`
  (gallery), `514d75489` (public-tree provider allowlist) and `fc97dc614`
  (Kotlin compilation fix).
- Changes:
  - The Rust store scans the originals in `home/screenshots` and
    `home/profiles/profile-N/screenshots`, supporting PNG, JPEG, WebP and GIF.
    It reads dimensions from image headers, records size and modification time,
    and sorts newest/oldest/name without loading full images.
  - List entries use opaque IDs such as `p<profile>/<filename>`. Inspect, open
    and delete re-resolve each ID under the active home, reject traversal,
    symlinks and unsafe names, and tolerate an image that was already removed.
    Unreadable instance folders and unsupported or broken files are skipped.
  - A direct `NativeBridge.screenshots(order)` JNI entry runs listing off the
    Compose main thread. The existing `screenshots` and `screenshot_delete`
    bridge actions reuse the same store.
  - Compose shows a responsive adaptive grid with 512 px sampled thumbnails and
    an LRU cache capped at one eighth of the heap. The full-screen preview
    samples at up to 2048 px and provides pinch zoom, one-finger pan and
    double-tap reset, plus filename, resolution, size and date.
  - Sharing uses the non-exported read-only `ScreenshotProvider` with a
    temporary `FLAG_GRANT_READ_URI_PERMISSION` grant and `ClipData`; the share
    sheet never receives a filesystem path. The provider accepts read mode only
    and returns empty metadata or a checked open failure when a file vanished.
  - Confirmed deletion removes only the original. Newest/oldest/name sorting,
    optional grouping by instance (not by unreliable filenames), an empty state,
    an explicit error/retry state and entries from Play and Settings complete
    the screen.
- Important files:
  `crates/horizon-client/src/android/screenshots.rs`,
  `crates/horizon-client/src/android/bridge.rs`, `android_main.rs`,
  `packaging/android/java/dev/horizon/android/ScreenshotProvider.java`,
  `packaging/android/kotlin/dev/horizon/android/launcher/Screenshots.kt`,
  `LauncherApp.kt`, `PlayScreen.kt`, `SettingsScreen.kt`, `AndroidManifest.xml`,
  `docs/android.md`, `docs/android-features.md`,
  `tools/test_android_screenshots.py`, `tools/check_public_tree.py`.
- Tests: `python3 tools/test_android_screenshots.py` passed 18 native tests plus
  the provider/share security contract. Cases cover empty and missing folders,
  a 160-file gallery, all supported formats, a 4096 × 2160 image, stable
  date/name order, traversal and symlink rejection, high-resolution metadata,
  unreadable folders, deletion persistence and repeated/missing deletion.
  `cargo check -p horizon-android --target aarch64-linux-android
  --no-default-features`, `cargo fmt -p horizon-android -- --check`,
  `git diff --check`, public-tree history check, Android unittest discovery
  15/15 and the workflow byte-identity check passed. Clippy reported only
  existing warnings outside the changed code.
- Android build: run 41986881875 succeeded in 17m17s at `fc97dc614` (release
  profile, arm64-v8a), including Kotlin/Java compilation, the Rust NDK build,
  signing, audit and artifact upload. Downloaded APK artifact
  `horizon-dev-33-arm64-v8a` (id `13396214296`) has SHA-256
  `1f1b19ec063c2d7f0e8a5a5601ab427be751d0bd2d2064a0b15a5a482896a116`,
  matching `SHA256SUMS.txt`. The dex contains `ScreenshotProvider`,
  `ScreenshotGallery`, `ScreenshotGrid`, `PreviewImage` and
  `DeleteScreenshotDialog`; the native library exports
  `NativeBridge_screenshots`, `ScreenshotProvider_nativeCall` and
  `ScreenshotProvider_nativeOpen`. The manifest contains the
  `.screenshots` provider with `grantUriPermissions`. Manual source-check run
  41989318206 also passed in 3m30s.
- Limitations: Kotlin/Java compilation and APK packaging are verified through
  CI because this Termux host has no Android SDK. No on-device test of the grid,
  pinch/pan gestures, orientation changes, share sheet, thumbnail memory or
  deletion dialog has been performed. The gallery lists internal originals;
  copies already published to `Pictures/HorizonAndroid` are independent.
  Grouping uses the instance profile only, because filenames do not reliably
  identify worlds. Sort and grouping choices survive Activity recreation but
  not a full process restart; the screenshot list itself is rescanned from disk.
  App-private files have no runtime storage permission to revoke, but
  unreadable folders and disappearing files are handled as described above.

## Extra: faster dev APK builds

- Status: COMPLETED (measured).
- Branch: `dev/android-ci-speed`, commits `ab30e9893`, `942998c71`.
- Changes: 4 Cargo jobs; the Cargo cache keeps its original path list so the
  default branch cache stays valid; Gradle build cache separated; Cargo,
  Gradle and Minecraft caches saved only on `refs/heads/android`; detailed
  display names.
- Tests: workflow file pairs are byte-identical
  (`.github/workflows/android-dev.yml` ↔ `packaging/android/dev-builds.yml`);
  public-tree and formatting checks passed.
- Android build: run 41870078843 succeeded, 14m51s total (11m33s build), cache
  restore 14s. The earlier cold run 41864509546 took 20m18s (16m57s build).
- Limitations: a feature branch still recompiles the large `horizon-client`
  crate, so runs that change that crate remain roughly 15–19 minutes. Caches
  are not saved on feature branches by design, to avoid evicting the default
  branch cache.

## Extra: writable Game Files (SAF)

- Status: COMPLETED and build-verified.
- Branch: `dev/android-game-files-write`, commits `1602f71b1`, `bcc5eef55`,
  `faf08411c` (and merge commits).
- Changes:
  - The provider advertises the real `DocumentsContract.Document` flags:
    write, delete, rename, move and directory create. It no longer advertises
    copy, because `copyDocument` is not implemented.
  - The four allowlisted configuration files can be written, renamed, moved
    and deleted; fixed roots and reports stay read-only.
  - Recursive delete for child folders after the system confirmation; moving a
    directory into Configuration under an allowlisted name is rejected.
  - `moveDocument` and target-parent notifications added to the Java adapter.
  - The game holds `game.lock`; worlds/config refuse content operations while
    the game is running. Each world's `session.lock` is probed.
  - Moves now set the launcher's content-changed notice.
  - Documentation updated in `docs/android-content.md` and
    `docs/android-features.md`.
- Important files: `crates/horizon-client/src/android/document_store.rs`,
  `document_provider.rs`, `launcher.rs`,
  `packaging/android/java/dev/horizon/android/HorizonDocumentsProvider.java`,
  `tools/test_android_document_store.py`.
- Tests: `python3 tools/test_android_document_store.py` 3/3 passed after the
  flag correction; worlds harness 5/5; Android unittest discovery 15/15;
  `rustfmt`, `git diff --check`, public-tree check passed.
- Android build: run 41942713416 succeeded at `faf08411c` (release profile,
  arm64-v8a). The downloaded APK contains `moveDocument`, `deleteDocument` and
  `renameDocument`; the earlier build at `12d3b5b48` also passed.
- Limitations: local source harnesses `test_android_content.py` and
  `test_android_appearance.py` fail on Termux for pre-existing host-target
  reasons (missing `rustix` in the content harness on Android hosts, and a
  missing `secure_store`/`java_player_with` module in the appearance harness).
  The source-check workflow only runs on `android`/PRs. No on-device Files
  test has been performed in this session.

## Build-environment note

On 2026-11-05 GitHub disabled workflows on the fork because of the scale of
Actions usage. The maintainer re-enabled them. The first run after re-enabling
is manual run 41942713416 for the storage flag fix `faf08411c`.

Actions usage is a finite resource on this fork, so each feature gets one
release-profile build with `all_abis=false`; avoid re-running a build without a
code change, and do not dispatch speculative builds.

There is still no Android SDK/NDK on this Termux host, so APK compilation must
go through CI.

## Next step

1. Feature 06 is complete; the branch is green and its APK was checked.
2. Start Feature 07 (Advanced Touch Control Editor) from
   `dev/android-feature-06-screenshot-gallery`.
