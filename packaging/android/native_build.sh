#!/usr/bin/env bash
# Horizon with native mods (docs/android-native-mods.md): the mods' horizon-mixins bundles applied to this
# checkout, built as the Android game library (tools/android_build.py --rust-only). Runs on the
# build server (mod_build_sandbox.sh) and on GitHub's runners (packaging/android/mod-builds/).
#
#   native_build.sh SOURCE OUT [BUNDLE...]
#
# SOURCE is a Horizon checkout at the commit to build, only read (git archive). Each BUNDLE is a
# folder holding a horizon-mixin.toml (an unpacked .hnmod). With no bundles it builds the plain
# game, which fills the cache a mod build starts from. OUT gets libhorizon_android.so and
# build.json: the commit, profile, ABI, horizon-mixins, the mods, and the library's SHA-256.
#
# NATIVE_CACHE (default ~/.cache/horizon-android-native) keeps horizon-mixins and the build folder
# with its target between builds: a file a build leaves as it was keeps its time, so Cargo
# recompiles only the crates the mods change. PROFILE defaults to fast; ABI to arm64-v8a.
set -euo pipefail

MIXINS_GIT=https://github.com/horizon-rust/horizon-mixins

fail() { echo "native_build: $*" >&2; exit 1; }
[ $# -ge 2 ] || fail "usage: native_build.sh SOURCE OUT [BUNDLE...]"
source=$(cd "$1" && pwd)
mkdir -p "$2"
out=$(cd "$2" && pwd)
shift 2
cache=${NATIVE_CACHE:-$HOME/.cache/horizon-android-native}
profile=${PROFILE:-fast}
abi=${ABI:-arm64-v8a}
case "$abi" in
  arm64-v8a) triple=aarch64-linux-android ;;
  armeabi-v7a) triple=armv7-linux-androideabi ;;
  x86) triple=i686-linux-android ;;
  x86_64) triple=x86_64-linux-android ;;
  *) fail "unsupported Android ABI: $abi" ;;
esac
case "$profile" in
  fast|release) ;;
  *) fail "unsupported profile: $profile" ;;
esac
mkdir -p "$cache"

# Horizon's launcher's (launcher/src/native_mods.rs, MIXINS_REV): the tool the game itself builds
# with unless a mod pins another.
mixins_rev=$(sed -n 's/^pub const MIXINS_REV: &str = "\([0-9a-f]*\)";/\1/p' \
  "$source/launcher/src/native_mods.rs" | head -1)
[ -n "$mixins_rev" ] || fail "launcher/src/native_mods.rs no longer names MIXINS_REV"
tools=$cache/mixins-${mixins_rev:0:12}
if [ ! -x "$tools/bin/horizon-mixins" ]; then
  cargo install --locked --git "$MIXINS_GIT" --rev "$mixins_rev" --root "$tools" \
    horizon-mixins-cli
fi

commit=$(git -C "$source" rev-parse HEAD)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# The source as committed, without links (horizon-mixins refuses them and nothing needs them).
mkdir "$work/source"
git -C "$source" archive --format=tar HEAD | tar -x -C "$work/source"
find "$work/source" -type l -delete
echo "$commit" > "$work/source/.horizon-revision"

# The game module is compiled by two crates (horizon-client's binary and horizon-android from
# android_main.rs), and horizon-mixins refuses a module destination it reaches under two names:
# while it plans, horizon-android's library is an empty file. The lockfile stays as it is.
manifest=crates/horizon-android/Cargo.toml
lib_line='path = "../horizon-client/src/android_main.rs"'

if [ $# -ge 1 ]; then
  grep -qxF "$lib_line" "$work/source/$manifest" || fail "$manifest no longer has: $lib_line"
  sed -i "s|^path = \"../horizon-client/src/android_main.rs\"\$|path = \"plan-stub.rs\"|" \
    "$work/source/$manifest"
  : > "$work/source/crates/horizon-android/plan-stub.rs"

  # The bundles, each pinned to the commit built here when it pins one (a mod tested on another
  # revision calls what this one has).
  mkdir "$work/mods"
  for bundle in "$@"; do
    [ -f "$bundle/horizon-mixin.toml" ] || fail "$bundle has no horizon-mixin.toml"
    copy=$work/mods/$(basename "$bundle")
    [ ! -e "$copy" ] || fail "two bundles named $(basename "$bundle")"
    cp -r "$bundle" "$copy"
    sed -i -E "s|^horizon_revision = \"[0-9a-f]*\"|horizon_revision = \"$commit\"|" \
      "$copy/horizon-mixin.toml"
  done
  echo "native_build: build of $commit with $(ls "$work/mods" | tr '\n' ' ')"
  "$tools/bin/horizon-mixins" apply --workspace "$work/source" --mods "$work/mods" \
    --out "$work/stage"
  # horizon-android's manifest goes back to building android_main.rs.
  git -C "$source" show "HEAD:$manifest" > "$work/stage/$manifest"
  rm -f "$work/stage/crates/horizon-android/plan-stub.rs"
else
  echo "native_build: base build of $commit"
  cp -a "$work/source" "$work/stage"
fi

# Into the build folder: a file whose bytes changed gets the time of now (no -t), one that did not
# keeps its time, so Cargo recompiles what changed even when the commit is older than the last
# build.
build=$cache/build-$abi
mkdir -p "$build"
rsync -r --checksum --delete --exclude /target/ "$work/stage/" "$build/"

rm -f "$build"/target/"$triple"/"$profile"/libhorizon_android.so
(cd "$build" && HORIZON_ANDROID_BUILD_COMMIT="$commit" \
  python3 tools/android_build.py --rust-only --abi "$abi" --profile "$profile")

library=$build/target/$triple/$profile/libhorizon_android.so
[ -f "$library" ] || fail "the build made no $library"
cp "$library" "$out/libhorizon_android.so"
python3 - "$out" "$commit" "$profile" "$abi" "$mixins_rev" "$@" <<'EOF'
import hashlib, json, pathlib, sys, tomllib
out, commit, profile, abi, mixins, *bundles = sys.argv[1:]
out = pathlib.Path(out)
library = out / "libhorizon_android.so"
listed = []
for folder in bundles:
    mod = tomllib.loads((pathlib.Path(folder) / "horizon-mixin.toml").read_text())["mod"]
    listed.append({"id": mod["id"], "version": mod["version"]})
json.dump({
    "version": "",
    "commit": commit,
    "profile": profile,
    "abi": abi,
    "mixins": mixins,
    "mods": listed,
    "sha256": hashlib.sha256(library.read_bytes()).hexdigest(),
    "size": library.stat().st_size,
}, open(out / "build.json", "w"), indent=2)
EOF
cat "$out/build.json"
