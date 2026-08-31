#!/bin/bash
# T14.17 two-sided calibration for the ACP path's permission events.
#
# Drives an ALREADY-RUNNING Warp through warpctrl and reads the event log after.
# Launch first, from the repo root:
#
#   WARP_FORK_EVENT_LOG=/tmp/t1417-events WARP_FORK_ACP_COMMAND="opencode acp" \
#     env -u WAYLAND_DISPLAY LIBGL_ALWAYS_SOFTWARE=1 ./target/release/warp-oss
#
# **Name the directory rather than passing `on`.** `on` resolves to
# `fork::state_dir()/events`, and `state_dir()` is `secure_state_dir()` joined
# with "fork" -- platform-dependent, and not somewhere a reader should guess. A
# reader that guesses wrong finds an empty directory and reports "no permission
# events", which is indistinguishable from an instrument that never wrote any.
# That false negative was caught here before it fired, and the fix is to delete
# the guess rather than to make a better one. Override with EVENTS=<dir> if you
# launched with something else.
#
# and check the binary postdates your source before believing any run --
# `date -r target/release/warp-oss` against the newest file you touched. A build
# started before a fix and run after it measures the pre-fix binary, which cost
# a rebuild and a wrong conclusion on 2026-08-30.
#
# WHY IT FIRES BOTH WAYS, which is the whole point. `CLAUDE.md`'s rule for a new
# instrument is to make it fire on a known-present phenomenon and stay silent on
# a known-absent one. That is not pedantry here: if the permission events were
# never written at all, the known-absent case would pass perfectly and the run
# would read as a success. It is the same trap as checking a permission
# allowlist with a command that must *pass* rather than one that must *ask*.
#
# The known-present case is a file write, because `opencode.json` in this repo
# sets `edit: ask`. The known-absent case is a turn with no tool calls -- and it
# is chosen so the agent must actually answer rather than sit silent, since a
# turn that runs nothing AND says nothing is zero evidence either way.
set -u
REPO=/home/effatha/git/warp
CTL="$REPO/target/release/warp-oss --warpctrl"
EVENTS=${EVENTS:-/tmp/t1417-events}
T=/tmp/t1417; mkdir -p $T
say() { printf '\n\n========== %s ==========\n' "$*"; }

say "instance (must not say no_instance)"
$CTL instance list --output-format json 2>&1 | head -20

say "pane into the repo -- the ACP agent resolves opencode.json from the pane cwd"
$CTL input submit 'cd /home/effatha/git/warp' 2>&1 | head -3
sleep 3

say "KNOWN ABSENT: no tool calls must write no permission events"
$CTL agent prompt --prompt 'Reply with exactly: PONG. Use no tools, read no files, run no commands.' --output-format json 2>&1 | tee $T/absent.json | head -20
for i in $(seq 1 25); do
  $CTL agent list --output-format json 2>/dev/null > $T/list.json
  grep -q '"is_busy":true' $T/list.json || break
  sleep 4
done
echo "-- approvals after the quiet turn (expect none):"
$CTL agent approvals --output-format json 2>&1 | head -10

say "KNOWN PRESENT: a file write must raise an edit request Warp can answer"
$CTL agent prompt --prompt 'Create a file at target/t1417-probe.txt whose entire contents are the single word: hello' --output-format json 2>&1 | head -15
for i in $(seq 1 30); do
  $CTL agent approvals --output-format json 2>/dev/null > $T/appr.json
  python3 - "$T/appr.json" <<'PY' && break
import json,sys
try: a=json.load(open(sys.argv[1])).get('approvals',[])
except Exception: a=[]
raise SystemExit(0 if a else 1)
PY
  sleep 4
done
echo "-- what parked:"; cat $T/appr.json

read -r ID DG CA < <(python3 - "$T/appr.json" <<'PY'
import json,sys
a=json.load(open(sys.argv[1])).get('approvals',[])
print(a[0]['approval_id'], a[0]['digest'], a[0]['can_approve']) if a else print('', '', '')
PY
)
echo "approval_id=$ID can_approve=$CA"
if [ -n "${ID:-}" ]; then
  say "answer it (digest goes as --digest, never positionally)"
  $CTL agent approve "$ID" --digest "$DG" --output-format json 2>&1 | head -15
  say "READ BACK: it must have left the list before anything downstream is believed"
  $CTL agent approvals --output-format json 2>&1 | head -10
fi
for i in $(seq 1 25); do
  $CTL agent list --output-format json 2>/dev/null > $T/list.json
  grep -q '"is_busy":true' $T/list.json || break
  sleep 4
done

say "THE INSTRUMENT"
if [ ! -d "$EVENTS" ] || [ -z "$(ls -A "$EVENTS" 2>/dev/null)" ]; then
  echo "  !! $EVENTS is missing or empty."
  echo "  !! This is an APPARATUS failure, not a finding: Warp was not launched"
  echo "  !! with WARP_FORK_EVENT_LOG pointing here. Do NOT read a zero off this"
  echo "  !! run -- relaunch and repeat. A measurement whose apparatus could have"
  echo "  !! produced it is worse than no measurement."
  exit 2
fi
cat "$EVENTS"/*.jsonl 2>/dev/null | python3 -c "
import json,sys,collections
rows=[];c=collections.Counter()
for line in sys.stdin:
    try: d=json.loads(line)
    except Exception: continue
    c[d.get('event')]+=1
    if str(d.get('event','')).startswith('permission_'): rows.append(d)
print('counts:')
for k,v in sorted(c.items(), key=lambda kv:-kv[1]): print(f'  {v:4d}  {k}')
print()
if not rows: print('NO PERMISSION EVENTS AT ALL -- see verdict below')
for d in rows:
    print('---')
    for k in ('event','decision','answered_by','can_approve','tool_name',
              'call_id','tool_input_preview','summary'):
        if k in d: print(f'  {k}: {d[k]}')
"
say "VERDICT"
REQ=$(cat "$EVENTS"/*.jsonl 2>/dev/null | grep -c '"event":"permission_request"' || true)
REP=$(cat "$EVENTS"/*.jsonl 2>/dev/null | grep -c '"event":"permission_replied"' || true)
echo "  permission_request: ${REQ:-0}   permission_replied: ${REP:-0}"
if [ -f "$REPO/target/t1417-probe.txt" ]; then
  echo "  probe file EXISTS -- an edit landed"
  [ "${REQ:-0}" = "0" ] && echo "  >>> FALSIFIER FIRED: an edit landed with zero asks recorded." \
                        || echo "  >>> instrument holds: the edit that landed was asked about."
else
  echo "  probe file absent -- no edit landed; falsifier does not apply"
fi
echo
echo "  Stop Warp with:  $CTL window close"
echo "  (cancel any in-flight turn first, or it will not close)"
