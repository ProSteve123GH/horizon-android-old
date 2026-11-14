# Horizon Android's mod builds

Native mods are compiled into Horizon's Android game library, and a phone can't do that itself. When a
player turns on **Compile on GitHub**, Horizon's gateway asks this repository to compile the game with
their mods, and the phone downloads the result.

This folder is a template: copy it into a **separate** public repository (for example
`<you>/horizon-mod-builds`). Keeping the runner in its own repository means a mod's code, which runs
while it compiles, can read only that repository's contents, never the game's.

- `.github/workflows/build.yml` compiles one player's mods. The mods come from the gateway under a
  random name, compile in a sandbox with no network, and the library goes back as a release asset
  (`build-<ask>`) the phone downloads — a public asset needs no token, where a run's artifact does.
- `.github/workflows/base.yml` makes the clean build cache for one commit of the game. It only runs
  trusted code, and it is the only job that writes the cache.
- `.github/workflows/cleanup.yml` deletes build releases older than four days; the gateway forgets a
  job after three, so a phone that asks again builds again anyway.
- `versions.json` says which commit each released version of the game is. The launcher sends its own
  commit for builds made by `tools/android_build.py`, so a dev build doesn't need an entry.
- `scripts/prepare.sh` fetches the commit, its build scripts and the mod sandbox.
- `scripts/tools.sh` installs Rust, the Android SDK and NDK, the crates and `horizon-mixins`.
- `scripts/room.sh` adds swap and frees the runner's unused toolchains.
- `scripts/publish.sh` puts the library in a release and tells the gateway the job is done.

## The gateway

Something has to hold the token that runs this repository's workflows, and the phone can't. Two
gateways speak the same protocol, so the launcher can't tell them apart:

- **`packaging/android/mod-build-worker/`** — a Cloudflare Worker. Free, no card, nothing to keep
  running: the token is its secret, the requests live in its KV, and the library is downloaded from
  the release. This is the one to pick if you have no server.
- **`packaging/android/mod_build_gateway.py`** — a small machine of your own, which can also run
  builds itself (`mod_build_server.py`) for players who don't want to wait for GitHub. It keeps the
  artifact and serves it from its own disk.

Either way the gateway needs a token that may run this repository's workflows (fine-grained:
**Actions: write**; **Contents: read**), and this repository must be public so the release assets can
be downloaded without one.

## Setting it up

1. Create a public repository from this folder.
2. Add the version you release to `versions.json` (`{"0.7.7": "<full commit>"}`), or rely on the
   launcher's commit.
3. Run **base** once for that commit (Actions → base → Run workflow). It takes a while and fills the
   cache every player's build starts from.
4. Put a gateway in front of it: deploy `mod-build-worker/` (`GITHUB_REPO` = this repository, and
   `GITHUB_TOKEN` as its secret), or run `mod_build_gateway.py` with `GITHUB_BUILDS_REPO` and
   `github-token` in its `CREDENTIALS_DIRECTORY`, plus a public URL (`GATEWAY_PUBLIC_URL`).
5. In Horizon: **Settings → Native mods → Build server** = the gateway's URL, and leave **Compile on
   GitHub** on.

The same scripts run on a machine of your own: `mod_build_server.py` is the direct service, and
`mod_build_gateway.py` is the door in front of it.
