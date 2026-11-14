#!/usr/bin/env bash
# What a finished build leaves behind: its library as a release asset, which a phone downloads
# without a token, and the answer the gateway is waiting for. Runs in the build.yml `publish` job.
set -euo pipefail

tag="build-$ASK"
status=result/status.json
[ -f "$status" ] || { echo "The build left no status.json"; exit 1; }

# The build may have failed for the mods' own fault: that is the answer, and there is nothing to publish.
state=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["state"])' "$status")
if [ "$state" = "done" ]; then
  [ -s result/libhorizon_android.so ] || { echo "The build left no library"; exit 1; }
  gh release create "$tag" \
    --title "build $ASK" \
    --notes "A build of Horizon Android with one player's native mods. Its library is downloaded by the launcher that asked for it." \
    --latest=false \
    result/libhorizon_android.so "$status" 2>/dev/null ||
    gh release upload "$tag" result/libhorizon_android.so "$status" --clobber
fi

# The gateway keeps the answer: the library's sha256 and size, the mods, and where the library is.
# A gateway that reads the run itself (`mod_build_gateway.py`) takes no report; that is not a failure.
python3 - "$GATEWAY" "$ASK" "$status" <<'EOF'
import json, sys, urllib.error, urllib.request

gateway, ask, path = sys.argv[1].rstrip("/"), sys.argv[2], sys.argv[3]
request = urllib.request.Request(
    f"{gateway}/v1/github/{ask}/state",
    data=json.dumps(json.load(open(path))).encode(),
    method="POST",
    headers={"Content-Type": "application/json", "User-Agent": "horizon-mod-builds/github-actions"},
)
try:
    urllib.request.urlopen(request, timeout=120).read()
except urllib.error.HTTPError as e:
    if e.code != 404:
        raise
    print("this gateway reads the run itself")
EOF
