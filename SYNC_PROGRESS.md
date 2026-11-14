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
