#!/usr/bin/env bash
#
# Build the fork's Linux/WSL GUI binary, capped and stamped.
#
# Two things this does that a bare `cargo build` does not:
#
#   CARGO_BUILD_JOBS     Honoured if the environment already set one; otherwise
#                        cargo's default (one job per core) is used. This was a
#                        hardcoded `=8` until 2026-09-11, when the clean uncapped
#                        build the cap was chosen against was finally run:
#                        `.fork/runs/uncapped-2026-09-11/`.
#
#                        The measurement: at `-j 32`, the entire parallel front
#                        summed to 9,229 MB across 21 concurrent compilers --
#                        LESS than the `warp` crate compiling by itself (12,706
#                        MB exact). `MemAvailable` bottomed at 25,871 MB of ~39
#                        GB, and it did so while exactly ONE compiler was
#                        running. So `-j` never stood between this build and the
#                        wall, at any width.
#
#                        The old extrapolation was wrong in an instructive way:
#                        "eight jobs averaged ~470 MB each, so thirty-two is ~15
#                        GB" averaged the eight crates that happened to be
#                        resident at a sampler tick. Uncapped, 31 compilers
#                        summed to 4,893 MB -- ~158 MB each. A wider front
#                        recruits SMALLER crates, because the big ones are the
#                        graph's tail and compile alone whatever `-j` says.
#
#                        One cost, measured: uncapping raises the SINGLE-crate
#                        peak, 11,342 MB at `-j 8` to 12,706 MB at `-j 32`,
#                        +12%. The jobserver bounds rustc's internal
#                        codegen-unit parallelism, so a wider `-j` lets one
#                        rustc run more of its 64 units at once.
#
#                        What is untouched by all of that is the hazard that
#                        actually took the guest down in 2026-08-29: an uncapped
#                        WSL build CONCURRENT WITH A WINDOWS ONE. That pressure
#                        is one level up, on the host's 64 GB, and the run above
#                        was deliberately the opposite -- a clean host with
#                        nothing else on it. **Never build on both sides at
#                        once** is therefore the rule that survives, and it is
#                        now the only one. `CARGO_BUILD_JOBS=8` is the knob for
#                        the situations where it cannot be honoured: a remote
#                        session where a dead VM costs the link, or a desk where
#                        something else memory-hungry is running.
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
# Uncapped unless the caller asked for a cap; see the header. Written as an
# `if` rather than `[[ ... ]] && unset`, because under `set -e` that form exits
# the script on the branch where a cap IS set -- the test is false, the `&&`
# short-circuits, and the line's status is 1.
if [[ -z "${CARGO_BUILD_JOBS:-}" ]]; then
  unset CARGO_BUILD_JOBS
else
  echo "=== capped at CARGO_BUILD_JOBS=$CARGO_BUILD_JOBS (uncapped is the default since 2026-09-11) ==="
  export CARGO_BUILD_JOBS
fi
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
