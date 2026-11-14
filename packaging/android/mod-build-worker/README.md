# Horizon Android's mod build gateway, on Cloudflare

Native mods are compiled into the game's own library, and a phone can't do that. This Worker is the
door between the launcher and GitHub's runners, with no machine of yours to keep: it dispatches the
builds repository's `build.yml`, keeps the player's mods in KV for the runner to fetch, and points the
phone's download at the release the run published. Free plan, no card.

It speaks the same protocol as `mod_build_gateway.py` (`docs/android-native-mods.md`), so the launcher
can't tell them apart: **Settings → Native mods → Build server** = the Worker's URL.

## Once

1. Make the builds repository from `packaging/android/mod-builds/` (public), and run its **base**
   workflow once for the game's commit.
2. Make a fine-grained token for that repository: **Actions: write**. (Reading contents is not
   needed; the workflow reads its own repository with its own token.)
3. Deploy the Worker, from this folder:

   ```sh
   npx wrangler kv namespace create STORE          # put the id in wrangler.toml
   npx wrangler secret put GITHUB_TOKEN            # paste the token
   # wrangler.toml: GITHUB_REPO = "you/horizon-mod-builds"
   npx wrangler deploy
   ```

   `wrangler` also takes `CLOUDFLARE_API_TOKEN` in the environment instead of logging in, which is how
   it runs from a phone. If `wrangler` can't run at all (on Termux it fails with "Unsupported
   platform: android arm64 LE", because it carries the `workerd` runtime, which has no Android
   build), use the plain-curl deployer instead:

   ```sh
   CLOUDFLARE_API_TOKEN=… GITHUB_TOKEN=… bash deploy.sh
   ```

   It finds or makes the `STORE` namespace, puts its id in `wrangler.toml`, uploads `src/index.js`,
   sets the `GITHUB_TOKEN` secret and enables `workers.dev`, then prints the URL. It needs a token
   with **Workers Scripts: Edit** and **Workers KV Storage: Edit**; set `CLOUDFLARE_ACCOUNT_ID` if
   the token sees more than one account.
4. In Horizon, set the Build server to the `https://….workers.dev` URL `wrangler` (or `deploy.sh`) prints.

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
