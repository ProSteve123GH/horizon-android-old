# Horizon Android 0.8.0

## Changes

- The launcher players see first is now Kotlin + Jetpack Compose
  (`packaging/android/kotlin/dev/horizon/android/launcher/`), in Material 3
  Expressive: dynamic colour on Android 12+, Material icons, expressive shapes
  and motion, a navigation rail on wide screens and a bottom bar on phones held
  upright. Sizes follow Android's density, so text and touch targets have their
  normal size.
- The launcher's logic stays in Rust. `crates/horizon-client/src/android/bridge.rs`
  gives the Compose UI one JSON snapshot of the state and takes its actions by
  name (`NativeBridge.snapshot`, `NativeBridge.call`, `NativeBridge.importDocument`).
  Installation, the catalog, Microsoft sign-in, settings, imports and diagnostics
  are the same code as before.
- The game (`HorizonActivity`, a NativeActivity) now runs in the `:game` process.
  Play writes `launch-request.json`; the game process starts the world without
  its own launcher pages and closes when the player returns to the launcher.
- The classic Rust launcher was removed. The Compose launcher is the only
  launcher UI; starting the game process without a play request finishes the
  activity back to the Compose launcher.
- Release APKs are shrunk with R8 (`packaging/android/proguard-rules.pro`).
  Compose adds about 3 MB.
- Native-mod (`.hnmod`) machinery: the APK ships a small trampoline
  (`crates/horizon-trampoline`) named by
  `android.app.lib_name`; on every start the game activity validates
  `files/native/current.json`, writes `files/native/active.json` and loads the
  chosen library before `NativeActivity` starts, so an installed modded build
  runs instead of the stock game. A build that crashes before its first
  frame twice is set aside and the stock game runs. `.hnmod` and unified
  `.hmod` bundles can be imported, listed and removed as source-only archives;
  Android does not compile or execute them, and there is no native
  enable/disable or **Compile** action. The Installed page labels sandboxed
  content/WASM mods separately from native source bundles. See
  [android-native-mods.md](android-native-mods.md).

## Build notes

- `compileSdk` is 36. Material 3 `1.5.0-alpha18` is the newest release with the
  Expressive APIs that builds with Android Gradle Plugin 8.11; newer ones need
  compile SDK 37 and a newer plugin.
- Builds need `platforms;android-36` installed in the SDK.

## Launcher polish

- Compact type scale and component sizes for phones; Horizon's mark on the home
  header; settings behind a gear as a full-screen layer, with sliders for
  resolution and refresh rate and Save / Cancel in every settings dialog.
- Profile manager: named profiles with a summary of their worlds and add-ons,
  created, renamed and deleted from the launcher (`profiles`, `profile_create`,
  `profile_rename`, `profile_delete` in `bridge.rs`).
- Catalog pages scroll as one list; settings, dialogs, the account sheet and the
  profile manager use two columns when the phone is held sideways.

## Validation

- Kotlin compiles and the release APK builds with R8.
- Android Rust code passes `--check` and `--clippy` for `arm64-v8a`.
- The Compose screens were rendered with Robolectric at 891 × 411 dp and
  411 × 891 dp to check layout. Not yet run on a phone: the game process
  hand-off and return need a device check.
- The launcher screens were used on a POCO X6 Pro (Android 17), portrait and
  landscape. Launching the game from this build still needs a device check.
- Not verified on a device: the trampoline load chain and the native-mod crash
  guard (a clean stock start must behave exactly as before); needs a phone check.
