#!/usr/bin/env bash
# What every job needs first: the commit, its source as the sandbox expects it, and the build
# scripts. The sandbox's paths are the build cache's, so they never change:
# ~/horizon-android-build, ~/android, ~/.rustup.
set -euo pipefail

repo=${HORIZON_REPO:-https://github.com/ludovic111/horizon}
path=${HORIZON_REPO_PATH:-ludovic111/horizon}

# The launcher sends the commit it was built from; a release can fall back to versions.json.
if [ -z "${COMMIT:-}" ]; then
  COMMIT=$(python3 - <<'EOF'
import json, os, sys
versions = json.load(open("versions.json"))
version = os.environ["VERSION"]
if version not in versions:
    sys.exit(f"No commit for version {version!r} in versions.json; add it.")
print(versions[version])
EOF
)
fi
[[ "$COMMIT" =~ ^[0-9a-f]{7,40}$ ]] || { echo "Not a commit: $COMMIT"; exit 1; }
# git fetch wants a full object name; an older launcher sends the short one. The API resolves it,
# and the cache key is then the same whether the phone or a release names the commit.
if [ "${#COMMIT}" -lt 40 ]; then
  api=${HORIZON_API:-https://api.github.com}
  full=$(curl -fsS "$api/repos/$path/commits/$COMMIT" |
    python3 -c 'import json, sys; print(json.load(sys.stdin)["sha"])') ||
    { echo "Cannot find commit $COMMIT in $path"; exit 1; }
  [[ "$full" =~ ^[0-9a-f]{40}$ ]] || { echo "Not a commit: $full"; exit 1; }
  COMMIT=$full
fi
if [ -n "${GITHUB_ENV:-}" ]; then echo "COMMIT=$COMMIT" >> "$GITHUB_ENV"; fi
if [ -n "${GITHUB_OUTPUT:-}" ]; then echo "commit=$COMMIT" >> "$GITHUB_OUTPUT"; fi

root=$HOME/horizon-android-build
mkdir -p "$root/jobs" "$root/base"
git init -q "$root/source"
git -C "$root/source" fetch -q --depth 1 "$repo" "$COMMIT"
git -C "$root/source" checkout -q --detach FETCH_HEAD

# The scripts of the commit built, not of a branch: they match its source. native_build.sh itself
# runs from the checkout; these run outside it.
mkdir -p tools
for name in mod_build_sandbox.sh mod_build_server.py mod_build_once.py; do
  curl -fsS --retry 5 -o "tools/$name" \
    "https://raw.githubusercontent.com/$path/$COMMIT/packaging/android/$name"
done
chmod +x tools/*.sh

# What the build reads when it starts, in the sandbox's home.
cat > "$HOME/.profile" <<'EOF'
export ANDROID_HOME=$HOME/android
EOF
