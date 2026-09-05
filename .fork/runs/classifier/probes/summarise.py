#!/usr/bin/env python3
"""Reduce `warpctrl acp probe --output-format ndjson` output to what a reader
needs: which agent and version answered, what was asked, whether it asked
permission. Usage: summarise.py probe-*.ndjson > probes.json"""
import json, sys
out = []
for path in sys.argv[1:]:
    rec = {"file": path.split("/")[-1], "agent": None, "mode_at_session_start": None, "mode_requested": None, "calls": []}
    for line in open(path):
        try:
            e = json.loads(line)
        except json.JSONDecodeError:
            continue
        k, p = e.get("kind"), e.get("payload") or {}
        if k == "initialized":
            rec["agent"] = p.get("agentInfo")
        elif k == "session":
            rec["mode_at_session_start"] = (p.get("modes") or {}).get("currentModeId")
        elif k == "consent_report":
            rec["mode_requested"] = [m.get("mode_id") for m in p.get("mode_requests_warp_sent", [])]
            rec["calls"] = [{"title": c["title"], "kind": c["kind"], "status": c["status"],
                             "permission_requests_received": c["permission_requests_received"]} for c in p.get("calls", [])]
    out.append(rec)
json.dump(out, sys.stdout, indent=1)
print()
