# Native mods on Android

Native mods (`.hnmod`) are Rust mixin bundles that change Horizon's own source before it is compiled
([native mods](native-mods.md)). A phone cannot compile the game, so the Android launcher sends the
mods that are turned on to a **build service**, downloads the library it makes, and the game loads that
library instead of the APK's own on the next start.

The launcher part lives in `crates/horizon-client/src/android/native_builds.rs`; the build service is
`packaging/android/mod_build_server.py` and `packaging/android/native_build.sh`. A gateway puts a
public door in front of it — `mod_build_gateway.py` on a machine of your own, or
`packaging/android/mod-build-worker/` on Cloudflare, which needs no machine at all — and either can
send every build to GitHub's runners instead of compiling it itself.

## In the launcher

- **Mods → Native mods → Import .hnmod** puts the file in the instance's
  `horizon-native-mods/` folder. Importing is a file copy; nothing is built or run.
- The card lists each native mod with a switch. A mod is on until it is turned off. Removing it deletes
  its file; a build that already has it keeps it until the next build.
- **Build** sends the mods that are on to the server named in **Settings → Native mods → Build server**.
  The build card shows the line, the compile, and the download. **Cancel** stops waiting; the server
  keeps compiling and the result is not downloaded.
- A finished build is chosen automatically. **Use** chooses it again after it was set aside (see below),
  resetting that count; **Stop using** makes the next start run the APK's own library; **Delete** removes
  the build from the phone.
- A build is only loaded under the APK version name and the install it was made for (the package
  manager's `lastUpdateTime`). Updating the APK makes every build wait for a new build, even at the
  same version name: another install has another commit's code.
- A build that closes twice before its first frame is set aside, and the launcher says so; the APK's
  own game plays. Choosing the build again after a new download starts the count afresh.

The build server address is a launcher setting, not a game setting: `https://host` or `http://host:port`,
no path. Cleartext `http://` is accepted for a server on the same network; the app's manifest still
refuses cleartext for everything else.

## The build service

The direct service is Python's standard library only. A machine that builds needs:

- a clone of this repository at `~/horizon-android-build/source` (the commit to build must be in it),
- Rust with the `aarch64-linux-android` target,
- the Android SDK at `~/android` with an NDK and build-tools (`ANDROID_HOME`),
- a `CARGO_HOME` at `~/horizon-android-build/cargo` with the crates fetched, and
- `horizon-mixins` installed for the launcher's pinned revision
  (`MIXINS_REV` in `launcher/src/native_mods.rs`).

`mod_build_server.py` and `mod_build_sandbox.sh` are started on the same machine:

```sh
export MOD_BUILD_ROOT=$HOME/horizon-android-build
python3 packaging/android/mod_build_server.py
```

It listens on `MOD_BUILD_PORT` (8787 by default) and speaks:

- `POST /v1/build?version=V&commit=C` — the body is, for each mod, a 4-byte big-endian length and the
  `.hnmod` file. The answer is `{"job": "<24 hex>"}`, or `{"starting": true}` while a sleeping gateway
  wakes. A direct server refuses `via=github`.
- `GET /v1/job/<id>` — `queued` (with `ahead`), `building` (with `seconds`), `done` (with `sha256`,
  `size` and `build`), or `failed` (with `error`).
- `GET /v1/job/<id>/lib` — the library; `Range: bytes=N-` resumes.
- `GET /v1/busy` — whether a build is under way or in line.

`commit` is the commit the APK was built from: the full SHA, which `tools/android_build.py` embeds
beside the short `HORIZON_ANDROID_BUILD_COMMIT` the launcher shows. A build made without it (a source
export) sends none, and the service reads `versions.json` in its root, `{"0.7.7": "<full commit>"}`,
for the version name.

Each build runs in `mod_build_sandbox.sh`: a mount, PID and network namespace with no network, a
read-only source and crates, a copy-on-write cache layer that is thrown away, and no privileges. The
build's scripts are the mod's code, so they must not reach the machine's files or keys. The base cache
(`MOD_BUILD_BASE=1`) is the exception: it only builds trusted code and is the layer every player's
build starts from. `MOD_BUILD_UNSANDBOXED=1` skips the sandbox for a machine that builds nobody else's
mods; do not use it on a service other people can reach.

The service keeps 12 results, one build at a time, 8 in line, and 16 mods in one request. A build
times out after 15 minutes (`BUILD_SECONDS`), and the sandbox after `MOD_BUILD_SECONDS` (870 by
default). The same mods for the same commit make the same job, so a second request gets the first
one's library or its error.

### The first build

Before the first build, fill the base cache once (trusted code only):

```sh
# From the server's working copy, with the server's CARGO_HOME.
export CARGO_HOME=$HOME/horizon-android-build/cargo
(cd "$HOME/horizon-android-build/source" && cargo fetch --locked)
rev=$(sed -n 's/^pub const MIXINS_REV: &str = "\([0-9a-f]*\)";/\1/p' \
  "$HOME/horizon-android-build/source/launcher/src/native_mods.rs")
cargo install --locked --git https://github.com/horizon-rust/horizon-mixins --rev "$rev" \
  --root "$HOME/horizon-android-build/base/mixins-${rev:0:12}" horizon-mixins-cli

# Then build the plain game into the cache. The sandbox has no network, so the tool above is
# already in the cache it mounts.
mkdir -p "$HOME/horizon-android-build/jobs/base/out" "$HOME/horizon-android-build/jobs/base/mods"
commit=$(git -C "$HOME/horizon-android-build/source" rev-parse HEAD)
MOD_BUILD_BASE=1 MOD_BUILD_SECONDS=3600 \
  packaging/android/mod_build_sandbox.sh "$HOME/horizon-android-build" "$commit" \
  "$HOME/horizon-android-build/jobs/base"
```

The base build writes `base/build-arm64-v8a/` (the source, the Cargo target and the library). Every
later build overlays it, so only the crates a mod changes are recompiled. A release should be built
from the same commit its `versions.json` names.

## The gateway

Something has to hold the token that runs the builds repository's workflows, and a phone can't. Two
gateways speak the same protocol, so the launcher can't tell them apart: `mod_build_gateway.py` on a
machine of your own, or `mod-build-worker/` on Cloudflare (below). Neither is a build machine when
every build goes to GitHub.

### On Cloudflare, without a machine of your own

`packaging/android/mod-build-worker/` is a Cloudflare Worker: free, no card, nothing to keep running.
It keeps the requests in KV, dispatches the builds repository's `build.yml`, and points the phone's
download at the release the run published. A release asset of a public repository needs no token,
where a run's artifact does, so the phone downloads through the Worker, which keeps the `Range` across
GitHub's redirect. A finished job stays known for three days: the same mods on the same game are then
no build at all.

Its bindings are `STORE` (KV), its secret `GITHUB_TOKEN` (fine-grained: **Actions: write** on the
builds repository), and its vars `GITHUB_REPO`, `GITHUB_WORKFLOW` (default `build.yml`), `GITHUB_REF`
(default `main`), `PER_DAY` and `PER_ADDRESS`. `npx wrangler kv namespace create STORE`,
`npx wrangler secret put GITHUB_TOKEN`, `npx wrangler deploy`, then set the launcher's **Build server**
to the printed `https://….workers.dev` URL. **Compile on GitHub** only steers a gateway that can also
run builds itself (`mod_build_gateway.py`): the Worker always uses GitHub's machines, and the launcher
waits the same 90 minutes for either.

The Worker's protocol has its own test
(`node --test packaging/android/mod-build-worker/test.mjs`), which the Android source checks run.

### `mod_build_gateway.py`

`mod_build_gateway.py` is the public address in front of a machine that may sleep, and the door to
GitHub's runners. It keeps the same protocol as the direct service and adds:

- `GET /v1/github/<ask>` — the mods of a job, fetched by the GitHub workflow. The name is random and
  short-lived; the body is the request's bytes.
- `GET /v1/busy`, `/v1/job/<id>` and `/v1/job/<id>/lib` — answered from its kept results or the
  backend.
- Limits per address and per day, a queue, a minimum free disk, and kept builds per request.

Environment:

- `BUILD_BACKEND` — the direct service's URL (required; the gateway parses it at start).
- `BUILD_VM` — the Azure VM resource id to wake. Empty means an always-on backend; the gateway then
  refuses a build only when it can't reach one.
- `GATEWAY_PUBLIC_URL` — the gateway's own URL, passed to the workflow and used to fetch the mods.
- `GATEWAY_PORT` — 8788 by default.
- `STATE_DIRECTORY` — where kept results stay; without it, nothing is kept and GitHub builds are off.
- `STORE_BYTES` — how much they may take, 10 GiB by default.
- `BUILDS_PER_DAY` — builds a day per address (and for everyone together), 80 by default.
- `GITHUB_BUILDS_REPO` — `owner/name` of the **separate** builds repository
  (`packaging/android/mod-builds/`). Empty turns GitHub builds off.
- `GITHUB_VERSIONS` — comma-separated version names whose builds always go to GitHub.
- `GITHUB_AT_ONCE`, `GITHUB_LINE`, `GITHUB_BUILDS_PER_DAY` — 8 runs at once, 24 waiting, 300 a day.
- `CREDENTIALS_DIRECTORY` — systemd's `LoadCredential`; `github-token` in it is a token that may
  dispatch workflows and read artifacts in the builds repository. Without it, GitHub builds are off.

A player turns on **Settings → Native mods → Compile on GitHub** to send `via=github`; the gateway
then dispatches the builds repository instead of waking its backend. A build for a release version
may also be routed by `GITHUB_VERSIONS` (a comma-separated list of version names), which is how a
maintainer can make GitHub the only builder for a release.

## The builds repository

`packaging/android/mod-builds/` is a template for a separate public repository. Copy its contents
into one (for example `<you>/horizon-mod-builds`), set `HORIZON_REPO`/`HORIZON_REPO_PATH` in the two
workflows to this fork, run **base** once for the release commit, and point the gateway's
`GITHUB_BUILDS_REPO` at it. The runner checks out the commit, fetches the build scripts from that
commit, compiles one player's mods in the sandbox, and leaves `status.json` and the library as the
run's artifact; a `publish` job then puts them in a `build-<ask>` release, which is what the phone
downloads, and tells the gateway. A `failed` job tells the gateway when the run dies instead, and
`cleanup` deletes releases older than four days. A mod's code runs while it compiles, so the token
only reads that repository, and the player's build never writes the base cache.

## Safety

A native mod is native code with the app's rights on the phone: it is not sandboxed there. Import only
mods whose authors you trust; the card says so before the first build. On the build machine the code
runs in the sandbox described above, with no network and a discarded cache. The mod's `.hnmod` is
checked before it is sent: paths, entry counts, size, the manifest and the mixin identity, and its
native hooks version against this game's `NATIVE_HOOKS_API`. A mod that needs a newer hooks version is
refused with the numbers, not built into a game that lacks the hooks.

## Troubleshooting

- **"Set the native mod build server first"** — no address in Settings → Native mods. A direct server
  is `http://<pc>:8787`; the gateway is its public `https://` URL.
- **"The build server has no game for Horizon …"** — the request had no commit and the server's
  `versions.json` has no entry for that version name. Add the release's commit, or use a build made by
  `tools/android_build.py`, which sends its commit.
- **"The build server can't be reached"** — check the address and that the server is listening; a
  phone on the same network needs the PC's LAN address, not `localhost`.
- **"GitHub refused the build"** — the gateway's token may not run the builds repository's workflows
  (fine-grained: **Actions: write**), or `GITHUB_REPO`/`GITHUB_BUILDS_REPO` names the wrong repository.
- **"GitHub did not start the build"** — the run was asked for and no machine took it within 25
  minutes; ask again. If it always says so, the builds repository's `build.yml` is not on the branch
  the gateway dispatches (`GITHUB_REF`, `main` by default).
- **The build is not used after an update** — the APK install changed; build again.
- **The game closed twice and the build is gone** — the build did not reach a first frame and was set
  aside. Remove the mod that caused it, or build again after fixing it. The launcher shows the notice
  once.

## Files

- `crates/horizon-client/src/android/native_builds.rs` — the launcher's state, request, polling and
  download.
- `crates/horizon-mods/src/index/native.rs` — the `.hnmod` reader and the bytes the service gets.
- `crates/horizon-client/src/android/documents.rs` — the SAF import into `horizon-native-mods/`.
- `packaging/android/kotlin/dev/horizon/android/NativeMods.kt` — the game's side: which build loads,
  the start count and the notice.
- `packaging/android/java/dev/horizon/android/HorizonActivity.java` — loads the chosen library and
  reports the first frame.
- `packaging/android/native_build.sh` — mixins, the staged source and
  `tools/android_build.py --rust-only`.
- `packaging/android/mod_build_server.py`, `mod_build_sandbox.sh`, `mod_build_once.py`,
  `mod_build_gateway.py` — the service, its sandbox, the one-shot GitHub build, and the gateway.
- `packaging/android/mod-build-worker/` — the same gateway on Cloudflare: the Worker, its
  `wrangler.toml`, and its protocol test.
- `packaging/android/mod-builds/` — the builds repository template.
