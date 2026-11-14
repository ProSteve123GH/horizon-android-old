#!/usr/bin/env bash
# Deploy this Worker with curl, for a machine where `wrangler` can't run: wrangler carries Cloudflare's
# `workerd` binary, and there is no build of it for Android arm64 (npx wrangler login fails there with
# "Unsupported platform: android arm64 LE"). This does what the three wrangler commands in README.md do,
# over Cloudflare's API, and needs nothing but curl and python3.
#
#   CLOUDFLARE_API_TOKEN=… GITHUB_TOKEN=… bash deploy.sh
#
# CLOUDFLARE_API_TOKEN: My Profile → API Tokens → Create Token → Edit Cloudflare Workers template, or a
# custom token with **Workers Scripts: Edit** and **Workers KV Storage: Edit**. CLOUDFLARE_ACCOUNT_ID
# is found from the token; set it if the token sees more than one account. GITHUB_TOKEN is the
# fine-grained token for the builds repository (**Actions: write**) that becomes the Worker's secret.
#
# It does what a deploy must, once:
#   kv namespace create STORE — the namespace is found by name or made, and its id goes into
#                               wrangler.toml, so `wrangler deploy` later sends the same one;
#   secret put GITHUB_TOKEN   — the token is set as a secret, never as a plain var;
#   deploy                    — src/index.js and wrangler.toml's compatibility date and vars;
# and it ends with the https://….workers.dev address to put in Settings → Native mods.
#
# Env: CF_API (default https://api.cloudflare.com/client/v4), CLOUDFLARE_SUBDOMAIN (the account's
# workers.dev name if it has none yet), CLOUDFLARE_ACCOUNT_ID, GITHUB_TOKEN.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
api=${CF_API:-https://api.cloudflare.com/client/v4}
die() { echo "$*" >&2; exit 1; }

command -v curl >/dev/null || die "curl is needed"
command -v python3 >/dev/null || die "python3 is needed"
: "${CLOUDFLARE_API_TOKEN:?set CLOUDFLARE_API_TOKEN (Workers Scripts: Edit, Workers KV Storage: Edit)}"
: "${GITHUB_TOKEN:?set GITHUB_TOKEN (fine-grained, this builds repository: Actions: write)}"
[ -f "$here/src/index.js" ] || die "src/index.js is missing; run this from the mod-build-worker folder"
[ -f "$here/wrangler.toml" ] || die "wrangler.toml is missing; run this from the mod-build-worker folder"

# Cloudflare answers {"success": false, "errors": [...]} for a refusal; the message is what helps.
field() { # field EXPR — a dot path into the answer, or the refusal on stderr and exit 1
  python3 -c '
import json, sys
answer = json.load(sys.stdin)
if not answer.get("success"):
    said = "; ".join(str(e.get("message", e)) for e in answer.get("errors", [])) or "no reason given"
    sys.exit("Cloudflare refused: " + said)
value = answer
for part in sys.argv[1].split("."):
    value = value[int(part)] if part.lstrip("-").isdigit() else value[part]
print("" if value is None else value)
' "$1"
}
cf() { curl -fsS --retry 3 --retry-all-errors -H "Authorization: Bearer $CLOUDFLARE_API_TOKEN" "$@"; }

name=$(python3 -c '
import re, sys
text = open("wrangler.toml").read()
found = re.search(r"^\s*name\s*=\s*\"([^\"]+)\"", text, re.M)
sys.exit("wrangler.toml has no name") if not found else print(found.group(1))
')
echo "Worker: $name"

account=${CLOUDFLARE_ACCOUNT_ID:-}
if [ -z "$account" ]; then
  accounts=$(cf "$api/accounts?per_page=50")
  count=$(printf '%s' "$accounts" | field result_info.total_count)
  account=$(printf '%s' "$accounts" | field result.0.id)
  [ -n "$account" ] || die "This token sees no account; check its permissions."
  if [ "$count" != 1 ]; then
    echo "This token sees $count accounts:" >&2
    printf '%s' "$accounts" | python3 -c '
import json, sys
for a in json.load(sys.stdin)["result"]:
    print("  " + a["id"] + "  " + a["name"], file=sys.stderr)
'
    die "Set CLOUDFLARE_ACCOUNT_ID to the one to use."
  fi
fi
echo "Account: $account"

# wrangler's `kv namespace create STORE` names it "<worker>-STORE". Keep the id in wrangler.toml, so a
# later `wrangler deploy` from a machine that can run it uses this namespace and not another.
want="$name-STORE"
namespace=$(cf "$api/accounts/$account/storage/kv/namespaces?per_page=100" |
  python3 -c '
import json, sys
answer = json.load(sys.stdin)
if not answer.get("success"):
    sys.exit("Cloudflare refused: the namespaces could not be listed")
for ns in answer["result"]:
    if ns.get("title") == sys.argv[1]:
        print(ns["id"])
        break
' "$want")
if [ -z "$namespace" ]; then
  namespace=$(cf -X POST "$api/accounts/$account/storage/kv/namespaces" \
    -H 'Content-Type: application/json' \
    -d "$(python3 -c 'import json, sys; print(json.dumps({"title": sys.argv[1]}))' "$want")" |
    field result.id)
  echo "KV: made $want → $namespace"
else
  echo "KV: $want → $namespace"
fi
[ -n "$namespace" ] || die "The KV namespace could not be made or found."

python3 - "$namespace" <<'EOF'
import re, sys
namespace = sys.argv[1]
text = open("wrangler.toml").read()
if not re.search(r"^\[\[kv_namespaces\]\]", text, re.M):
    sys.exit("wrangler.toml has no [[kv_namespaces]] for STORE")
fixed, count = re.subn(r'(\[\[kv_namespaces\]\][^\[]*?^\s*id\s*=\s*")[^"]*(")',
                       lambda m: m.group(1) + namespace + m.group(2), text, count=1, flags=re.M | re.S)
if not count:
    sys.exit("wrangler.toml's [[kv_namespaces]] has no id line")
open("wrangler.toml", "w").write(fixed)
EOF

# The module and its bindings: the namespace, and the vars exactly as wrangler.toml has them.
metadata=$(mktemp)
python3 - "$namespace" "$metadata" <<'EOF'
import json, re, sys
namespace, out = sys.argv[1:]
text = open("wrangler.toml").read()

def string(key, default=None):
    found = re.search(r'^\s*%s\s*=\s*"([^"]*)"' % re.escape(key), text, re.M)
    return found.group(1) if found else default

def number(key, default):
    found = re.search(r"^\s*%s\s*=\s*(\d+)" % re.escape(key), text, re.M)
    return found.group(1) if found else default

compat = string("compatibility_date")
if not compat:
    sys.exit("wrangler.toml has no compatibility_date")
vars = [("GITHUB_REPO", string("GITHUB_REPO")),
        ("GITHUB_WORKFLOW", string("GITHUB_WORKFLOW", "build.yml")),
        ("GITHUB_REF", string("GITHUB_REF", "main")),
        ("PER_DAY", number("PER_DAY", "40")),
        ("PER_ADDRESS", number("PER_ADDRESS", "16"))]
missing = [k for k, v in vars if not v]
if missing:
    sys.exit("wrangler.toml has no " + ", ".join(missing))
bindings = [{"type": "kv_namespace", "name": "STORE", "namespace_id": namespace}]
bindings += [{"type": "plain_text", "name": k, "text": v} for k, v in vars]
json.dump({"main_module": "index.js", "compatibility_date": compat, "bindings": bindings},
          open(out, "w"))
EOF

echo "Uploading src/index.js…"
cf -X PUT "$api/accounts/$account/workers/scripts/$name" \
  -F "metadata=@$metadata;type=application/json" \
  -F "index.js=@$here/src/index.js;type=application/javascript+module" >/dev/null
rm -f "$metadata"

# A secret goes in its own call: a binding of type secret_text in the upload would put it in the
# request's metadata, and it is not a var anyone should read back.
python3 -c '
import json, os, sys
json.dump({"name": "GITHUB_TOKEN", "text": os.environ["GITHUB_TOKEN"], "type": "secret_text"},
          sys.stdout)
' | cf -X PUT "$api/accounts/$account/workers/scripts/$name/secrets" \
  -H 'Content-Type: application/json' --data @- >/dev/null
echo "Secret: GITHUB_TOKEN set"

# workers.dev: the account's subdomain, made on the first Worker, then this script in front of it.
subdomain=$(cf "$api/accounts/$account/workers/subdomain" | field result.subdomain || true)
if [ -z "$subdomain" ]; then
  subdomain=${CLOUDFLARE_SUBDOMAIN:-}
  [ -n "$subdomain" ] || subdomain=$(printf '%s' "$name" | tr -c 'a-z0-9-' '-' | cut -c1-32)
  case "$subdomain" in
    [a-z0-9]*) ;;
    *) die "CLOUDFLARE_SUBDOMAIN must start with a letter or a digit" ;;
  esac
  cf -X PUT "$api/accounts/$account/workers/subdomain" -H 'Content-Type: application/json' \
    -d "$(python3 -c 'import json, sys; print(json.dumps({"subdomain": sys.argv[1]}))' "$subdomain")" \
    >/dev/null
  echo "workers.dev: $subdomain (the account's first Worker)"
fi
cf -X PUT "$api/accounts/$account/workers/scripts/$name/subdomain" \
  -H 'Content-Type: application/json' \
  -d '{"enabled":true,"previews_enabled":false}' >/dev/null

url="https://$name.$subdomain.workers.dev"
echo
echo "Deployed: $url"
echo "Put that in Horizon: Settings → Native mods → Build server."
