# Removing `.hnmod` mod-building support (Android fork)

Branch: `fix/android-remove-hnmod-build` (off `android`).

## Why

Upstream retired the `.hnmod` native-mod package format in PR #16941 ("mod API merge",
merged 2026-11-16). The decisive commit is `f6f6d205cea3bab5cb44a4b81ec34883348a17a9`,
"Use hmod for all Horizon mod packages". Follow-ups:

- PR #17992 / merge `3d084a888a06aded69c1d2ef459aefe41e8895c7` (2026-11-17):
  retired `.hnmod` files are listed only so the player can remove them.
- PR #18267 / `b33743bf6f9ab978a86610c0ddfe6a8b21cbb51a` (2026-11-18):
  dropped `.hnmod` files get an explicit "old format, no longer loads" message.
- Upstream issue #17934: launcher 0.2.70 retired the format.

So `.hnmod` is no longer a supported way to build a mod. Keeping a second,
incompatible build pipeline in this fork would only mislead players.

## The new SDK (research result)

The old WIT-based `crates/horizon-mod-sdk` no longer exists upstream. Mods are now
written against `horizon-mod-api` (lib name `mod_api`), with shared hooks, Parts,
attachments and dependencies for native and WebAssembly targets; `horizon-mod-vocab`,
`horizon-mod-api-gen`, `horizon-mod-api-macros`, `horizon-mod-host`, `horizon-mod-points`,
`horizon-mod-runtime`, `horizon-mod-surface` and `horizon-mod-package` make up the
system. Every mod ships as one `.hmod` holding `horizon-mod.toml`, `assets/`, `data/`,
and either Rust source or `mod.wasm`.

Authoring (upstream `docs/modding/src/start/first-mod.md`):

```sh
cargo horizon-mod new greeter --template native   # or --template wasm
horizon-launcher native-build . --instance "My Instance" --trust-native-code
```

The launcher compiles against a **signed prebuilt SDK for the exact pinned game
build**; the game itself is never rebuilt for a mod. The SDK is published per build
as tag `sdk-<full commit sha>`.

**Android:** upstream publishes no native SDK slice for Android. `tools/ci/nightly.py`
lists only `x86_64-unknown-linux-gnu` (client and server), `x86_64-pc-windows-gnu`
and `aarch64-apple-darwin`. `docs/modding/src/reference/sdk/06-hosts.md` says an
unsupported host gets `E_NATIVE_HOST`. There is no upstream claim that the Wasm
runtime ships on Android. This fork's hosted-runner compile flow therefore has no
upstream equivalent and is removed here.

## Scope of this change

Remove `.hnmod` **build/packaging** support in all three places:

1. `cargo horizon-mod`: no `--template native`, no `.hnmod` check kind.
2. Launcher: no `native-build` command, no `.hnmod` pack path, no game-rebuild pipeline.
3. Android: no in-game Compile/Recompile button, no hosted build/submit/download pipeline.

Explicitly preserved:

- Runtime loading, listing and toggling of already-installed mods (`mod_list.rs`,
  `native_status.rs`, Android import/enable code).
- The native host API and dispatchers (`native_on`, `native_setting`,
  `native_set_setting`, `native_mod_key`, `NativeMod`, tick/HUD/key/frame dispatchers).
  Built-in Android features and the standalone Simple Voice Chat use these; deleting
  them would remove voice chat's on/off switch, settings, keybinds and per-frame driving.
- The `crates/horizon-mods` `.hnmod` reader, so old installed files can still be
  displayed/removed rather than silently breaking a save.

Porting the new `mod_api` SDK to this fork is a separate, much larger project and is
not attempted here.
