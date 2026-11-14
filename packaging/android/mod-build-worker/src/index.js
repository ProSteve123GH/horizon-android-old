// Horizon Android's native mod builds on GitHub's machines, with no machine of ours: a Cloudflare
// Worker keeping the protocol of `mod_build_gateway.py` (docs/android-native-mods.md).
//
//   POST /v1/build?version=…&commit=…   [u32 big-endian length][zip]…  → {"job": "<24 hex>"}
//   GET  /v1/job/<job>                  → {"state": "queued|building|done|failed", …}
//   GET  /v1/job/<job>/lib              → the library, Range resumes
//   GET  /v1/github/<ask>               → a request's mods, for the workflow to fetch
//   POST /v1/github/<ask>/state         → the workflow's answer
//
// The launcher knows nothing of GitHub: it asks for a build as it would of any server, and the
// Worker runs the builds repository's `build.yml`, keeps the request in KV for the runner to fetch,
// and points the download at the release the run published. The token that may run that repository's
// workflows is the Worker's secret; it never reaches the phone.
//
// The same mods on the same game are the same job (`job` is a digest of the request), so a build
// asked for twice is downloaded twice and compiled once.
//
// Bindings: `STORE` (KV). Secrets: `GITHUB_TOKEN`. Vars: `GITHUB_REPO` (owner/name, required),
// `GITHUB_WORKFLOW` (default build.yml), `GITHUB_REF` (default main), `PER_DAY`, `PER_ADDRESS`.

const DAY = 24 * 60 * 60;
/// The most one request may be (`mod_build_server.py`'s `MAX_BODY`).
const MAX_BODY = 32 << 20;
/// A KV value holds 25 MiB; a request is kept in pieces this big.
const CHUNK = 8 << 20;
const LIBRARY = "libhorizon_android.so";
const JOB = /^[0-9a-f]{24}$/;
const ASK = /^[0-9a-f]{32}$/;
const VERSION = /^\d+(\.\d+){1,3}(-dev\.[A-Za-z0-9]+)?$/;
const COMMIT = /^[0-9a-f]{7,40}$/;
/// A run that has not fetched its mods in this long, or one still building after this, isn't coming.
const QUEUED_SECONDS = 25 * 60;
const BUILDING_SECONDS = 75 * 60;
/// How long a finished job stays known: asking for the same mods again is then no build at all.
const DONE_SECONDS = 3 * DAY;

const json = (status, value) =>
  new Response(JSON.stringify(value), {
    status,
    headers: { "Content-Type": "application/json" },
  });

const hex = (buffer) =>
  [...new Uint8Array(buffer)].map((b) => b.toString(16).padStart(2, "0")).join("");

const digest = async (bytes) => hex(await crypto.subtle.digest("SHA-256", bytes));

const seconds = () => Math.floor(Date.now() / 1000);

const perDay = (env) => Number(env.PER_DAY ?? 40);
const perAddress = (env) => Number(env.PER_ADDRESS ?? 16);

/// A free Worker has 10 ms of CPU for the whole request: a body over `EXACT` is summed by its length
/// and its two ends rather than whole. Every mod's zip ends with its central directory, and the
/// launcher's request is one mod after another.
const EXACT = 4 << 20;
const SAMPLE = 1 << 20;

/// The name of a request's job: a digest of the game and the mods, so the same mods on the same game
/// are one job.
async function jobName(head, body) {
  const size = new Uint8Array(4);
  new DataView(size.buffer).setUint32(0, body.length);
  const sample =
    body.length <= EXACT
      ? body
      : concat(
          body.subarray(0, SAMPLE),
          body.subarray(body.length - SAMPLE, body.length),
        );
  const both = new Uint8Array(head.length + size.length + sample.length);
  both.set(head);
  both.set(size, head.length);
  both.set(sample, head.length + size.length);
  return (await digest(both)).slice(0, 24);
}

function concat(first, second) {
  const both = new Uint8Array(first.length + second.length);
  both.set(first);
  both.set(second, first.length);
  return both;
}

/// A request's body, or nothing when it is empty or bigger than the service takes.
async function bodyOf(request) {
  const stated = Number(request.headers.get("Content-Length") ?? "0");
  if (!(stated > 0) || stated > MAX_BODY) {
    return null;
  }
  const body = new Uint8Array(await request.arrayBuffer());
  return body.length === stated ? body : null;
}

/// The mods of `ask` in KV, gone: the runner has them, or the build they were for is over.
async function forgetAsk(env, ask) {
  const meta = await env.STORE.get(`ask:${ask}`, "json");
  if (!meta) {
    return;
  }
  for (let i = 0; i < meta.chunks; i += 1) {
    await env.STORE.delete(`body:${ask}:${i}`);
  }
  await env.STORE.delete(`ask:${ask}`);
}

async function forgetJob(env, job, ask) {
  await forgetAsk(env, ask);
  await env.STORE.delete(`job:${job}`);
}

/// Asks the builds repository to compile these mods. The runner fetches them from us by `ask`.
async function dispatch(env, base, ask, version, commit) {
  try {
    const response = await fetch(
      `https://api.github.com/repos/${env.GITHUB_REPO}/actions/workflows/${env.GITHUB_WORKFLOW || "build.yml"}/dispatches`,
      {
        method: "POST",
        headers: {
          Authorization: `Bearer ${env.GITHUB_TOKEN}`,
          Accept: "application/vnd.github+json",
          "Content-Type": "application/json",
          "X-GitHub-Api-Version": "2022-11-28",
          "User-Agent": "horizon-mod-builds",
        },
        body: JSON.stringify({
          ref: env.GITHUB_REF || "main",
          inputs: { ask, version, commit, gateway: base },
        }),
      },
    );
    return response.ok;
  } catch {
    return false;
  }
}

async function asked(request, env) {
  const url = new URL(request.url);
  const version = url.searchParams.get("version") ?? "";
  const commit = url.searchParams.get("commit") ?? "";
  if (!VERSION.test(version)) {
    return json(400, { error: "That is not a version of the game." });
  }
  if (commit && !COMMIT.test(commit)) {
    return json(400, { error: "That is not a commit." });
  }
  if (!env.GITHUB_REPO || !env.GITHUB_TOKEN) {
    return json(503, { error: "Building on GitHub isn't available right now." });
  }
  const body = await bodyOf(request);
  if (!body) {
    return json(413, { error: "That is too big to compile." });
  }
  // The same mods on the same game, whichever machine is asked: what one built, the other has.
  const head = new TextEncoder().encode(
    `/v1/build?version=${version}&commit=${commit}`,
  );
  const job = await jobName(head, body);
  const known = await env.STORE.get(`job:${job}`, "json");
  // A failure the mods caused stands for a day, as on a server that keeps them; one that wasn't is
  // built again.
  if (known && !(known.state === "failed" && !known.settled)) {
    return json(200, { job });
  }
  const now = seconds();
  const day = new Date(now * 1000).toISOString().slice(0, 10);
  const address = request.headers.get("CF-Connecting-IP") ?? "unknown";
  const mine = Number(await env.STORE.get(`day:${day}:${address}`)) || 0;
  if (mine >= perAddress(env)) {
    return json(429, {
      error: "You have reached today's build limit. Try again tomorrow.",
    });
  }
  const all = Number(await env.STORE.get(`day:${day}:all`)) || 0;
  if (all >= perDay(env)) {
    return json(429, {
      error:
        "The build server has reached today's limit for all players. Try again tomorrow.",
    });
  }
  const ask = hex(crypto.getRandomValues(new Uint8Array(16)));
  const chunks = Math.max(1, Math.ceil(body.length / CHUNK));
  for (let i = 0; i < chunks; i += 1) {
    await env.STORE.put(
      `body:${ask}:${i}`,
      body.slice(i * CHUNK, (i + 1) * CHUNK),
      { expirationTtl: 2 * DAY },
    );
  }
  await env.STORE.put(
    `ask:${ask}`,
    JSON.stringify({ job, version, commit, chunks, size: body.length }),
    { expirationTtl: 2 * DAY },
  );
  await env.STORE.put(
    `job:${job}`,
    JSON.stringify({ state: "queued", ask, version, commit, at: now }),
    { expirationTtl: 2 * DAY },
  );
  await env.STORE.put(`day:${day}:${address}`, String(mine + 1), {
    expirationTtl: 2 * DAY,
  });
  await env.STORE.put(`day:${day}:all`, String(all + 1), {
    expirationTtl: 2 * DAY,
  });
  // The mods of a build that was told to try again are not needed any more.
  if (known && known.ask && known.ask !== ask) {
    await forgetAsk(env, known.ask);
  }
  if (!(await dispatch(env, url.origin, ask, version, commit))) {
    await forgetJob(env, job, ask);
    return json(429, { error: "GitHub refused the build. Try again later." });
  }
  return json(200, { job });
}

/// The mods, for the runner: whoever has the name, which is random.
async function mods(env, ask) {
  if (!ASK.test(ask)) {
    return json(404, { error: "No such thing here." });
  }
  const meta = await env.STORE.get(`ask:${ask}`, "json");
  if (!meta) {
    return json(404, { error: "No such thing here." });
  }
  const job = await env.STORE.get(`job:${meta.job}`, "json");
  if (job && job.state === "queued") {
    await env.STORE.put(
      `job:${meta.job}`,
      JSON.stringify({ ...job, state: "building", started: seconds() }),
      { expirationTtl: 2 * DAY },
    );
  }
  const parts = [];
  for (let i = 0; i < meta.chunks; i += 1) {
    const part = await env.STORE.get(`body:${ask}:${i}`, "arrayBuffer");
    if (!part) {
      return json(404, { error: "No such thing here." });
    }
    parts.push(part);
  }
  return new Response(new Blob(parts), {
    headers: {
      "Content-Type": "application/octet-stream",
      "Content-Length": String(meta.size),
    },
  });
}

/// The run's answer: what the service would say of the job, and where its library is.
async function answered(request, env, ask) {
  if (!ASK.test(ask)) {
    return json(404, { error: "No such thing here." });
  }
  const meta = await env.STORE.get(`ask:${ask}`, "json");
  if (!meta) {
    return json(404, { error: "No such thing here." });
  }
  let value;
  try {
    value = await request.json();
  } catch {
    return json(400, { error: "That is not the build's answer." });
  }
  const job = (await env.STORE.get(`job:${meta.job}`, "json")) ?? {
    version: meta.version,
    commit: meta.commit,
    at: seconds(),
  };
  if (value.state === "done") {
    const sha = String(value.sha256 ?? "");
    const size = Number(value.size ?? 0);
    const build = value.build;
    if (
      !/^[0-9a-f]{64}$/.test(sha) ||
      !(size > 0) ||
      typeof build !== "object" ||
      build === null
    ) {
      return json(400, { error: "That is not the build's answer." });
    }
    await env.STORE.put(
      `job:${meta.job}`,
      JSON.stringify({
        ...job,
        state: "done",
        sha256: sha,
        size,
        // What the launcher writes beside the library: which build, and its mods.
        build: { ...build, version: meta.version, key: meta.job },
        url: `https://github.com/${env.GITHUB_REPO}/releases/download/build-${ask}/${LIBRARY}`,
      }),
      { expirationTtl: DONE_SECONDS },
    );
  } else {
    await env.STORE.put(
      `job:${meta.job}`,
      JSON.stringify({
        ...job,
        state: "failed",
        error: String(value.error ?? "The build failed.").slice(0, 600),
        settled: Boolean(value.settled),
      }),
      { expirationTtl: DAY },
    );
  }
  await forgetAsk(env, ask);
  return json(200, {});
}

async function looked(env, job) {
  const known = await env.STORE.get(`job:${job}`, "json");
  if (!known) {
    return json(404, { error: "No such job. Build it again." });
  }
  const now = seconds();
  if (known.state === "queued" || known.state === "building") {
    const waited = now - (known.started ?? known.at ?? now);
    const late =
      known.state === "queued"
        ? waited > QUEUED_SECONDS
        : waited > BUILDING_SECONDS;
    if (late) {
      const error =
        known.state === "queued"
          ? "GitHub did not start the build. Try again later."
          : "The build took too long. Try again later.";
      await env.STORE.put(
        `job:${job}`,
        JSON.stringify({ ...known, state: "failed", error, settled: false }),
        { expirationTtl: DAY },
      );
      return json(200, { state: "failed", error, settled: false });
    }
  }
  if (known.state === "building") {
    return json(200, {
      state: "building",
      seconds: now - (known.started ?? now),
    });
  }
  if (known.state === "queued") {
    return json(200, { state: "queued", ahead: 0 });
  }
  if (known.state === "done") {
    return json(200, {
      state: "done",
      sha256: known.sha256,
      size: known.size,
      build: known.build,
    });
  }
  return json(200, {
    state: "failed",
    error: known.error ?? "The build failed.",
    settled: Boolean(known.settled),
  });
}

/// A release asset behind its redirect, Range kept: the phone resumes where it stopped.
async function follow(url, range) {
  let target = url;
  for (let hop = 0; hop < 4; hop += 1) {
    const headers = range ? { Range: range } : {};
    const response = await fetch(target, { headers, redirect: "manual" });
    if (response.status < 300 || response.status >= 400) {
      return response;
    }
    const location = response.headers.get("Location");
    if (!location) {
      return response;
    }
    target = new URL(location, target).toString();
  }
  return new Response(null, { status: 502 });
}

async function library(request, env, job) {
  const known = await env.STORE.get(`job:${job}`, "json");
  if (!known || known.state !== "done" || !known.url) {
    return json(404, { error: "No such thing here." });
  }
  let upstream;
  try {
    upstream = await follow(known.url, request.headers.get("Range"));
  } catch {
    return json(503, { error: "The build's library can't be reached right now." });
  }
  const headers = new Headers();
  for (const name of [
    "Content-Type",
    "Content-Length",
    "Content-Range",
    "Accept-Ranges",
  ]) {
    const value = upstream.headers.get(name);
    if (value) {
      headers.set(name, value);
    }
  }
  if (!headers.has("Content-Type")) {
    headers.set("Content-Type", "application/octet-stream");
  }
  return new Response(upstream.body, { status: upstream.status, headers });
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const parts = url.pathname.split("/").filter(Boolean);
    if (parts[0] !== "v1") {
      return json(404, { error: "No such thing here." });
    }
    if (request.method === "POST" && url.pathname === "/v1/build") {
      return asked(request, env);
    }
    if (request.method === "GET" && parts[1] === "github" && parts.length === 3) {
      return mods(env, parts[2]);
    }
    if (
      request.method === "POST" &&
      parts[1] === "github" &&
      parts.length === 4 &&
      parts[3] === "state"
    ) {
      return answered(request, env, parts[2]);
    }
    if (request.method === "GET" && parts[1] === "job" && parts.length >= 3) {
      if (!JOB.test(parts[2])) {
        return json(404, { error: "No such thing here." });
      }
      if (parts.length === 3) {
        return looked(env, parts[2]);
      }
      if (parts.length === 4 && parts[3] === "lib") {
        return library(request, env, parts[2]);
      }
    }
    return json(404, { error: "No such thing here." });
  },
};
