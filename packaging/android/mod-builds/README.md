# Horizon Android's mod builds

Native mods are compiled into Horizon's Android game library, and a phone can't do that itself. When a
player presses **Build**, Horizon's gateway (a Cloudflare Worker or `mod_build_gateway.py`) asks this
repository to compile the game with their mods, and the phone downloads the result.

This folder is a template: copy it into a **separate public repository** (for example
`<you>/horizon-mod-builds`). Keeping the runner in its own repository means a mod's code, which runs
while it compiles, can read only that repository's contents, never the game's.

## 1. Make the repository

1. On GitHub, create a new **public** repository, for example `you/horizon-mod-builds`. Public
   matters: the phone downloads the finished library from a release asset without a token.
2. Copy everything in this folder into it, including the hidden `.github/` folder, and commit it to
   `main`.
3. In `.github/workflows/base.yml` and `build.yml`, set `HORIZON_REPO` to the game repository's
   URL and `HORIZON_REPO_PATH` to its `owner/name`. The template already names this
   fork: `https://github.com/ProSteve123GH/horizon-android-old` and
   `ProSteve123GH/horizon-android-old`.
4. Add each release to `versions.json`:

   ```json
   {
     "0.7.7": "<the release's full commit>"
   }
   ```

   The launcher sends its own commit for builds made by `tools/android_build.py`, so a dev build
   does not need an entry; this is the fallback for a launcher that cannot name one.

## 2. Make the clean cache (once per commit)

Every player's build starts from a cache of a clean build of the game at one commit.

1. Open **Actions → base → Run workflow**.
2. `version` = a version in `versions.json` (`0.7.7`); `commit` = the release's full commit, or
   empty to use `versions.json`.
3. Wait about 15 minutes. The cache is keyed by the full commit; without it `build.yml` stops on
   the cache restore.

Run it again for a new game commit. `build.yml`'s cache key is the commit the launcher was built
from (a dev build's short commit is resolved to the full one first).

## 3. Put a gateway in front

Something has to hold the token that may run this repository's workflows; the phone cannot. Two
gateways speak the same protocol, so the launcher cannot tell them apart:

- **`packaging/android/mod-build-worker/`** — a Cloudflare Worker. Free, no card, nothing to keep
  running: the token is its secret, the requests live in its KV, and the library is downloaded from
  the release. This is the one to pick if you have no server. Its README has the full steps:
  `wrangler login`, `wrangler kv namespace create STORE`, `GITHUB_REPO` = this repository,
  `wrangler secret put GITHUB_TOKEN`, `wrangler deploy`.
- **`packaging/android/mod_build_gateway.py`** — a small machine of your own, which can also run
  builds itself (`mod_build_server.py`) for players who don't want to wait for GitHub. It keeps the
  artifact and serves it from its own disk.

Either way the gateway needs a fine-grained GitHub token for this repository with **Actions: write**
(the gateway dispatches the workflow and reads the run's artifact with it), and this repository must
be public.

## 4. Point Horizon at it

1. **Settings → Native mods → Build server** = the gateway's URL (the Worker's
   `https://<worker>.<account>.workers.dev`).
2. Leave **Compile on GitHub** on.
3. Open **Mods**, press **Build** on a native (`.hnmod`) mod, and wait for the run. The finished
   library appears in **Settings → Native mods → Builds**; press **Use**.

## What runs

- `.github/workflows/build.yml` compiles one player's mods. The mods come from the gateway under a
  random name, compile in a sandbox with no network, and the library goes back as a release asset
  (`build-<ask>`) the phone downloads — a public asset needs no token, where a run's artifact does.
- `.github/workflows/base.yml` makes the clean build cache for one commit of the game. It only runs
  trusted code, and it is the only job that writes the cache.
- `.github/workflows/cleanup.yml` deletes build releases older than four days; the gateway forgets a
  job after three, so a phone that asks again builds again anyway.
- `scripts/prepare.sh` fetches the commit, its build scripts and the mod sandbox.
- `scripts/tools.sh` installs Rust, the Android SDK and NDK, the crates and `horizon-mixins`.
- `scripts/room.sh` adds swap and frees the runner's unused toolchains.
- `scripts/publish.sh` puts the library in a release and tells the gateway the job is done. It
  posts with an explicit `User-Agent`: Cloudflare's edge refuses the default Python urllib one
  (error 1010), so without it a job's answer never arrives.

The same scripts run on a machine of your own: `mod_build_server.py` is the direct service, and
`mod_build_gateway.py` is the door in front of it.
