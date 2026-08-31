#!/bin/bash
# T14.17 two-sided calibration for the ACP path's permission events.
#
# Drives an ALREADY-RUNNING Warp through warpctrl. Launch it first, from the
# repo root:
#
#   WARP_FORK_EVENT_LOG=/tmp/t1417-events WARP_FORK_ACP_COMMAND="opencode acp" \
#     env -u WAYLAND_DISPLAY LIBGL_ALWAYS_SOFTWARE=1 ./target/release/warp-oss
#
# **Name the log directory rather than passing `on`.** `on` resolves to
# `fork::state_dir()/events`, and `state_dir()` is `secure_state_dir()` joined
# with "fork" -- platform-dependent, and on macOS routed through an app-group
# container. A reader that guesses wrong finds an empty directory and reports
# "no permission events", which is indistinguishable from an instrument that
# never wrote any. Delete the guess; do not improve it. Override with EVENTS=.
#
# And check the binary postdates your source before believing any run:
# `date -r target/release/warp-oss` against the newest file you touched.
#
# NOTE the argument shapes, both learned by running this and being refused:
# `agent prompt <PROMPT>` and `agent read <CONVERSATION>` take their subject
# POSITIONALLY, not as `--prompt`/`--conversation`. Only the digest is a flag:
# `agent approve <ID> --digest <D>`.
#
# ---------------------------------------------------------------------------
# WHY IT FIRES BOTH WAYS. CLAUDE.md's rule for a new instrument is to make it
# fire on a known-present phenomenon and stay silent on a known-absent one.
# That is not pedantry here: if the permission events were never written at all,
# the known-absent half would pass perfectly and the run would read as a
# success. Same trap as checking an allowlist with a command that must *pass*
# rather than one that must *ask*.
#
# EACH HALF IS MEASURED ON ITS OWN DELTA, not on a total at the end. The log is
# cumulative over the session, so a final count that includes the known-present
# events can never establish that the known-absent phase wrote none. An earlier
# draft of this script made exactly that mistake.
set -u
REPO=/home/effatha/git/warp
CTL="$REPO/target/release/warp-oss --warpctrl"
EVENTS=${EVENTS:-/tmp/t1417-events}
PROBE="$REPO/target/t1417-probe.txt"
T=/tmp/t1417; mkdir -p "$T"
say() { printf '\n\n========== %s ==========\n' "$*"; }

# Counts permission events on disk. `warpctrl --output-format json` is
# PRETTY-printed, so `grep '"is_busy":true'` never matches -- an earlier draft
# used that and its wait loop broke instantly, which would have had the log read
# before anything was written. The event log itself is `serde_json::to_string`,
# i.e. compact, so these patterns are correct for the FILE and would be wrong
# for warpctrl output. Keep the two straight.
perm_count() { cat "$EVENTS"/*.jsonl 2>/dev/null | grep -c '"event":"permission_' || true; }
req_count()  { cat "$EVENTS"/*.jsonl 2>/dev/null | grep -c '"event":"permission_request"' || true; }
rep_count()  { cat "$EVENTS"/*.jsonl 2>/dev/null | grep -c '"event":"permission_replied"' || true; }

# Busy is read by parsing, never by grepping pretty JSON.
busy() {
  $CTL agent list --output-format json 2>/dev/null | python3 -c "
import json,sys
try: d=json.load(sys.stdin)
except Exception: print('unknown'); raise SystemExit
cs = d.get('conversations', d if isinstance(d, list) else [])
print('yes' if any(c.get('is_busy') for c in cs) else 'no')"
}
settle() {
  for _ in $(seq 1 "${2:-30}"); do
    [ "$(busy)" = "yes" ] || return 0
    sleep 4
  done
  echo "  (still busy after the wait -- $1)"
}

say "instance (must not be empty)"
$CTL instance list --output-format json 2>&1 | head -20

say "pane into the repo"
echo "The ACP agent resolves opencode.json from the PANE's cwd, and a fresh pane"
echo "starts in \$HOME -- so this decides whether its permission rules load at all."
$CTL input submit 'cd /home/effatha/git/warp' 2>&1 | head -3
sleep 3

# ------------------------------------------------------------- known absent
say "KNOWN ABSENT: a turn with no tool calls must write no permission events"
BEFORE_A=$(perm_count)
echo "permission events before: $BEFORE_A"
$CTL agent prompt 'Reply with exactly the word PONG and nothing else. Use no tools, read no files, run no commands.' --output-format json 2>&1 | head -25
settle "quiet turn"
AFTER_A=$(perm_count)
DELTA_A=$(( AFTER_A - BEFORE_A ))
echo "permission events after: $AFTER_A   delta: $DELTA_A"
echo "-- approvals now (expect none):"
$CTL agent approvals --output-format json 2>&1 | head -10

# ------------------------------------------------------------ known present
say "KNOWN PRESENT: a file write must raise an edit request Warp can answer"
BEFORE_P=$(perm_count)
BEFORE_REQ=$(req_count); BEFORE_REP=$(rep_count)
rm -f "$PROBE"
$CTL agent prompt 'Create a file at target/t1417-probe.txt whose entire contents are the single word: hello' --output-format json 2>&1 | head -20

echo "-- waiting for the ask to park"
ID=""; DG=""; CA=""
for _ in $(seq 1 30); do
  $CTL agent approvals --output-format json 2>/dev/null > "$T/appr.json" || true
  eval "$(python3 - "$T/appr.json" <<'PY'
import json,sys
try: a = json.load(open(sys.argv[1])).get('approvals', [])
except Exception: a = []
if a:
    print(f"ID={a[0]['approval_id']!r}; DG={a[0]['digest']!r}; CA={a[0]['can_approve']!r}")
PY
)"
  [ -n "$ID" ] && break
  sleep 4
done
echo "-- what parked:"; cat "$T/appr.json" 2>/dev/null | head -40
echo "approval_id=${ID:-<none>}  can_approve=${CA:-<none>}"

if [ -n "$ID" ]; then
  say "answer it (the digest goes as --digest, never positionally)"
  $CTL agent approve "$ID" --digest "$DG" --output-format json 2>&1 | head -20
  say "READ BACK before believing anything downstream"
  echo "After any mutation, confirm the mutation. An approval passed wrongly once"
  echo "parked a turn for 171s and read exactly like a wedge."
  $CTL agent approvals --output-format json 2>&1 | head -10
fi
settle "write turn"
AFTER_P=$(perm_count)
DELTA_P=$(( AFTER_P - BEFORE_P ))
DELTA_REQ=$(( $(req_count) - BEFORE_REQ ))
DELTA_REP=$(( $(rep_count) - BEFORE_REP ))

# ------------------------------------------------------------------ readout
say "THE INSTRUMENT"
if [ ! -d "$EVENTS" ] || [ -z "$(ls -A "$EVENTS" 2>/dev/null)" ]; then
  echo "  !! $EVENTS is missing or empty."
  echo "  !! APPARATUS failure, not a finding: Warp was not launched with"
  echo "  !! WARP_FORK_EVENT_LOG pointing here. Do NOT read a zero off this run."
  exit 2
fi
cat "$EVENTS"/*.jsonl 2>/dev/null | python3 -c "
import json,sys,collections
rows=[]; c=collections.Counter()
for line in sys.stdin:
    try: d=json.loads(line)
    except Exception: continue
    c[d.get('event')]+=1
    if str(d.get('event','')).startswith('permission_'): rows.append(d)
print('counts across the whole run:')
for k,v in sorted(c.items(), key=lambda kv:-kv[1]): print(f'  {v:4d}  {k}')
print()
for d in rows:
    print('---')
    for k in ('event','decision','answered_by','can_approve','tool_name',
              'call_id','tool_input_preview','summary'):
        if k in d: print(f'  {k}: {d[k]}')
"

say "VERDICT"
echo "  known ABSENT  delta: $DELTA_A   (must be 0)"
echo "  known PRESENT delta: $DELTA_P   (request +$DELTA_REQ, replied +$DELTA_REP; both must be >0)"
echo
OK=1
[ "$DELTA_A" -ne 0 ] && { echo "  FAIL: a turn with no tool calls wrote $DELTA_A permission events."; OK=0; }
[ "$DELTA_REQ" -lt 1 ] && { echo "  FAIL: the write turn recorded no permission_request."; OK=0; }
[ -n "$ID" ] && [ "$DELTA_REP" -lt 1 ] && { echo "  FAIL: an answer was given but no permission_replied was recorded."; OK=0; }
if [ -z "$ID" ]; then
  echo "  INCONCLUSIVE: nothing ever parked, so the firing half never ran."
  echo "  Check the pane cwd is the repo -- opencode.json is what makes it ask."
  OK=0
fi
if [ -f "$PROBE" ]; then
  echo "  probe file EXISTS -- an edit landed"
  if [ "$DELTA_REQ" -lt 1 ]; then
    echo "  >>> FALSIFIER FIRED: an edit landed with zero asks recorded. The"
    echo "      instrument captured only Warp's non-involvement."
  fi
else
  echo "  probe file absent -- no edit landed"
fi
[ "$OK" = "1" ] && echo && echo "  >>> CALIBRATED BOTH WAYS."
echo
echo "  Stop Warp with:  $CTL window close"
echo "  Cancel any in-flight turn first, or it will not close -- and the result"
echo "  now says close:requested rather than claiming the window shut."
