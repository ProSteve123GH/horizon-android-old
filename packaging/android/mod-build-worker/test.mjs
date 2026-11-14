// The Worker's protocol, as the launcher and the workflow see it: node --test (CI: Android source checks).
// `env.STORE` and `fetch` are faked; everything else is the real `src/index.js`.
import assert from "node:assert/strict";
import test from "node:test";

import worker from "./src/index.js";

const BASE = "https://builds.example";
const REPO = "owner/repo";

/// A KV that keeps what it is given, in bytes, as the real one does.
function memory() {
  const map = new Map();
  const bytes = (value) => {
    if (typeof value === "string") return new TextEncoder().encode(value);
    if (value instanceof Uint8Array) return value;
    return new Uint8Array(value);
  };
  return {
    map,
    async get(key, type) {
      const value = map.get(key);
      if (value === undefined) return null;
      if (type === "json") return JSON.parse(new TextDecoder().decode(value));
      if (type === "arrayBuffer") return value.slice().buffer;
      return new TextDecoder().decode(value);
    },
    async put(key, value) {
      map.set(key, bytes(value));
    },
    async delete(key) {
      map.delete(key);
    },
  };
}

/// A request as the Worker reads one: Node's Request refuses a Content-Length, which the launcher sends.
function call(url, { method = "GET", body, headers = {} } = {}) {
  const text = body === undefined ? "" : new TextDecoder().decode(body);
  return {
    method,
    url,
    headers: new Headers(headers),
    async arrayBuffer() {
      return body === undefined ? new ArrayBuffer(0) : body.slice().buffer;
    },
    async json() {
      return JSON.parse(text);
    },
  };
}

/// GitHub's API and the release asset behind a redirect, as the Worker meets them.
function github(asset) {
  const dispatches = [];
  const fetch = async (url, options = {}) => {
    if (url.startsWith("https://api.github.com/")) {
      dispatches.push(JSON.parse(options.body));
      return new Response(null, { status: 204 });
    }
    if (url.startsWith("https://github.com/")) {
      return new Response(null, {
        status: 302,
        headers: { Location: "https://objects.example/lib" },
      });
    }
    const range = options.headers?.Range;
    const from = range ? Number(range.replace("bytes=", "").replace("-", "")) : 0;
    return new Response(asset.slice(from), {
      status: range ? 206 : 200,
      headers: {
        "Content-Type": "application/octet-stream",
        "Content-Length": String(asset.length - from),
        ...(range
          ? { "Content-Range": `bytes ${from}-${asset.length - 1}/${asset.length}` }
          : {}),
      },
    });
  };
  return { dispatches, fetch };
}

const mods = new Uint8Array([0, 0, 0, 9, 80, 75, 3, 4, 1, 2, 3, 4, 5]);
const library = new Uint8Array([10, 11, 12, 13, 14, 15, 16, 17, 18, 19]);

function setup() {
  const store = memory();
  const gh = github(library);
  globalThis.fetch = gh.fetch;
  const env = {
    STORE: store,
    GITHUB_REPO: REPO,
    GITHUB_TOKEN: "a-token",
    GITHUB_WORKFLOW: "build.yml",
    GITHUB_REF: "main",
    PER_DAY: "40",
    PER_ADDRESS: "16",
  };
  const send = (url, options) =>
    worker.fetch(call(url, options), env, {});
  return { store, gh, env, send };
}

const buildUrl = (version = "0.7.7", commit = "") =>
  `${BASE}/v1/build?version=${version}${commit ? `&commit=${commit}` : ""}&via=github`;

test("a build is dispatched, its mods fetched, and its answer becomes the job", async () => {
  const { gh, send } = setup();
  const answer = await send(buildUrl(), {
    method: "POST",
    body: mods,
    headers: { "Content-Length": String(mods.length), "CF-Connecting-IP": "1.2.3.4" },
  });
  assert.equal(answer.status, 200);
  const { job } = await answer.json();
  assert.match(job, /^[0-9a-f]{24}$/);

  assert.equal(gh.dispatches.length, 1);
  const [asked] = gh.dispatches;
  assert.match(asked.inputs.ask, /^[0-9a-f]{32}$/);
  assert.equal(asked.inputs.version, "0.7.7");
  assert.equal(asked.inputs.commit, "");
  assert.equal(asked.inputs.gateway, BASE, "the runner must be told where to fetch the mods");
  assert.equal(asked.ref, "main");

  const fetched = await send(`${BASE}/v1/github/${asked.inputs.ask}`);
  assert.equal(fetched.status, 200);
  assert.deepEqual(new Uint8Array(await fetched.arrayBuffer()), mods);
  assert.equal(fetched.headers.get("Content-Length"), String(mods.length));

  const running = await (await send(`${BASE}/v1/job/${job}`)).json();
  assert.equal(running.state, "building");
  assert.equal(typeof running.seconds, "number");

  const answered = await send(`${BASE}/v1/github/${asked.inputs.ask}/state`, {
    method: "POST",
    body: new TextEncoder().encode(
      JSON.stringify({
        state: "done",
        sha256: "a".repeat(64),
        size: library.length,
        build: { mods: [{ id: "example", version: "1.0.0" }] },
      }),
    ),
    headers: { "Content-Type": "application/json" },
  });
  assert.equal(answered.status, 200);

  const done = await (await send(`${BASE}/v1/job/${job}`)).json();
  assert.equal(done.state, "done");
  assert.equal(done.sha256, "a".repeat(64));
  assert.equal(done.size, library.length);
  assert.equal(done.build.version, "0.7.7", "the launcher keeps this beside the library");
  assert.equal(done.build.key, job);
  assert.equal(done.build.mods[0].id, "example");

  assert.equal(
    (await send(`${BASE}/v1/github/${asked.inputs.ask}`)).status,
    404,
    "the mods are gone once the build has them",
  );
});

test("the library is served behind GitHub's redirect, Range kept", async () => {
  const { gh, send } = setup();
  const { job } = await (
    await send(buildUrl(), {
      method: "POST",
      body: mods,
      headers: { "Content-Length": String(mods.length) },
    })
  ).json();
  const ask = gh.dispatches[0].inputs.ask;
  await send(`${BASE}/v1/github/${ask}/state`, {
    method: "POST",
    body: new TextEncoder().encode(
      JSON.stringify({ state: "done", sha256: "b".repeat(64), size: library.length, build: {} }),
    ),
  });

  const whole = await send(`${BASE}/v1/job/${job}/lib`);
  assert.equal(whole.status, 200);
  assert.deepEqual(new Uint8Array(await whole.arrayBuffer()), library);

  const resumed = await send(`${BASE}/v1/job/${job}/lib`, {
    headers: { Range: "bytes=4-" },
  });
  assert.equal(resumed.status, 206);
  assert.equal(resumed.headers.get("Content-Range"), `bytes 4-${library.length - 1}/${library.length}`);
  assert.deepEqual(new Uint8Array(await resumed.arrayBuffer()), library.slice(4));
});

test("the same mods on the same game are compiled once", async () => {
  const { gh, send } = setup();
  const first = await (
    await send(buildUrl(), {
      method: "POST",
      body: mods,
      headers: { "Content-Length": String(mods.length) },
    })
  ).json();
  const ask = gh.dispatches[0].inputs.ask;
  await send(`${BASE}/v1/github/${ask}/state`, {
    method: "POST",
    body: new TextEncoder().encode(
      JSON.stringify({ state: "done", sha256: "c".repeat(64), size: library.length, build: {} }),
    ),
  });

  const again = await (
    await send(buildUrl(), {
      method: "POST",
      body: mods,
      headers: { "Content-Length": String(mods.length) },
    })
  ).json();
  assert.equal(again.job, first.job);
  assert.equal(gh.dispatches.length, 1);

  const other = await (
    await send(buildUrl("0.7.8"), {
      method: "POST",
      body: mods,
      headers: { "Content-Length": String(mods.length) },
    })
  ).json();
  assert.notEqual(other.job, first.job, "another game is another build");
  assert.equal(gh.dispatches.length, 2);
});

test("a request over the exact digest is kept whole and fetched whole", async () => {
  const { gh, send } = setup();
  const big = new Uint8Array(5 << 20);
  for (let i = 0; i < big.length; i += 1) big[i] = i & 0xff;
  const answer = await send(buildUrl(), {
    method: "POST",
    body: big,
    headers: { "Content-Length": String(big.length) },
  });
  assert.equal(answer.status, 200);
  const { job } = await answer.json();

  const fetched = await send(`${BASE}/v1/github/${gh.dispatches[0].inputs.ask}`);
  assert.equal(fetched.status, 200);
  assert.deepEqual(new Uint8Array(await fetched.arrayBuffer()), big);
  assert.equal((await (await send(`${BASE}/v1/job/${job}`)).json()).state, "building");
});

test("a failure the mods caused stands, one that wasn't is built again", async () => {
  const { gh, send } = setup();
  const other = new Uint8Array([0, 0, 0, 1, 7]);
  const post = (body) =>
    send(buildUrl(), {
      method: "POST",
      body,
      headers: { "Content-Length": String(body.length) },
    });
  const answer = async (ask, value) =>
    send(`${BASE}/v1/github/${ask}/state`, {
      method: "POST",
      body: new TextEncoder().encode(JSON.stringify(value)),
    });

  const first = await (await post(other)).json();
  await answer(gh.dispatches[0].inputs.ask, {
    state: "failed",
    error: "The mods do not compile.",
    settled: true,
  });
  const failed = await (await send(`${BASE}/v1/job/${first.job}`)).json();
  assert.equal(failed.state, "failed");
  assert.equal(failed.error, "The mods do not compile.");
  assert.equal((await (await post(other)).json()).job, first.job);
  assert.equal(gh.dispatches.length, 1, "the same broken mods are not compiled again");

  const transient = new Uint8Array([0, 0, 0, 2, 8]);
  const second = await (await post(transient)).json();
  await answer(gh.dispatches[1].inputs.ask, {
    state: "failed",
    error: "GitHub's machine went away.",
  });
  assert.equal((await (await send(`${BASE}/v1/job/${second.job}`)).json()).state, "failed");
  assert.equal((await (await post(transient)).json()).job, second.job);
  assert.equal(gh.dispatches.length, 3, "a build that wasn't the mods' fault is asked for again");
});

test("what is not a request is refused, and says why", async () => {
  const { gh, send } = setup();
  const tooBig = await send(buildUrl(), {
    method: "POST",
    body: mods,
    headers: { "Content-Length": String(33 << 20) },
  });
  assert.equal(tooBig.status, 413);
  assert.match((await tooBig.json()).error, /too big/);

  const notAVersion = await send(buildUrl("nightly"), {
    method: "POST",
    body: mods,
    headers: { "Content-Length": String(mods.length) },
  });
  assert.equal(notAVersion.status, 400);

  assert.equal(gh.dispatches.length, 0);
  assert.equal((await send(`${BASE}/v1/job/${"0".repeat(24)}`)).status, 404);
  assert.equal((await send(`${BASE}/v1/job/nope`)).status, 404);
  assert.equal((await send(`${BASE}/v1/github/${"0".repeat(32)}`)).status, 404);
});

test("a job no one finishes is given up on", async () => {
  const { env, send } = setup();
  const { job } = await (
    await send(buildUrl(), {
      method: "POST",
      body: mods,
      headers: { "Content-Length": String(mods.length) },
    })
  ).json();
  const key = `job:${job}`;
  const known = await env.STORE.get(key, "json");
  await env.STORE.put(
    key,
    JSON.stringify({ ...known, state: "building", started: 0 }),
  );
  const late = await (await send(`${BASE}/v1/job/${job}`)).json();
  assert.equal(late.state, "failed");
  assert.match(late.error, /took too long/);
});
