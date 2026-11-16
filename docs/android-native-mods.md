# Native mods on Android (.hnmod)

Status: **import, list, remove and "play without native mods" work. Nothing can
build a native mod yet — this fork has no Horizon mod build service.** The
game's loader machinery is in place and is exercised on every start; it selects
between the stock library and an installed modded build, exactly as if a build
service already existed. The Compose launcher's **Compile** button is disabled
and says why; no network call, fake endpoint or local build is attempted.

## What a native mod is

A `.hnmod` is a [horizon-mixins](https://github.com/horizon-rust/horizon-mixins)
source bundle: a `horizon-native.json` manifest, a `horizon-mixin.toml` and
`.rs` hook files that patch the game's own Rust before it is compiled. It is
not sandboxed and not interpreted — it is compiled into the game. See
[native-mods.md](native-mods.md) for the bundle format and hook points.

The upstream registry validates bundles with `horizon_mods::index::native`
(schema 1, `id` `[a-z0-9_]{1,64}`, exact `.rs`/`.toml` sources under
`mixins/<id>/` plus crate text files and `data/` directories, at most 512
files and 64 MiB unpacked, manifest identity matching the mixin manifest).
This fork imports bundles with the same rules
(`horizon-mods`, `index` feature), so a bundle the registry accepts imports
here, and vice versa.

## What works today

Three import paths feed the same bundle folder, all with the same validation
(`horizon-mods`, `Bundle::read`):

- **SAF picker**: the source picker on the Mods page (**Native mods (.hnmod) → Import**);
- **HTTPS link**: paste an `https://…/name.hnmod` link into the Mods page search field and
  press Search — the file is downloaded (at most 64 MiB) and stored on success;
- **Horizon catalog**: source-only `.hnmod` rows carry **Install**: the bundle is
  downloaded and verified against the registry's SHA-256, validated and stored.

The bundle is stored as `<instance>/horizon-native-mods/<id>.hnmod` (it moves with the rest
of the game files if you change **Settings → Storage → Game files location**). No second
copy is made and nothing is executed.

The list shows each bundle's id, version, size and one of:

| State | Meaning |
| --- | --- |
| Imported | Stored; no build is selected to run it. |
| Selected — runs on the next start | `files/native/current.json` points at a compiled build of it. |
| Set aside after crashing twice | The build crashed before its first frame twice and was set aside (see below). |
| Invalid bundle | The stored file no longer parses; remove and re-import it. |

Each row has a **Remove** button (with a confirmation) that deletes only the
stored bundle. It never touches a built library. **Play without native mods**
forgets the selected build: the next start runs the stock game.

## What does not

**Compile** is visible and disabled on purpose:

> This build of Horizon has no Horizon mod build service. Native mods need a
> service that recompiles the game; nothing can build them on the phone yet.

A source-only bundle cannot become a runnable library on the phone and cannot
be run by the stock game. To make one fire you need one of:

- a Linux builder (VM or service) running the MIT-licensed toolchain adapted
  from [GoyDevv/Horizon-Android](https://github.com/GoyDevv/Horizon-Android),
  with `versions.json` entries for this fork's commits, which compiles the
  game with the bundles' hooks and returns the resulting
  `libhorizon_android.so`; or
- that author's build service, once it accepts this fork's releases.

Until one exists, `files/native/current.json` is never written by the fork
itself; it can only appear from a manual or external install (see below),
in which case the loader honors it and the launcher shows the build as
selected.

## File layout

All paths inside the app's private `files/` directory, written by
`HorizonActivity` (Java) and `NativeMods.java` before the game starts:

| File | Role |
| --- | --- |
| `horizon-native-mods/<id>.hnmod` | The imported bundle (per instance, with the game files). |
| `native/current.json` | The selected build, if any: `{"id", "version", "lib", "sha256", "versionCode", "abi", "added"}`. The `alias`-like fields let the launcher match it to this APK. |
| `native/active.json` | Written on **every** start: `{"lib": …}` — stock or the selected build. The trampoline reads this. |
| `native/active.json.tmp` | Staging file for the atomic rename that publishes `active.json`. |
| `native/builds/<id>/<version>/libhorizon_android.so` | Where a build service must place the compiled library (referenced from `current.json`). Not used yet. |
| `native/pending_crash` | Marker left on every start that uses a modded build; the game deletes it on its first frame. |
| `native/crash_streak` | How many starts in a row died before the first frame (`0` on a clean start). |
| `native/current.json.rejected` | A set-aside build, kept for the error message. |
| `native/last_rejected` | Human-readable reason the build was set aside; the launcher shows it in a banner. |

## The load chain

1. `HorizonActivity.onCreate` runs before `super.onCreate`:
   - parses `native/current.json` and checks it: well-formed, targets this
     APK's `longVersionCode`, `abi` matches a supported ABI, the library
     exists and its SHA-256 matches, and the crash guard (below) passes;
   - writes `native/active.json` atomically (stock path, or the chosen
     library) **and always writes it** — the trampoline depends on it;
   - creates `native/pending_crash` when a modded build was chosen;
   - `System.load()s` the chosen library, so `HorizonActivity`'s JNI bindings
     resolve against the same library the process runs;
   - then `super.onCreate` loads **`libhorizon_trampoline.so`** (the
     `android.app.lib_name` meta-data).
2. The trampoline (`crates/horizon-trampoline`, a cdylib with no game code)
   reads `active.json`, sets `HORIZON_GAME_LIB` and
   `HORIZON_NATIVE_PENDING` (the `pending_crash` path) in the environment,
   `dlopen()`s the named library and calls its `android_main` — the game, either
   the stock build or a modded one.
3. On its first `about_to_wait`, the game deletes `pending_crash` and writes
   `crash_streak = 0` when those variables are set and the marker exists,
   marking the start clean. Missing variables mean a normal stock run.

## The crash guard

A modded build that crashes before rendering its first frame is suspected on
the next start: `crash_streak` increments. Two crashes in a row set the build
aside — `current.json` moves to `current.json.rejected`, `last_rejected`
records why, and the stock game runs. Removing the marker by hand (or playing
without native mods) resets the cycle. A build that reaches the first frame
clears the guard itself.

## Installing a build by hand (for testers)

A build service is not required to *try* the loader: with a modded
`libhorizon_android.so` built for this fork's APK, write `builds/<id>/<ver>/libhorizon_android.so`
and a matching `current.json` (`versionCode` of this APK, `abi` `arm64-v8a`,
`sha256` of the library, `added` ISO-8601). The launcher shows it as selected;
the crash guard still applies. Removing it (**Play without native mods**, or
deleting `current.json`) returns to stock.

## What was verified

- CI builds the APK with the trampoline and runs the source checks.
- The stock path is designed to be indistinguishable from the previous APK:
  `current.json` absent ⇒ `active.json` points at the stock library.

Not verified on a device: the Java load order, the trampoline's `dlopen`
dispatch, and the crash guard end-to-end. Anyone with the dev APK should
confirm the game still starts, and that **Play without native mods** and the
`.hnmod` import/list/remove flows behave. Nothing about native mods is known
to work or fail beyond what is listed here.
