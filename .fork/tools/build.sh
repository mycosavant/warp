#!/usr/bin/env bash
#
# Build the fork's Linux/WSL GUI binary, capped and stamped.
#
# Two things this does that a bare `cargo build` does not:
#
#   CARGO_BUILD_JOBS=8   A single rustc on the `warp` crate was sampled at
#                        15.1 GB (2026-09-04). Windows and the WSL guest draw on
#                        the same 64 GB, so an uncapped build here is one half of
#                        the pair that took the guest down. Never build on both
#                        sides at once.
#
#   warp-oss.version     Written beside the binary after a successful build,
#                        holding `v0.fork.<sha>[-dirty]`. `ChannelState::
#                        app_version()` reads it at startup when no
#                        GIT_RELEASE_TAG was compiled in, so the About page,
#                        `--version`, the discovery record and the remote
#                        server's handshake all name the commit. Unstamped, the
#                        About page shows the literal `v#.##.###` and
#                        `--version` says `<unknown>` -- that placeholder means
#                        NO version, it is not a format.
#
#                        This used to `export GIT_RELEASE_TAG` instead, which
#                        compiles the tag into warp_core. Measured 2026-09-05:
#                        a changed tag invalidates 55 crates, so every commit
#                        became a near-clean release build, twenty minutes and
#                        a 41 GB peak. Never set GIT_RELEASE_TAG here again.
#
# Naming a version is only safe because `fork::autoupdate_allowed` exists. Two
# of the three guards on the self-replace path are `app_version().is_none()`, so
# before that predicate landed, answering "which commit is this" would have made
# the build eligible to be replaced from Warp's servers.
#
# `--features gui,warp_control_cli` is not optional: `warp_control_cli` is in no
# `default` list, and without it the binary has no `--warpctrl` at all.
#
# Usage:  .fork/tools/build.sh [--debug] [extra cargo args...]

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

profile_args=(--release)
profile_dir=release
if [[ "${1:-}" == "--debug" ]]; then
  profile_args=()
  profile_dir=debug
  shift
fi

sha=$(git rev-parse --short HEAD)
[[ -n "$(git status --porcelain)" ]] && sha="$sha-dirty"
version="v0.fork.$sha"
export CARGO_BUILD_JOBS=8
# A stray GIT_RELEASE_TAG in the environment would silently put the cascade
# back; the sidecar is the only stamp this script makes.
unset GIT_RELEASE_TAG

echo "=== building warp-oss ($profile_dir), to be named $version, at $(date +%H:%M:%S) ==="
cargo build --bin warp-oss --features gui,warp_control_cli "${profile_args[@]}" "$@"

bin="target/$profile_dir/warp-oss"
# Only after a successful build, so a failed one leaves the old binary and the
# old sidecar agreeing with each other.
printf '%s\n' "$version" > "$bin.version"
echo "=== $bin  $(date -r "$bin" '+%Y-%m-%d %H:%M:%S')  $(stat -c%s "$bin") bytes ==="

# Cheapest proof that the feature and the sidecar both took. A build missing
# warp_control_cli answers "unexpected argument" to every warpctrl command,
# which reads as a broken feature rather than a missing flag; and a sidecar the
# binary did not find reports `<unknown>` here while the build looks fine.
echo "=== version: $("./$bin" --version 2>&1 | head -1) ==="
if "./$bin" --warpctrl instance list >/dev/null 2>&1; then
  echo "=== warpctrl: present ==="
else
  echo "=== warpctrl: MISSING - check the --features line ==="
fi
