# Horizon Android's mod builds

Native mods are compiled into Horizon's Android game library, and a phone can't do that itself. When a
player turns on **Compile on GitHub**, Horizon's gateway asks this repository to compile the game with
their mods, and the phone downloads the result from the gateway.

This folder is a template: copy it into a **separate** public repository (for example
`<you>/horizon-mod-builds`) and set the gateway's `GITHUB_BUILDS_REPO` to that repository. Keeping the
runner in its own repository means a mod's code, which runs while it compiles, can read only that
repository's contents, never the game's.

- `.github/workflows/build.yml` compiles one player's mods. The mods come from the gateway under a
  random name, compile in a sandbox with no network, and the result goes back as the run's artifact.
- `.github/workflows/base.yml` makes the clean build cache for one commit of the game. It only runs
  trusted code, and it is the only job that writes the cache.
- `versions.json` says which commit each released version of the game is. The launcher sends its own
  commit for builds made by `tools/android_build.py`, so a dev build doesn't need an entry.
- `scripts/prepare.sh` fetches the commit, its build scripts and the mod sandbox.
- `scripts/tools.sh` installs Rust, the Android SDK and NDK, the crates and `horizon-mixins`.
- `scripts/room.sh` adds swap and frees the runner's unused toolchains.

## Setting it up

1. Create a public repository from this folder.
2. Add the version you release to `versions.json` (`{"0.7.7": "<full commit>"}`), or rely on the
   launcher's commit.
3. Run **base** once for that commit (Actions → base → Run workflow). It takes a while and fills the
   cache every player's build starts from.
4. Give the gateway a token that may run this repository's workflows (`GITHUB_BUILDS_REPO`, and
   `github-token` in its `CREDENTIALS_DIRECTORY`), and a public URL (`GATEWAY_PUBLIC_URL`) where the
   runner can fetch the request from `/v1/github/<ask>`.

The same scripts run on a machine of your own: `mod_build_server.py` is the direct service, and
`mod_build_gateway.py` is the door in front of it.
