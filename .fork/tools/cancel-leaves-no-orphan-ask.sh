#!/bin/bash
# T14.17: does cancelling a turn with a parked question record that nobody
# answered it?
#
# The unit tests pin `AsksNothingMore` itself -- a dropped guard writes
# `unanswered`, a disarmed one writes nothing. What they cannot pin is the
# claim the guard was built on: that cancellation drops the waiting task at all.
# That was read off `take_until` (`mod.rs:287`) and `registry`'s module docs, and
# a reading is what this fork does not ship on.
#
# Drives an ALREADY-RUNNING Warp. Launch it first, from the repo root:
#
#   WARP_FORK_EVENT_LOG=/tmp/t1417-cancel WARP_FORK_ACP_COMMAND="opencode acp" \
#     env -u WAYLAND_DISPLAY LIBGL_ALWAYS_SOFTWARE=1 ./target/release/warp-oss
#
# CHECK THE BINARY POSTDATES THE FIX before believing a pass -- a release build
# started before a fix and run after it measures the pre-fix binary, which cost a
# rebuild and a wrong conclusion on 2026-08-30.
#
# ---------------------------------------------------------------------------
# BOTH HALVES, because the passing one cannot fail on its own. If the guard
# never fired, phase B alone would still show a tidy log and read as a success;
# what distinguishes a working guard from a dead one is that the ANSWERED turn
# does *not* also write `unanswered`. A disarm that silently stopped working
# would double every answer, and only phase B catches that.
set -u
REPO=/home/effatha/git/warp
CTL="$REPO/target/release/warp-oss --warpctrl"
EVENTS=${EVENTS:-/tmp/t1417-cancel}
T=/tmp/t1417-cancel-work; mkdir -p "$T"
say() { printf '\n\n========== %s ==========\n' "$*"; }

# Compact, because the event log is `serde_json::to_string`. warpctrl's own
# --output-format json is PRETTY-printed and these patterns would be wrong for it.
unanswered() { cat "$EVENTS"/*.jsonl 2>/dev/null | grep -c '"decision":"unanswered"' || true; }
requests()   { cat "$EVENTS"/*.jsonl 2>/dev/null | grep -c '"event":"permission_request"' || true; }
replies()    { cat "$EVENTS"/*.jsonl 2>/dev/null | grep -c '"event":"permission_replied"' || true; }

busy() {
  $CTL agent list --output-format json 2>/dev/null | python3 -c "
import json,sys
try: d=json.load(sys.stdin)
except Exception: print('unknown'); raise SystemExit
print('yes' if any(c.get('is_busy') for c in d.get('conversations',[])) else 'no')"
}

# `PendingApproval` carries no conversation id -- checked, it has `session_id`
# and `tab_id` and neither is what `agent cancel` takes. So the conversation is
# read off `agent list` as the busy one, which is also the more honest source:
# it is the turn actually in flight rather than a field assumed to mean that.
busy_conversation() {
  $CTL agent list --output-format json 2>/dev/null | python3 -c "
import json,sys
try: d=json.load(sys.stdin)
except Exception: raise SystemExit
for c in d.get('conversations',[]):
    if c.get('is_busy'): print(c['conversation_id']); break"
}

# Returns approval_id and digest for the first parked question.
park_wait() {
  ID=""; DG=""
  for _ in $(seq 1 30); do
    $CTL agent approvals --output-format json 2>/dev/null > "$T/appr.json" || true
    eval "$(python3 - "$T/appr.json" <<'PY'
import json,sys
try: a = json.load(open(sys.argv[1])).get('approvals', [])
except Exception: a = []
if a:
    print(f"ID={a[0]['approval_id']!r}; DG={a[0].get('digest','')!r}")
PY
)"
    [ -n "$ID" ] && return 0
    sleep 3
  done
  return 1
}

say "instance (must not be empty)"
$CTL instance list --output-format json 2>&1 | head -12

say "pane into the repo -- opencode.json is what makes it ask at all"
$CTL input submit 'cd /home/effatha/git/warp' 2>&1 | head -3
sleep 3

# ---------------------------------------------------- A: the cancelled ask
say "A: park a question, then CANCEL the turn without answering it"
BEFORE_U=$(unanswered); BEFORE_REQ=$(requests); BEFORE_REP=$(replies)
echo "before -- unanswered:$BEFORE_U requests:$BEFORE_REQ replies:$BEFORE_REP"
rm -f "$REPO/target/t1417-cancel-probe.txt"
$CTL agent prompt 'Create a file at target/t1417-cancel-probe.txt whose entire contents are the single word: orphan' --output-format json 2>&1 | head -10

if park_wait; then
  CONV=$(busy_conversation)
  echo "parked: approval_id=$ID  conversation=${CONV:-<none busy>}"
  if [ -z "$CONV" ]; then
    echo "!! a question is parked but no conversation reports busy -- APPARATUS"
    echo "!! failure, not a finding. Cancelling the wrong turn proves nothing."
    exit 2
  fi
  say "cancel it -- no answer given"
  $CTL agent cancel "$CONV" --output-format json 2>&1 | head -10
  # READ BACK the mutation before measuring what follows it. An approval that
  # was never delivered once parked a turn for 171s and read exactly like a
  # wedge, with the instrument that tells them apart sitting right there.
  echo "-- approvals after cancel (the question must be gone):"
  $CTL agent approvals --output-format json 2>&1 | head -12
else
  echo "!! nothing ever parked -- APPARATUS failure, not a finding."
  echo "!! Check the pane cwd and that the agent is one that asks Warp at all."
  exit 2
fi
for _ in $(seq 1 15); do [ "$(busy)" = "yes" ] || break; sleep 3; done

AFTER_U=$(unanswered)
DU_A=$(( AFTER_U - BEFORE_U ))
DREQ_A=$(( $(requests) - BEFORE_REQ ))
DREP_A=$(( $(replies) - BEFORE_REP ))
echo
echo "A deltas -- request:+$DREQ_A  replied:+$DREP_A  unanswered:+$DU_A"
[ -f "$REPO/target/t1417-cancel-probe.txt" ] && echo "  !! the probe file EXISTS -- the write landed despite no answer" \
  || echo "  probe file absent, as a cancelled unanswered edit should be"

# ---------------------------------------------- B: the answered ask, once
say "B: an ANSWERED question must record its real decision and NOT 'unanswered'"
BEFORE_U2=$(unanswered); BEFORE_REP2=$(replies)
rm -f "$REPO/target/t1417-answered-probe.txt"
$CTL agent prompt 'Create a file at target/t1417-answered-probe.txt whose entire contents are the single word: answered' --output-format json 2>&1 | head -10
if park_wait; then
  echo "parked: approval_id=$ID"
  $CTL agent approve "$ID" --digest "$DG" --output-format json 2>&1 | head -10
  echo "-- approvals after approve (must be empty before believing anything downstream):"
  $CTL agent approvals --output-format json 2>&1 | head -8
else
  echo "  (nothing parked in phase B -- cannot judge the disarm)"
fi
for _ in $(seq 1 20); do [ "$(busy)" = "yes" ] || break; sleep 3; done
DU_B=$(( $(unanswered) - BEFORE_U2 ))
DREP_B=$(( $(replies) - BEFORE_REP2 ))
echo
echo "B deltas -- replied:+$DREP_B  unanswered:+$DU_B"

# ------------------------------------------------------------------ readout
say "THE LINES"
if [ ! -d "$EVENTS" ] || [ -z "$(ls -A "$EVENTS" 2>/dev/null)" ]; then
  echo "  !! $EVENTS is missing or empty -- APPARATUS failure. Warp was not"
  echo "  !! launched with WARP_FORK_EVENT_LOG pointing here. Read no zero off this."
  exit 2
fi
cat "$EVENTS"/*.jsonl 2>/dev/null | python3 -c "
import json,sys
for line in sys.stdin:
    try: d=json.loads(line)
    except Exception: continue
    if not str(d.get('event','')).startswith('permission_'): continue
    print('---')
    for k in ('event','decision','answered_by','can_approve','call_id','summary'):
        if k in d: print(f'  {k}: {d[k]}')
"

say "VERDICT"
OK=1
echo "  A (cancelled): request +$DREQ_A, replied +$DREP_A, unanswered +$DU_A"
echo "  B (answered):  replied +$DREP_B, unanswered +$DU_B"
echo
[ "$DREQ_A" -lt 1 ] && { echo "  INCONCLUSIVE: phase A recorded no ask, so nothing was orphaned to fix."; OK=0; }
[ "$DU_A" -lt 1 ] && { echo "  FAIL: a cancelled question wrote no 'unanswered' line. Either the guard"
                       echo "        does not fire, or cancellation does not drop the task the way"
                       echo "        mod.rs:287 was read to say it does."; OK=0; }
[ "$DU_B" -ne 0 ] && { echo "  FAIL: an ANSWERED question also wrote 'unanswered' ($DU_B). The disarm"
                       echo "        is not taking, and every answered permission is being doubled."; OK=0; }
[ "$DREP_B" -lt 1 ] && { echo "  (phase B recorded no reply -- the answered half is unjudged)"; OK=0; }
[ "$OK" = "1" ] && echo "  >>> CONFIRMED: the orphan is closed, and answering still disarms."
echo
echo "  Stop Warp with:  $CTL window close   (cancel any in-flight turn first)"
