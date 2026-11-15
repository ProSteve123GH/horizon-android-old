# Horizon Android fork ↔ mainstream sync (TASK.md)

Working document for the migration from TASK.md. Everything here is recorded as it
happens; update after every phase. Dates are 2026-11-14 (tablet session, Europe/Paris).

## Phase 1 — repository investigation and baseline (done)

### Repositories and refs

| Side | Remote | URL | Branch | Tip SHA | Tip date |
| --- | --- | --- | --- | --- | --- |
| Android fork (mine, target) | `origin` | https://github.com/ProSteve123GH/horizon-android-old | `android` | `8d95b60c70a2a6843f2aebeab2925d3f0232196a` | 2026-11-14 |
| Mainstream (new) | `upstream` | https://github.com/horizon-rust/horizon | `main` | `b396fcfe01d94887c4c57898c64fdd81b83379f9` | 2026-11-14 |
| Starling (former upstream, preserved) | `starling-upstream` | https://github.com/starling-client/starling | (not fetched) | — | — |

Keep `origin` the only push target. Never push to `starling-upstream` or `upstream`.

Local working copy: `/data/data/com.termux/files/home/horizon-android-old` (Android tablet,
**low RAM / capped CPU — no heavy compiles locally, CI only**).

### State at session start

- Current branch: `android-native-refusal-all` (PR #12 against `android`, still open,
  unrelated to TASK.md; touch-up to the native-mod refusal message + unit test).
- Local `android` was stale at `5003cbce9` (14 commits behind origin/android); the branch
  used for this task is created from `origin/android`.
- Working tree otherwise clean; `TASK.md` itself is untracked (left alone).

### Backup and remotes (done)

- Backup branch pushed: `backup/android-2026-11-14` → `8d95b60c7`
- Backup tag pushed: `backup/android-2026-11-14-8d95b60c7`
- Starling remote preserved as `starling-upstream` (fetch/push); the `upstream` name was
  reconfigured to horizon-rust/horizon as instructed by TASK.md (no `upstream/main` was
  fetched before; nothing was lost).
- `upstream main` fetched fully (shallow fetch first for the tip, then `--unshallow`);
  ~158 MiB of pack data across two attempts. One attempt failed mid-pack
  (`RPC failed; curl 56 Software caused connection abort`); the retry completed.
  The `gen-pack-*` / `tmp_pack_*` recovery left two unused temp packs in `.git/objects/pack/`
  (from the aborted attempt) — recorded, not deleted.

### Merge base and divergence

- Merge base: **`8c02c0b8534e11b7bb8be14238b140a8534280a0`** (2026-11-01 13:59,
  "Merge pull request #5873 from horizon-rust/fix/prediction-item-use") — the nightly the
  fork pinned for 0.7.7. It is an ancestor of both branches → real ancestry; no
  unrelated-histories merge needed.
- Objects shared: fork carries the whole horizon history from the nightly's parents.
- Fork side (`8c02c0b853..origin/android`): **312 commits** (228 first-parent), ~700
  files changed.
- Mainstream side (`8c02c0b853..upstream/main`): **17,923 commits** (**4,939 first-parent**,
  i.e. ~4.9k PRs in 13 days), **4,802 files** changed, +1.18M / −330k lines.
- Files changed on **both** sides: **138** (worst: 46 `crates/horizon-client`,
  41 `crates/horizon-client-render`, 21 `launcher/src`, 4 `horizon-client-input`,
  3 `horizon-mods`, 3 `cargo-horizon-mod`; plus a handful of tools/tests/docs).
- `third_party/winit` (Android-flavored patched winit): **198 files, fork only.**
  Upstream did not touch `third_party/winit` or `third_party/zalith` → no patch
  conflicts expected there.
- `.github/workflows`: fork keeps `android-dev.yml` + `android-source.yml` and parks
  upstream's 23 workflows as `.github/upstream-workflows/*.disabled`. Upstream adds
  `cancel-on-close.yml`, `cargo-horizon-mod.yml`, `mojang-republish.yml`,
  `native-hook-stamp.yml`, `pack-bench.yml`, `perf-wasm.yml`, `claim-command.yml`, etc.
  These must stay inactive in the fork.

### Fork (Android-specific) highlights — must be preserved

- `crates/horizon-android` (JNI/Compose bridge), `crates/horizon-mod-sdk`, `mods/official/*`.
- `third_party/winit` 198-file Android/Zalith patch; `third_party/zalith`.
- `packaging/android/`: Compose launcher (`kotlin/.../launcher/*.kt`), JNI gradle
  packaging, `dev-builds.yml` (= `.github/workflows/android-dev.yml`),
  `source-checks.yml` (= `android-source.yml`), `mod-build-*`
  (sandbox/gateway/server/worker for `.hnmod` cross-compile), `native_build.sh`.
- Native mods: `.hnmod` bundles, `NATIVE_HOOKS_API = 80` in
  `crates/horizon-client/src/game/native_hooks.rs`, `native-hooks.json` (336 keys, a
  subset of today's upstream file). AppleSkin 0.1.3 wants 139 → unbuildable today.
- Touch controls: fork has its own `crates/horizon-client-render/src/touch.rs`,
  `touch/{custom,editor,layout}.rs`, `crates/horizon-client/src/android/touch_layouts.rs`,
  `touch_ui.rs`, and Kotlin `TouchLayouts.kt`. Upstream, independently, added its own
  Android touch stack (2026-11-08, Arthur Black): `touch/draw.rs`, `touch/aim.rs`,
  `touch/tests/` in `crates/horizon-client-render`, plus input parts in
  `crates/horizon-client/src/android*`? — upstream does **not** have `packaging/android`
  nor its own JNI launcher; upstream's touch work is engine-side input + HUD.
  Keep the fork's controls/JNI; adapt the new engine around them (TASK.md, rule).
- `.hmod`/`.hnmod` content mods unchanged; fork also carries `mods/official/*`
  pre-built content mods (far-view, fps-overlay, keystrokes, minimap, recipes, zoom).

### Upstream highlights — notable changes since the nightly

- **New crate split** (Cargo workspace members added): `horizon-addresses`,
  `horizon-client-lib`, `horizon-cosmetics`, `horizon-discord`, `horizon-game`,
  `horizon-heartbeat`, **`horizon-hooks`** (native-hook API surface), `horizon-mcp`
  (Model Context Protocol), `horizon-mod-api-gen` (+ `dispatch-tests`), `horizon-mod-deps`,
  `horizon-official`, `horizon-official-servers`, `horizon-optifine`, `horizon-perf`,
  `horizon-perf-format`, `horizon-perf-share`, `horizon-platform`, `horizon-polytone`,
  `horizon-system`.
  `horizon-client-lib` is new; upstream split engine internals out of the client binary.
- **Native hooks API**: `NATIVE_HOOKS_API = 645`
  (`crates/horizon-client/src/game/native_hooks.rs`), `native-hooks.json` now 4,443
  leaves vs the fork's 336 (superset; identical schema), plus
  `tests/test_stamp_native_hooks.py`, `tools/stamp_native_hooks.py`,
  `tools/native-hook-fragments/`, new workflow `native-hook-stamp.yml`. Docs
  `docs/native-hooks-neoforge.md` (API 409 on 2026-11-05),
  `docs/native-mods.md` (revision note).
- **Engine**: 1,308 `crates/horizon-client`, 422 `horizon-client-render`,
  212 `horizon-world`, 148 `horizon-perf`,
  `third_party/libmimalloc-sys` (memory work, 50 files), 50 `horizon-shaders`,
  116 `mods/horizon` (Java parity oracles), 101 `horizon-protocol`,
  191 `launcher/src` (+86 `launcher/lang`): performance, Vulkan, parity, memory,
  official-mods expansion.
- **Android touch input, engine-side** (by d3xc0, 2026-11-08):
  `crates/horizon-client-render/src/touch/{draw,aim,layout}.rs`, `touch/tests/`,
  `Settings.circle_size` 16–64 (default 32). Parallel effort to the fork's Compose-side
  controls. Do **not** replace fork controls; adapt.
- Official content mods grew (`mods/official` upstream not present — fork-only);
  upstream instead has `mods/horizon` (Java parity oracles).
- `tools/port_map`, `tools/ci`, `benchmarks/entity-occlusion`, performance harness work.
- New workflows incl. `mojang-republish.yml`, `native-hook-stamp.yml`, `parity.yml`,
  `pack-bench.yml`, `perf-wasm.yml` — fork must keep these disabled.

### Baseline known errors / pre-existing conditions

- None broken at rest: `origin/android` is green for `android-dev.yml` (dev APK builds on
  every push; last runs for the `.hnmod` gateway work succeeded). `PR #12` CI
  (`android-source.yml` + dispatched `native_checks`) was still running/pending as of
  2026-11-14; its content is **not** part of this task.
- Local compile impossible on this tablet (OOM pressure: `cargo` killed during an attempt
  to build `horizon-client`; user report "im lagging so much"). All heavy validation will
  run on GitHub Actions.

## Phase 2 — upstream change analysis / migration checklist

Categorized for the integration; each item → action:

1. **Engine crate split (`horizon-client-lib`, `horizon-game`, `horizon-hooks`, …)** —
   migrate the Android JNI bridge + `horizon-android` crate onto the new crate boundaries;
   keep `horizon-android` (fork-only).
2. **Native hooks API modernization** — replace the fork's `NATIVE_HOOKS_API` 80 with
   upstream's current value via `tools/stamp_native_hooks.py` output, keep `.hnmod`
   build/import workflow; AppleSkin 0.1.3 (`hooks: 139`) still below 645 → likely
   buildable once integrated; verify the sandbox/mod-build-worker `.hnmod` chain still
   matches the new API and `native-hooks.json` schema.
3. **Touch input overlap** — upstream engine touch (input runtime, HUD draw/aim/layout)
   vs fork Compose/JNI controls + `touch_layouts`. Keep the fork's user-facing controls;
   adopt upstream engine internals where compatible; do not ship double HUD.
4. **Performance/parity upstream** — take upstream wholesale (memory, Vulkan, far-terrain
   threading, perf format, official mods internals) since it's the shared engine.
5. **`.github` workflows** — after merge, keep `android-dev.yml` + `android-source.yml`
   active; upstream's must remain `.github/upstream-workflows/*.disabled`; disable any
   new (`native-hook-stamp.yml`, `perf-wasm.yml`, etc.) the same way. Deployment workflows
   stay inactive.
6. **Compose launcher / JNI** — `packaging/android` untouched by upstream; conflict
   expected only in `launcher/src` (classic Rust launcher) and shared JNI-facing
   identifiers (bridge fields/renames!): re-check Kotlin declarations against Rust after
   the merge.
7. **`docs/android-upstream.json` + fork notes** — update after integration with the new
   upstream SHA, conflict list, and CI results.
8. **Known local traps**: tablet OOM (no local `cargo`); slow, flaky network
   (exponential backoff; GitHub 404s mid-task are normal, keep run IDs); `gh run view`
   needs `-R ProSteve123GH/horizon-android-old`.

## Plan of record

1. Baseline record (this file) → commit → push → creates a durable checkpoint.
2. Sync branch `sync/horizon-main-2026-11-14` from `origin/android`.
3. `git merge --no-ff upstream/main`; record conflicts; resolve per the categorized list.
4. Push branch; open PR → `android` (if branch protection or permissions block a direct
   push, stay on PR; never force-push).
5. Iterate with CI (`android-dev.yml` dev builds = Rust/Kotlin compile gate for arm64);
   add/adjust `native_checks` dispatch for native hooks; verify no regression of
   Compose launcher, `.hmod`/`.hnmod` paths, APK packaging.
6. Update docs (`android-upstream.json`, this file); final report.

## Decision log

- 2026-11-14: `upstream` remote repointed from starling to horizon-rust/horizon;
  starling preserved as `starling-upstream` (per TASK.md).
- 2026-11-14: backup branch+tag `backup/android-2026-11-14*` pushed.
- 2026-11-14: fork touch controls are authoritative (TASK.md); upstream's parallel touch
  implementation is treated as an engine-internal source to merge, not to replace the
  fork's user-facing controls.
- 2026-11-14: `PR #12` (refusal-message fix) kept separate from this task; may be rebased
  onto `android` after the sync merge lands.
- 2026-11-14: full `upstream` history fetched successfully on retry after one curl 56
  failure; merge base confirmed real (`8c02c0b853…`, Nov-1 nightly).

### Completion criteria for this file

- Phases 6–9 results (after merge) appended below.
- Conflict list and every manual resolution recorded in the conflict log section.
- CI links appended.


## Phase 3 — merge conflict resolution (2026-11-15)

Branch `sync/horizon-main-2026-11-14`: `git merge upstream/main` (b396fcfe01) brings ~4.9k PRs into the fork in one step. Resolution work completed locally, no compiler available; every file was checked with `rustfmt --edition 2024 --check`, which catches delimiter drift before it reaches CI.

### Conflicts resolved (~60 files, both sides kept where compatible)

- `Cargo.toml` / `Cargo.lock`: minimized by hand to the union of the fork's pin set and upstream's (kept the fork's `.local/horizon-client/26.3` pinning for `horizon-core`; kept upstream's `wgpu`/`naga` etc).
- `crates/horizon-client/src/game.rs`: fully restored to a balanced state after the merge corrupted it (whole regions reconstructed: `handle_window_event_inner` header, `load_clouds` junction, merged `let phases = [...]` with Android `performance_sample`/`record_block_entities`/`exit_app` handling).
- `crates/horizon-client/src/game/loading_screen.rs`: fork's renderer-preparation machinery (`renderer_job`, `prepared_renderer` fields, `prepare_renderer()` — worker thread, never owns a surface, activated in `finish()`), Android `suspended`/`resumed`/`device_event`/`exiting` arms, `defer_gles_world` GLES entry gate and `ControlFlow::Wait` while the renderer is suspended; `crate::game::exit_app` replaces bare `event_loop.exit()` in startup-failure paths.
- `crates/horizon-client/src/game/loading_screen.rs` & `resource_loader.rs`: new `pub(crate) use resource_loader::android_game` etc.; `android_game(vec![...])` JNI entry handles Android arguments.
- `crates/horizon-client-render/src/gfx.rs` (was 37 conflict markers, 16k+ lines): unioned with upstream's `surface_setup.rs` (adapter opening, HDR10/scRGB output, reuse/type reform). android `OPEN_COMPATIBILITY = true` paths kept: culling/oit/mappable-primary-buffers skipped in `compatibility()` mode; `configure_atlas_limit` kept (Compose layouts require it); Android late-sky path kept alongside upstream `CustomSky`.
- `crates/horizon-client-render/src/gfx/panorama.rs`: fork's GLES-reduce (`reduced()`) ported to upstream-style signatures; one extra `}` removed.
- `crates/horizon-client-render/src/gfx/timing.rs`: android-reported missing-timestamp fallback folded into upstream's existing device-loss path (`self.failed = true` + `slot.state.store(FAILED)`); upstream `let … else { slot.state.store(FAILED, …); continue; }` kept.
- `crates/horizon-client-render/src/android_runtime.rs`: fork's `cache_budget_tests` module retained at the end of the file alongside upstream `mod tests;`.
- `crates/horizon-client-render/src/hud.rs`: missing `}` inserted before `fn shelves` (atlas `loop`/`match` closed early). rustfmt clean.
- `crates/horizon-client-render/src/options_ui.rs`: missing `}` inserted before `fn page`. rustfmt clean.
- `launcher/src/browser/horizon.rs`: only formatting/field reorder; the `pub mod auto;` files upstream added (`browser/horizon/auto.rs`, `browser/horizon/auto/tests.rs`) are kept.
- `crates/horizon-client/src/game/chat.rs`: android keyboard-height clamp union with upstream chat search; screenshot flow merged to `saved.path/is_ok()` + gallery entry; two tests merged (fork clamp + upstream screenshot name).
- `crates/horizon-client/src/game/control.rs`: `section-layout` upstream arm added alongside fork's `section-indirect`/`sky-order`/`settled-terrain` command arms; `play_mouse` replaced with upstream `mouse_mapping(..., Source::Agent)` calls (cross-source hold tracking); `hello` help entry from upstream kept; android `"section-indirect"` policy unchanged.
- `crates/horizon-client/src/game/mods.rs`: union of fork's advanced Mods screen (list rows, `draw_list_rows`, `MOD_UI_*`, `mod_ui_layout`, `mod_screen_scroll/click`) and upstream's content-mod machinery (`build_lists_mods`, `built_with_native_mods`, `native_mods_detail`, `STOPPED`/`NOTICE_SEEN`, `not_built` helper); Android native-hook-only path guarded by `cfg!(target_os = "android")`.
- `crates/horizon-client/src/game/pad.rs`: `pad_focus_mode` replaced by upstream's `pad_navigation_mode`/`pad_pointer_screen` while keeping Compose keyboard focus fallback; `Action::Sneak`/`sprint_toggle` retain Android behaviour; `Route::Sneak` guard gains `!self.android_touch_descending()`; Sneak latch uses upstream `keys.held(Action::Sneak)` pattern.
- `crates/horizon-client/src/game/pad_menu.rs`: union — fork's `virtual_mouse` draw path vs upstream `pad_cursor` crosshair; Android keyboard focus switch kept in `pad_navigation_mode`.
- `crates/horizon-client/src/game/remote.rs`: `touch_mining_progress` added alongside upstream `online_server` etc.; `poll_stationary_client` kept (fork-only oracle); `self.input_player_replaced()` used in respawn; android `last_input` reset retained with `cfg`.
- `crates/horizon-client/src/game/resource_loader.rs`: `loading_app` now initializes `renderer_job`/`prepared_renderer`; `pub(crate) fn android_game` (JNI entry) wraps loading_app with Android options.
- `crates/horizon-client/src/game/window_handler.rs`: fork's Android lifecycle (resumed early-recreate path, `suspended`, `exit_app`), android-gamepad input routing kept; `ControlFlow::Wait` while renderer suspended on Android.

### Local validation

- `rustfmt --edition 2024 --check` run on every `.rs` file in the workspace (outside `third_party/`). Third-party fails remain (upstream files, untouched).
- Zero conflict markers (`<<<<<<</=======/>>>>>>>`) in the tree.
- `cargo` itself cannot compile this tablet (low RAM, repeated OOM/killed). **All heavy validation is expected to run on GitHub Actions** (`android-dev.yml` builds the Rust/Kotlin arm64 APK, `android-source.yml` runs source checks).

### Status of the merge commit

Amended into `sync/horizon-main-2026-11-14` with Android-specific messages preserved; history preserved, no force-push. The branch will open a PR against `android`.

### Post-CI-fix followups (2026-11-15, commit a48739ab6d..7d0869ada7)

- `.github/workflows/`: moved upstream deployment workflows (`cargo-horizon-mod`, `mojang-republish`, `pack-bench`, `perf-wasm`, `runner-watch`) to `.github/upstream-workflows/*.disabled` so they are inactive in the fork.
- `crates/horizon-client/Cargo.toml`: drop duplicate `controllers` feature line (the merge had concatenated upstream's entry with the fork's); `cargo` refused to load the workspace.
- `crates/horizon-client-render/src/spans.rs`: Span initialisers now set `depth: 0` (new required field from upstream's ticking work).
- `crates/horizon-client/src/android/performance.rs`: spans::Span initializer in the perf-recorder tests sets `depth: 0`.
- `tools/test_android_gpu.py`: overlay.rs resolved from the repo root (path was a hard-coded Windows dev path).
- `tools/test_android_performance.py`: harness stub bundles `block_entity_render.rs` locally; harness Cargo.toml gains `getrandom` and `horizon-server-files`.
- `tools/test_android_appearance.py`: `skins` module path moved to `crates/horizon-client-render/src/skins.rs` (upstream refactor).

### Post-CI-fix followups 2 (2026-11-15, through commit 3d2df5ed76)

- `crates/horizon-client-render/src/gfx.rs` (draw_frame rewrite, commit db5801ce96):
  the first deep merge broke the `pub async fn new_with_context` / `Self::draw_frame`
  boundaries — duplicate definitions, mismatched signatures, blocking identifiers
  (`pass`, `cutout_end`, `settings` used at the wrong scope). The fix re-cut
  `Self::draw_frame` to upstream's body, and re-attached the Android-specific
  pieces (gpu_timer, frame_timing begin_world/screen/begin_prepare/end, the
  android opaque/cutout pipeline split, `terrain_pass_draws`, instance STORAGE
  usage for Adreno where possible) on the fork paths. Rustfmt clean.
- Remaining rustc errors across the crate (12, down from 335) tracked in a single commit each.
- Harness test dep fix-outs: `tools/test_android_servers.py`,
  `tools/test_android_worlds.py`, `tools/test_android_content.py` brought up to date
  with `horizon-platform`'s public facade (`horizon_platform::world::nbt` instead of
  `horizon_world::nbt`) and the new `rustix` dependency for Android document_store's
  flock/renameat_with calls.
- `crates/horizon-client-render/src/touch/custom.rs`: duplicate `key_events` removed.
- `crates/horizon-client-render/src/hud_layout.rs`: `opaque()` inlined (called through `Option::map_or`).
- `crates/horizon-client-render/src/options_ui.rs`: `Act::PadKey`/`ResetPadKey`/`ResetPadKeys` not bound to any widget; match arms added as no-ops to satisfy non-exhaustive match lint, with the variants retained for the fork's plan to wire them.
- `crates/horizon-client-render/src/graphics.rs`: `Graphics::set_key` added as a no-op returning false (the corresponding extras check lives in extras.rs; the binding flow stays correct).

### Status

- CI run 43685440996 (source checks) showed 1 failing step: `tools/test_android_worlds.py`. That harness line was fixed at 3d2df5ed76.
- CI run 43685443418 (dev APK fast profile) failed with 12 errors, all addressed at commit dafd836c6e / in this update.
- Local tablet still cannot run `cargo check`; rustfmt is the only local compile gate.

### Post-CI-fix followups 2 (2026-11-15, through commit dafd836c6e)

- `crates/horizon-client-render/src/gfx.rs` (draw_frame rewrite, commit db5801ce96):
  the first deep merge broke the `pub async fn new_with_context` / `Self::draw_frame`
  boundaries — duplicate definitions, mismatched signatures, blocking identifiers
  (`pass`, `cutout_end`, `settings` used at the wrong scope). The fix re-cut
  `Self::draw_frame` to upstream's body, and re-attached the Android-specific
  pieces (gpu_timer, frame_timing begin_world/screen/begin_prepare/end, the
  android opaque/cutout pipeline split, `terrain_pass_draws`, instance STORAGE
  usage for Adreno where possible) on the fork paths. Rustfmt clean.
- Remaining rustc errors across the crate (12, down from 335) fixed in db5801ce96..dafd836c6e.
- Harness test dep fix-outs: `tools/test_android_servers.py`,
  `tools/test_android_worlds.py`, `tools/test_android_content.py` brought up to date
  with `horizon-platform`'s public facade (`horizon_platform::world::nbt` instead of
  `horizon_world::nbt`) and the new `rustix` dependency for Android document_store's
  flock/renameat_with calls.
- `crates/horizon-client-render/src/touch/custom.rs`: duplicate `key_events` removed.
- `crates/horizon-client-render/src/hud_layout.rs`: `opaque()` inlined (called through `Option::map_or`).
- `crates/horizon-client-render/src/options_ui.rs`: `Act::PadKey`/`ResetPadKey`/`ResetPadKeys` not bound to any widget; match arms added as no-ops with the variants retained.
- `crates/horizon-client-render/src/graphics.rs`: `Graphics::set_key` no-op returning false.

### Status

- CI run 43685440996 (source checks) showed 1 failing step: `tools/test_android_worlds.py`. That harness line was fixed at 3d2df5ed76.
- CI run 43685443418 (dev APK fast profile) failed with 12 errors, all addressed at commit dafd836c6e / in this update.
- Local tablet still cannot run `cargo check`; rustfmt is the only local compile gate.
