# Native mods on Android (.hnmod)

Status: **import, list, enable/disable, remove, Compile and "play without
native mods" are wired end to end.** Bundles are compiled on the fork's own
build service (a Cloudflare Worker that rebuilds the game on GitHub-hosted
runners); the launcher downloads the resulting library, verifies it against
the job's SHA-256, writes `files/native/current.json` and the Java loader
hands it to the trampoline on the next start. The Java load chain was
verified on a device with a no-op mod compiled by the service; the Compile
button flow itself has not yet been exercised on a phone (see
[What was verified](#what-was-verified)).

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

Each row shows the id, version, size, a toggle that decides whether the mod
takes part in **Compile** (persisted in
`<instance>/horizon-native-mods/enabled.json`; a missing file means every
imported bundle is enabled), and a **Remove** button. Long-press a row to see
its id, version and file name. Each bundle's state:

| State | Meaning |
| --- | --- |
| Imported | Stored; part of Compile if its toggle is on. |
| Selected — runs on the next start | `files/native/current.json` points at a compiled build of it. |
| Set aside after crashing twice | The build crashed before its first frame twice and was set aside (see below). |
| Invalid bundle | The stored file no longer parses; remove and re-import it. |

## The Compile flow, end to end

1. Import one or more bundles as above and enable the ones you want built
   (the toggle per row).
2. Press **Compile** (Settings → Mods → Native mods (.hnmod), the big
   button below the list). If no mod is enabled the button is greyed out and
   says why; while a build is in flight it stays greyed out.
3. A confirmation dialog states the risk, literally:
   > Native mods run with full access to this device. Only compile mods you
   > trust. The mod code is compiled on a GitHub-hosted runner; nothing is
   > executed here. This may take a few minutes.

   **Cancel** / **Compile**.
4. The launcher packs the enabled bundles and `mods.json` into a single
   `mods.bin` (a zip), computes each bundle's SHA-256, and POSTs it
   (as the whole body) to `https://horizon-android-mod-builds.steve-horizon-android-hnmod.workers.dev/v1/build?version=0.7.7`.
   The request is content-addressed by the service; nothing about your APK
   other than this fork's pinned revision is published. The POST runs on a
   worker thread; if the upload exceeds the limits the compile refuses
   up front (more than 30 MiB of bundles, or a `mods.bin` larger than
   32 MiB — both are refused with a plain message, before anything leaves
   the phone).
5. The service answers `{"job": "<24 hex>"}` (or `{"cached": true}` when
   the exact same build was done before — the upload is then nearly free).
   The launcher stores the job id in `files/native/build.json` and starts
   polling `/v1/job/<job>` every few seconds for up to 45 minutes.
6. The inline status row under the list shows progress:
   - `queued` → **Waiting for a free build slot (N ahead).**
   - `building` → **Building… <seconds>s (up to ~15 min).** On a warm cache
     a no-op mod takes roughly 5–15 minutes; a cold or re-pinned base cache
     runs longer. **This is not a failure**; leave the launcher open or
     come back later (see Resuming below).
   - `done` → **Ready — restart Horizon to use it.** with a **Restart**
     button (see Restart below).
   - `failed` → the error string the worker returned.
7. When the job reaches `done`, the launcher automatically downloads
   `/v1/job/<job>/lib` to
   `files/native/builds/<job>/libhorizon_android.so`
   (atomic `.partial` + rename; resumes an interrupted download when the
   service honors `Range: bytes=N-` with HTTP 206). The library is
   SHA-256-verified against the job's published hash; on a mismatch it is
   deleted and **one from-scratch retry** is performed. Still mismatching:
   the download is abandoned, the partial file is removed, and you can
   press **Retry** or **Compile** again.
8. On success the launcher writes `files/native/current.json` in exactly the
   schema the Java loader reads (see below) and deletes `build.json`.
   The launcher's Mods page shows a badge **Native build active: <mod ids>**,
   switches the button label to **Recompile (applies after re-launch)** and
   keeps **Play without native mods** (which forgets the build) for recovery.
9. Restart the game. `HorizonActivity` picks the modded library at the
   start, the trampoline loads it, and the mods run with full access.

### Resuming

Closing or restarting the launcher mid-build does not lose state: the job id
is persisted in `files/native/build.json`, and the launcher resumes polling
(or the download, with its `.partial` file) on the next start of the UI.

### Restart

The **Restart** button sends `killBackgroundProcesses` for this app's own
`:game` process (the `KILL_BACKGROUND_PROCESSES` normal permission, granted
at install; no prompt). If the game is running, it is closed; press **Play**
to start with the modded build. If the kill is a no-op on your device, close
the game from Recents and press **Play** — the effect is the same: the game
reads `current.json` only at process start. A clean exit never advances the
native-mod crash guard, so restarting this way can never count as a crash.

## The versionCode invariant

`current.json` names the `versionCode` of the APK the build was compiled
for. **The build service compiles the game at the exact commit its
`versions.json` pins**, and the Java loader refuses a build whose
`versionCode` differs from the running APK — silently, logging one line and
running the stock game. Therefore the pinned commit and the APK you install
must come from the same `dev/android-classic-launcher-removal` commit: APKs
built from CI after `versions.json` is re-pinned to that commit work; any
other combination means native mods **never activate** despite every badge
saying the build is installed. `aapt2 dump badging file.apk` shows the
`versionCode`; the fork's CI dev APKs use `1000 + run_number`.

## Failure modes and recovery

| Symptom | What happened / what to do |
| --- | --- |
| Compile greyed out | No bundle imported (import one), no mod enabled (toggle one on), or a build is in flight (wait). |
| "No mods selected…" | Same as above; the dialog never opened. |
| "the selected mods add up to more than 30 MiB" / "build archive is larger than 32 MiB" | Disable some bundles or split the set; the service caps request size. |
| "cannot reach the build service" / "the build service answered HTTP 413/429" / "…HTTP 503" | Network or service capacity problem. 413 = too large (see above). 429/503 = busy or rate-limited; wait a few minutes. |
| "the build service did not return a job id" | The service rejected the request body. Keep the APK and the service's pinned commit in sync. |
| "Waiting for a free build slot (N ahead)" for a long time | All build runners are busy. On a warm base cache an idle slot finishes in minutes; the line disappears when the job starts. |
| **Building…** runs for a very long time | A cold build cache runs longest; typical no-op builds are ~5–15 minutes. 45 minutes is the client-side timeout, after which the job is forgotten and you can compile again. |
| "lost contact with the build service" | Eight consecutive polls failed. Check the network, keep the launcher open, or compile again. |
| "downloaded library failed sha256 — retried from scratch once and it still did not match" | The library bytes do not match the job's hash; the file was discarded. Check the connection to the service; if it repeats, the service/pinned commit is suspect — do not retry endlessly. |
| Modded build crashes before the first frame twice | The crash guard sets the build aside (`current.json` moves to `current.json.rejected`, `last_rejected` records why) and the stock game runs. Launcher shows the reason and **Play without native mods** stays available; the bundle itself is untouched. |
| Native mods never seem to activate after install | `versionCode` mismatch with the running APK (see above), `abi` not in `Build.SUPPORTED_ABIS`, a missing `native/active.json`, or a sha256 mismatch — all fail the Java check silently. Check logcat (`HorizonNativeMods` tag) for the rejection line. |
| **Play without native mods** | Forgets the selected build: stock game on the next start. The bundles, the downloaded library and `build.json` are kept; Compile again and the same cached build can be reinstalled if the APK did not change. |

**Play without native mods is always safe**: it only deletes
`native/current.json`, `active.json`, `pending_crash` and `crash_streak`.
Bundles are never touched.

## File layout

All paths inside the app's private `files/` directory (`home`) plus the game
content folder (`game_home`, default the same):

| Path | Role |
| --- | --- |
| `game_home/horizon-native-mods/<id>.hnmod` | The imported bundle (moves with the game files). |
| `game_home/horizon-native-mods/enabled.json` | `{"enabled": ["<id>", …]}` — the Compile selection; missing file = all enabled. |
| `native/current.json` | The selected build: `{"id", "version", "lib", "stock", "abi", "versionCode", "sha256", "mods":[{"id","version","sha256"}]}`. |
| `native/active.json` | Written on **every** start by Java: `{"lib": …, "stock": …}`. The trampoline reads this; installs read `stock` from it when present. |
| `native/active.json.tmp` | Staging file for the atomic rename that publishes `active.json`. |
| `native/builds/<job>/libhorizon_android.so` | The library the build service produced for job `<job>`. |
| `native/builds/<job>/libhorizon_android.so.partial` | Interrupted download, resumed with `Range:` when possible. |
| `native/build.json` | The in-flight build (job id, mods, hashes, APK identity, `stock`) so the launcher can resume after a restart. Deleted on success. |
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

**Compile** does this automatically; the by-hand path exists for tests. With
a modded `libhorizon_android.so` built for this fork's APK, write
`files/native/builds/<id>/libhorizon_android.so` and a matching
`current.json` (the schema above; `versionCode` of this APK, `abi`
`arm64-v8a`, `sha256` of the library). The launcher shows it as selected; the
crash guard still applies. Removing it (**Play without native mods**, or
deleting `current.json`) returns to stock.

## What was verified

- The stock path runs indistinguishably from the previous APK
  (`current.json` absent ⇒ `active.json` points at the stock library) —
  confirmed on-device for the stock game.
- The Java load order, the trampoline's `dlopen` dispatch and the crash
  guard end to end — confirmed on-device: a modded build selected by an
  installed `current.json` starts and survives, and a crashing build is
  set aside.
- The build service compiles the pinned commit with an imported bundle
  end to end through GitHub Actions (a no-op mod produced a working
  library); the Compile loop in this APK drives the same service.
- Not yet verified: pressing **Compile** on a phone and watching the whole
  import → compile → install → restart → mod-live cycle. Anyone with the
  dev APK should run exactly that and report back.
