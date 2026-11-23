# Native mods on Android (`.hmod` and legacy `.hnmod`)

Status: **import, list, enable/disable and remove are wired end to end.**
Native mod **building is not supported on Android**: there is no upstream
Android SDK slice to rebuild the game against, and the hosted build pipeline
has been removed from this fork. Native mods now use the new `mod_api` SDK and
`.hmod` packages on supported hosts. Nothing is compiled or executed on the
phone; a bundle carrying native code can only run as part of a game library
installed for this APK by other means (for example, by hand for testing).

## What a native mod is

A `.hnmod` bundle is a native-only mod; a unified `.hmod` may carry both
content and native code. The archive's contents, not its extension, decide
what it is: a file with `horizon-native.json` is native
(`horizon_mods::index::native::holds`), whether it arrives as `.hmod` or
`.hnmod`. Native code runs **unsandboxed** with full access to the device; it
is never treated like a sandboxed content mod.

The fork imports bundles with the same validation as the upstream registry
(`horizon-mods`, `index` feature): schema 1, id `[a-z0-9_]{1,64}`, exact
`.rs`/`.toml` sources under `mixins/<id>/`, at most 512 files and 64 MiB
unpacked, with manifest identity matching the mixin manifest.

## What works today

Three import paths feed the same bundle folder, all with the same validation
(`horizon-mods`, `Bundle::read`):

- **SAF picker**: the source picker on the Mods page
  (**Native mods (.hmod / legacy .hnmod) → Import**);
- **HTTPS link**: paste an `https://…/name.hmod` (or `.hnmod`) link into the
  Mods page search field and press **Import .hmod** (at most 64 MiB). A pasted
  link gets **no registry hash or signature check** by default; only a URL that
  exactly matches a known Horizon registry file is checked against that entry,
  and a failed check refuses the import. Paste only links you trust.
- **Horizon catalog**: native rows carry **Install**; the bundle is downloaded
  and checked against the registry's hash/signature where the registry
  provides one, then validated and stored.

The bundle is stored as `<game>/horizon-native-mods/<id>.hmod` or `<id>.hnmod`
(it moves with the rest of the game files if you change **Settings → Storage →
Game files location**). A unified `.hmod` is also mirrored to
`<game>/horizon-mods/<id>.hmod` so the content installer and list see its
content part; a legacy `.hnmod` has no mirror. **Remove** deletes both files
when they provably belong to that bundle. Import itself never executes code.

Each row shows the id, version, size, an enable/disable toggle (persisted in
`<game>/horizon-native-mods/enabled.json`; a missing file means every imported
bundle is enabled), and a Remove button. Long-press a row for its id and
version. The row's state:

| State | Meaning |
| --- | --- |
| Imported | Stored and enabled in the launcher's list. |
| Disabled | Stored, but its toggle is off. |
| Selected — runs on the next start | `files/native/current.json` names a library for it. |
| Set aside after crashing twice | The library crashed before its first frame twice. |
| Invalid bundle | The stored file no longer parses; remove and re-import it. |

**Play without native mods** clears the loader state so the stock game runs on
the next start. It never touches the bundles, their mirrored content, or a
library installed by hand.

## File layout

All paths inside the app's private `files/` directory (`home`) plus the game
content folder (`game_home`, by default the same):

| Path | Role |
| --- | --- |
| `game_home/horizon-native-mods/<id>.hmod` / `<id>.hnmod` | The imported native bundle, in its original format. |
| `game_home/horizon-mods/<id>.hmod` | Mirror of the content part of a unified `.hmod`. No mirror for legacy `.hnmod`. |
| `game_home/horizon-native-mods/enabled.json` | `{"enabled": ["<id>", …]}`; a missing file means all enabled. |
| `native/current.json` | The library selected for this APK, when one is installed by hand. |
| `native/active.json` | Written every start by Java: `{"lib": …, "stock": …}`; the trampoline reads it. |
| `native/pending_crash`, `native/crash_streak` | Crash guard for a selected library. |
| `native/first-frame` | Written by the game on its first frame. |
| `native/last_known_good.json`, `native/current.json.rejected`, `native/last_rejected`, `native/last_failed_mod` | Rollback and rejection state for the Java loader. |

The loader (`NativeMods.java`) validates `current.json` (version code, ABI,
APK install stamp, optional API gates and SHA-256), writes `active.json`, and
loads the chosen library before `super.onCreate`. The trampoline forwards
Android's start callback to it. A library that crashes before its first frame
twice is set aside and the stock game runs.

## What was verified

- The stock path runs indistinguishably from the previous APK
  (`current.json` absent ⇒ `active.json` points at the stock library) —
  confirmed on-device.
- The Java load order, the trampoline's `dlopen` dispatch and the crash guard —
  confirmed on-device: a library selected via `current.json` starts and
  survives, and a crashing one is set aside.
- The hosted build service is no longer part of this fork. Its former
  import → compile → install → restart cycle is not supported on Android.
