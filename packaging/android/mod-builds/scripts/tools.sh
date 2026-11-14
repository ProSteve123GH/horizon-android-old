#!/usr/bin/env bash
# What the clean cache holds besides the build: the toolchain, the Android tools, the crates, and
# horizon-mixins, none of which the sandbox can download. Only for base.yml.
set -euo pipefail
root=$HOME/horizon-android-build

rm -rf "$HOME/.rustup" "$HOME/.cargo"
# The sandbox mounts .rustup read-only, so the toolchain the game pins (rust-toolchain.toml) and the
# target it builds for must be present here or the sandbox tries to download them and fails on the
# read-only tree. Install exactly what the game asks for, down to its components.
channel=$(sed -n 's/^channel = "\([^"]*\)"/\1/p' "$root/source/rust-toolchain.toml" | head -1)
[ -n "$channel" ] || channel=stable
mapfile -t components < <(sed -n 's/^components = \["\(.*\)"\]/\1/p' "$root/source/rust-toolchain.toml" | head -1 | tr -d '"' | tr ',' '\n' | sed 's/^ //')
# shellcheck disable=SC2086
curl -fsSL https://sh.rustup.rs | sh -s -- -y --no-modify-path --profile minimal \
  --default-toolchain "$channel" --target aarch64-linux-android \
  $(for c in "${components[@]}"; do printf -- "--component %s " "$c"; done)
# shellcheck disable=SC1091
. "$HOME/.cargo/env"

# `yes` ends on a broken pipe when the licenses are all taken, which pipefail would count as a failure.
(yes || true) | "$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" --sdk_root="$HOME/android" \
  --install "build-tools;35.0.0" "platforms;android-36" "ndk;28.2.13676358" | tail -3
ls "$HOME/android/ndk/28.2.13676358/toolchains" "$HOME/android/build-tools/35.0.0/aapt2" \
  "$HOME/android/platforms/android-36/android.jar"

rev=$(sed -n 's/^pub const MIXINS_REV: &str = "\([0-9a-f]*\)";/\1/p' \
  "$root/source/launcher/src/native_mods.rs" | head -1)
[ -n "$rev" ] || { echo "launcher/src/native_mods.rs no longer names MIXINS_REV"; exit 1; }
git_url=$(sed -n 's/^MIXINS_GIT=//p' "$root/source/packaging/android/native_build.sh" | head -1)
cargo install --locked --git "$git_url" --rev "$rev" --root "$root/base/mixins-${rev:0:12}" \
  horizon-mixins-cli

cargo fetch --locked --manifest-path "$root/source/Cargo.toml"

# The sandbox's home has this folder as its .cargo.
rm -rf "$root/cargo"
mv "$HOME/.cargo" "$root/cargo"
