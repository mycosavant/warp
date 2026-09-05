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
#   GIT_RELEASE_TAG      `ChannelState::app_version()` is `option_env!` of this,
#                        read at compile time. Unset, the About page shows the
#                        literal `v#.##.###` and `--version` says `<unknown>` --
#                        that placeholder means NO version was stamped, it is not
#                        a format, and the binary then contains nothing at all
#                        that identifies its commit. With two checkouts in play
#                        that has cost real time.
#
# Setting a version is only safe because `fork::autoupdate_allowed` exists. Two
# of the three guards on the self-replace path are `app_version().is_none()`, so
# before that predicate landed, stamping a version to answer "which commit is
# this" would have made the build eligible to be replaced from Warp's servers.
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
export GIT_RELEASE_TAG="v0.fork.$sha"
export CARGO_BUILD_JOBS=8

echo "=== building warp-oss ($profile_dir) as $GIT_RELEASE_TAG at $(date +%H:%M:%S) ==="
cargo build --bin warp-oss --features gui,warp_control_cli "${profile_args[@]}" "$@"

bin="target/$profile_dir/warp-oss"
echo "=== $bin  $(date -r "$bin" '+%Y-%m-%d %H:%M:%S')  $(stat -c%s "$bin") bytes ==="

# Cheapest proof that both env vars actually took. A build missing
# warp_control_cli answers "unexpected argument" to every warpctrl command,
# which reads as a broken feature rather than a missing flag; and a version that
# did not reach warp_core reports `<unknown>` here while the build looks fine.
echo "=== version: $("./$bin" --version 2>&1 | head -1) ==="
if "./$bin" --warpctrl instance list >/dev/null 2>&1; then
  echo "=== warpctrl: present ==="
else
  echo "=== warpctrl: MISSING - check the --features line ==="
fi
