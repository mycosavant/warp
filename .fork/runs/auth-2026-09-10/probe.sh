#!/usr/bin/env bash
# Re-run the 2026-09-07 survey's two refusals, on this date, against this binary.
# No Warp process, no GUI, no credential.
set -u
W=/home/effatha/git/warp/target/release/warp-oss
CWD=/home/effatha/git/warp
run() {
  local name="$1"; shift
  echo "== $name : $* =="
  timeout 180 "$W" --warpctrl acp probe --command "$*" --cwd "$CWD" \
    --prompt 'Say the word ready and nothing else.' \
    >"$name.jsonl" 2>"$name.err"
  echo "exit=$?"
  echo "-- stderr --"; cat "$name.err"
  echo
}
run codex  'npx -y @zed-industries/codex-acp@0.16.0'
run gemini 'npx -y @google/gemini-cli@0.58.0 --acp'
