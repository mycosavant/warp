#!/usr/bin/env bash
# Answer the single parked request. $1 = approve|deny
# Reads the id and digest off the entry rather than constructing them, and
# reads back afterwards -- both rules this fork learned by breaking them.
set -u
cd /home/effatha/git/warp
W=./target/release/warp-oss
A=$($W --warpctrl agent approvals --output-format json)
n=$(echo "$A" | grep -c '"approval_id"')
if [ "$n" -eq 0 ]; then echo "nothing parked"; exit 1; fi
ID=$(echo "$A" | python3 -c "import json,sys;print(json.load(sys.stdin)['approvals'][0]['approval_id'])")
DG=$(echo "$A" | python3 -c "import json,sys;print(json.load(sys.stdin)['approvals'][0]['digest'])")
SU=$(echo "$A" | python3 -c "import json,sys;print(json.load(sys.stdin)['approvals'][0]['summary'])")
echo ">> $1: $SU  ($ID)"
$W --warpctrl agent "$1" "$ID" --digest "$DG" --output-format json | grep -E '"decision"|"keystroke"'
left=$($W --warpctrl agent approvals --output-format json | grep -c '"approval_id"')
echo ">> read-back: $left still parked"
