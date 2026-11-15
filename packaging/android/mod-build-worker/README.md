# Horizon Android's mod build gateway, on Cloudflare

Native mods are compiled into the game's own library, and a phone can't do that. This Worker is the
door between the launcher and GitHub's runners, with no machine of yours to keep: it dispatches the
builds repository's `build.yml`, keeps the player's mods in KV for the runner to fetch, and points the
phone's download at the release the run published. Free plan, no card.

It speaks the same protocol as `mod_build_gateway.py` (`docs/android-native-mods.md`), so the launcher
can't tell them apart: **Settings → Native mods → Build server** = the Worker's URL.

## What you need

- A GitHub account and a Cloudflare account, both free, and about 15 minutes.
- This repository (or a fork of it with the same `packaging/android/` layout) at the commit the app
  was built from: `ProSteve123GH/horizon-android-old`.

The Worker needs a builds repository before it can be useful, so start there.

## 1. Make the builds repository (GitHub's runners)

The compile happens in a separate **public** repository made from `packaging/android/mod-builds/`.
Public matters: the phone downloads the finished library from that repository's release assets
without a token, and the Worker never reads the game repository.

1. On GitHub, create a new **public** repository, for example `you/horizon-mod-builds`.
2. Copy everything in `packaging/android/mod-builds/` into it, including the hidden `.github/`
   folder, and commit it to `main`.
3. In `.github/workflows/base.yml` and `build.yml`, set `HORIZON_REPO` to the game repository's
   URL and `HORIZON_REPO_PATH` to its `owner/name`. The template already names this
   fork (`ProSteve123GH/horizon-android-old`); change both only if you build from another fork.
4. Add the release you run to `versions.json`, for example `{"0.7.7": "<full commit>"}`. The
   launcher usually sends the commit it was built from, so only a launcher that cannot name one
   falls back to this file.
5. Make the clean build cache once: **Actions → base → Run workflow**, `version` = the game version
   (`0.7.7`), `commit` = the release's full commit, or empty to use `versions.json`. It takes about
   15 minutes. Every player's build starts from this cache; without it the build job stops on the
   cache restore.
6. Make a fine-grained GitHub token for **that repository only**, with **Actions: write**. Nothing
   else is needed: the workflow reads the builds repository with its own token.

Keep the token for step 2.

## 2. Deploy this Worker

Run these from this folder (`packaging/android/mod-build-worker/`).

1. Install `wrangler` and sign in:

   ```sh
   wrangler login
   ```

   On Termux, the `wrangler` in `PATH` handles `login` and `deploy`; its local `dev` server needs
   `workerd`, which has no Android build. `wrangler` also takes `CLOUDFLARE_API_TOKEN` in the
   environment instead of a login.

2. Make the KV store:

   ```sh
   wrangler kv namespace create STORE
   ```

   Copy the printed `id` into `wrangler.toml` under `[[kv_namespaces]]` (binding `STORE`).

3. In `[vars]`, set `GITHUB_REPO = "<you>/horizon-mod-builds"`. `GITHUB_WORKFLOW` and `GITHUB_REF`
   already name `build.yml` on `main`.

4. Store the token from step 1:

   ```sh
   wrangler secret put GITHUB_TOKEN
   ```

   Paste the fine-grained token at the prompt. It stays a secret: it never goes into
   `wrangler.toml`.

5. Deploy:

   ```sh
   wrangler deploy
   ```

   The last line prints the URL, `https://<worker>.<account>.workers.dev`. That is the build server.

6. Check that it answers the protocol without starting a build:

   ```sh
   curl -sS -X POST 'https://<worker>.<account>.workers.dev/v1/build?version=bad'
   ```

   It should answer HTTP 400 with `{"error":"That is not a version of the game."}`.

If `wrangler` cannot run at all, `deploy.sh` does the same over Cloudflare's API with curl and
python3:

```sh
CLOUDFLARE_API_TOKEN=… GITHUB_TOKEN=… bash deploy.sh
```

It finds or makes the `STORE` namespace, puts its id in `wrangler.toml`, uploads `src/index.js`,
sets the `GITHUB_TOKEN` secret and enables `workers.dev`, then prints the URL. It needs a token with
**Workers Scripts: Edit** and **Workers KV Storage: Edit**; set `CLOUDFLARE_ACCOUNT_ID` if the token
sees more than one account.

## 3. Point Horizon at it

1. In the launcher, set **Settings → Native mods → Build server** to the URL from step 2.
2. Leave **Compile on GitHub** on. The Worker always uses GitHub's runners; the switch only steers a
   gateway that can build by itself.
3. Open **Mods**, find a native (`.hnmod`) mod, and press **Build**. The launcher uploads the mods
   that are on, waits for the run (up to 90 minutes), and downloads the library.
4. The result appears in **Settings → Native mods → Builds**; press **Use** to play it. Select
   several native mods before pressing **Build** to compile them all in one run.

This fork's launcher already names its own Worker
(`https://horizon-mod-builds.steve-horizon-android-hnmod.workers.dev`), so steps 1–2 are only needed
when you deploy your own.

## Troubleshooting

- **`400 {"error":"That is not a version of the game."}`** — the launcher sent a version name that
  is neither a release (`0.7.7`) nor a dev build (`0.7.7-dev.<run>.<commit>`). Check the app's
  version name; the build server rejects what it cannot name.
- **`429 {"error":"GitHub refused the build."}`** — the `GITHUB_TOKEN` lacks **Actions: write** on
  `GITHUB_REPO`, or `GITHUB_REPO` names another repository.
- **`429 {"error":"GitHub did not start the build."}`** — Actions are disabled on the builds
  repository, or `GITHUB_REF` is not the branch that holds `build.yml`.
- **The build job stops on the cache restore** — run **base** once for that commit (step 1.5), then
  ask for the build again.
- **`403` from Cloudflare before the Worker runs** — the URL is not the one `wrangler deploy`
  printed (or that deployment was removed). Opening the URL in a browser should answer
  `{"error":"No such thing here."}` with 404; anything else is the wrong host.
- **`Set the native mod build server first`** — the Build server row is empty; paste the URL again.

## What it does

- `POST /v1/build?version=…&commit=…` — the launcher's mods. The job's name is a digest of the
  request, so the same mods on the same game are built once; a finished build stays known for three
  days.
- `GET /v1/job/<job>` — where the build is: `queued`, `building`, `done`, `failed`.
- `GET /v1/job/<job>/lib` — the library, from the release asset, `Range` resumes kept.
- `GET /v1/github/<ask>` — the mods, for the runner. `POST /v1/github/<ask>/state` — its answer.

## Limits

- **KV**, free plan: 1000 writes, 100k reads, 1 GB a day. A build writes its request in 8 MiB pieces
  plus a few small records, so `PER_DAY` (40) is what keeps the writes in budget; lower it if you
  raise the request size.
- A KV value holds 25 MiB, so a request is kept in pieces; `MAX_BODY` is the service's 32 MiB.
- Workers free plan: 100k requests a day. The launcher asks about its job every two seconds, so a
  twenty-minute build is about 600 of them; `PER_DAY` keeps the day's reads in budget too.
- The library is not stored here: the phone downloads it from the release, through this Worker, so a
  100 MB library is not a KV write.
- `cleanup.yml` in the builds repository deletes build releases older than four days.

## Tests

`node --test test.mjs` — the protocol as the launcher and the workflow see it, with a fake KV and a
fake GitHub. The Android source checks run it.
