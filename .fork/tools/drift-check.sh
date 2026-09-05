#!/usr/bin/env bash
#
# Weekly upstream drift report. Reports; never merges.
#
# The automation stops at the report on purpose. Every merge this fork has done
# has needed judgment at the conflict -- which side's doc comment is still true,
# whether a removed import is dead in the merged body, whether a widened
# signature reaches a fork seam. None of that is decidable from a count, and a
# script that merged on a green count would be asserting it is.
#
# What it costs: one fetch. It never writes the index or the working tree.
# `merge-tree --write-tree` writes objects to the object database and reports
# the conflicts a real merge would raise, without touching either.
#
# Read the three numbers in this order:
#
#   overlap    files both sides changed since the merge base. This is the one
#              that predicts cost. Divergence does not -- the fork can add 300
#              files upstream never touches and merge clean.
#   conflicts  what `git merge` would actually stop on today.
#   crash-ish  a FLOOR, not a count. Calibrated 2026-09-04 against the 94-commit
#              backlog: reading the subjects by hand found 16 stability fixes,
#              this grep finds 6. The ten it misses are the ones phrased as
#              their fix rather than their symptom -- "Guard reversed glyph
#              bounds", "Don't start MAA from buffered child event after
#              teardown begins", "Recover the renderer after Windows RDP device
#              loss". No pattern reaches those. Treat any non-zero number as
#              "go read the log", and never as the severity of what is waiting.
#              (The first version of this grep also reported 5 because `hang`
#              matched inside "Change owner of vertical tabs". Word boundaries
#              now, which is why the pattern is spelled out longhand.)
#
# Each is printed with its change since the last run, because the argument for
# merging is the trend, not the level.
#
# Usage:  drift-check.sh [--no-fetch] [--repo <path>]

set -uo pipefail

REPO="${WARP_FORK_REPO:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
BRANCH="${WARP_FORK_BRANCH:-dev}"
UPSTREAM="${WARP_FORK_UPSTREAM:-upstream/master}"
DO_FETCH=1

while (( $# )); do
  case "$1" in
    --no-fetch) DO_FETCH=0; shift ;;
    --repo)     REPO="$2"; shift 2 ;;
    -h|--help)  sed -n '2,28p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "drift-check: unknown argument: $1" >&2; exit 2 ;;
  esac
done

STATE_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/warp-fork"
STATE="$STATE_DIR/drift.tsv"
mkdir -p "$STATE_DIR"

git -C "$REPO" rev-parse --git-dir >/dev/null 2>&1 || {
  echo "drift-check: $REPO is not a git repository" >&2; exit 2; }

if (( DO_FETCH )); then
  if ! git -C "$REPO" fetch --quiet "${UPSTREAM%%/*}" 2>/dev/null; then
    echo "drift-check: could not fetch ${UPSTREAM%%/*} -- reporting against the refs on disk" >&2
  fi
fi

git -C "$REPO" rev-parse --verify --quiet "$UPSTREAM" >/dev/null || {
  echo "drift-check: no such ref: $UPSTREAM" >&2; exit 2; }
git -C "$REPO" rev-parse --verify --quiet "$BRANCH" >/dev/null || {
  echo "drift-check: no such ref: $BRANCH" >&2; exit 2; }

# Computed, never pasted. A base handed in by hand once published this fork's
# divergence as 1168 files when it was 204: `git diff A...B` silently degrades
# to a two-dot diff when A is already an ancestor of B, so a stale base reads as
# a clean one.
BASE="$(git -C "$REPO" merge-base "$UPSTREAM" "$BRANCH")"

up_commits=$(git -C "$REPO" rev-list --count "$BASE..$UPSTREAM")
fork_commits=$(git -C "$REPO" rev-list --count "$BASE..$BRANCH")

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
git -C "$REPO" diff --name-only "$BASE" "$UPSTREAM" | sort > "$tmp/up"
git -C "$REPO" diff --name-only "$BASE" "$BRANCH"   | sort > "$tmp/fork"
comm -12 "$tmp/up" "$tmp/fork" > "$tmp/overlap"
overlap=$(wc -l < "$tmp/overlap")

# --write-tree reports what a merge would hit without performing one. Exit 1
# means conflicts; the conflicted paths are the lines before the first blank.
mt_out=$(git -C "$REPO" merge-tree --write-tree --name-only "$BRANCH" "$UPSTREAM" 2>/dev/null)
mt_rc=$?
if (( mt_rc == 0 )); then
  conflicts=0; conflict_files=""
elif (( mt_rc == 1 )); then
  conflict_files=$(printf '%s\n' "$mt_out" | sed -n '2,/^$/p' | sed '/^$/d')
  conflicts=$(printf '%s' "$conflict_files" | grep -c . )
else
  echo "drift-check: merge-tree failed (rc=$mt_rc); it needs git 2.38+" >&2
  conflicts=-1; conflict_files=""
fi

CRASHISH_RE='\b(crash|crashes|crashed|panic|panics|panicked|hang|hangs|hung'
CRASHISH_RE+='|deadlock|deadlocks|segfault|use-after-free|race|regression'
CRASHISH_RE+='|flake|flaky|leak|leaks|overflow|OOM)\b'
crashish=$(git -C "$REPO" log --format='%s' "$BASE..$UPSTREAM" \
  | grep -icP "$CRASHISH_RE" || true)

# Age of the drift: when the oldest unmerged upstream commit landed.
oldest_epoch=$(git -C "$REPO" log --format=%ct --reverse "$BASE..$UPSTREAM" | head -1)
if [[ -n "$oldest_epoch" ]]; then
  days=$(( ( $(date +%s) - oldest_epoch ) / 86400 ))
else
  days=0
fi

prev=$(tail -n 1 "$STATE" 2>/dev/null)
delta() { # current, previous-field-index
  local cur="$1" idx="$2" p
  [[ -z "$prev" ]] && { printf '(first run)'; return; }
  p=$(printf '%s' "$prev" | cut -f"$idx")
  [[ -z "$p" || ! "$p" =~ ^-?[0-9]+$ ]] && { printf '(no prior)'; return; }
  local d=$(( cur - p ))
  if   (( d > 0 )); then printf '(+%d since %s)' "$d" "$(printf '%s' "$prev" | cut -f1)"
  elif (( d < 0 )); then printf '(%d since %s)'  "$d" "$(printf '%s' "$prev" | cut -f1)"
  else                   printf '(unchanged since %s)' "$(printf '%s' "$prev" | cut -f1)"; fi
}

today=$(date +%F)
echo "upstream drift — $today"
echo "  base            $(git -C "$REPO" log --format='%h %s' -1 "$BASE" | cut -c1-64)"
echo "  unmerged        $up_commits upstream commits, oldest $days days old $(delta "$up_commits" 2)"
echo "  fork ahead      $fork_commits commits $(delta "$fork_commits" 3)"
echo "  overlap         $overlap files changed by both $(delta "$overlap" 4)"
echo "  conflicts       $conflicts files $(delta "$conflicts" 5)"
echo "  crash-ish       $crashish of $up_commits subjects, a floor $(delta "$crashish" 6)"

if (( conflicts > 0 )); then
  echo
  echo "  would conflict:"
  printf '%s\n' "$conflict_files" | sed 's/^/    /'
fi

echo
echo "  no merge was performed, and this script cannot perform one."
echo "  when you do merge, the gates are in CLAUDE.md under \"Working rules\":"
echo "    cargo check --workspace --all-targets   (catches what a binary build hides)"
echo "    both catalog pins, the local_sync byte-stability tests"
echo "    a two-run baseline before believing any test-failure count"

printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
  "$today" "$up_commits" "$fork_commits" "$overlap" "$conflicts" "$crashish" >> "$STATE"
