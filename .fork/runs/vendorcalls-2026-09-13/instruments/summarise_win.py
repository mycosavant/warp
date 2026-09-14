"""Summarise a configuration G run: Windows sockets of the probe tree, proxy requests, answer."""
import json, sys

d = sys.argv[1]


def read_text(path):
    raw = open(path, "rb").read()
    for enc in ("utf-8-sig", "utf-16"):
        try:
            t = raw.decode(enc)
            if "\x00" not in t:
                return t
        except UnicodeDecodeError:
            pass
    return raw.decode("utf-8", "replace")


print("timeline:")
for l in read_text(f"{d}/timeline.txt").splitlines():
    print("  " + l)
print("windows sockets, non-listen, distinct per (pid,name,remote):")
seen = {}
for l in read_text(f"{d}/win-sockets.tsv").splitlines():
    if l.startswith("#") or not l.strip():
        continue
    f = l.split("\t")
    if len(f) < 7:
        continue
    ts, pid, name, proto, local, remote, state = f[:7]
    if state in ("Listen", "Bound") or remote in ("0.0.0.0:0", "*:*", "[::]:0"):
        continue
    seen.setdefault((pid, name, remote), (ts, state, local))
for (pid, name, remote), (ts, state, local) in sorted(seen.items(), key=lambda x: x[1][0]):
    loop = remote.startswith("127.") or remote.startswith("[::1]")
    print(f"  {ts} pid={pid} {name:<14} {state:<11} {local} -> {remote} {'loopback' if loop else 'NON-LOOPBACK'}")
print("requests seen by proxy:")
for l in open(f"{d}/requests.jsonl"):
    r = json.loads(l)
    if r.get("event"):
        continue
    creds = {h: [("SAME-AS-accessToken" if c["equals_accessToken"] else "other", c["value_len"]) for c in v] for h, v in r["credentials"].items()}
    print(f"  {r['t']} {r['method']} {r['host']}{r['path']} q={r['query_keys']} -> {r.get('status')} ua={r['user_agent_family']} creds={creds} canaries={r['canaries_in_request']}")
answer, first = [], None
for l in read_text(f"{d}/probe.ndjson").splitlines():
    if not l.strip():
        continue
    try:
        o = json.loads(l)
    except json.JSONDecodeError:
        continue
    if "agent_message_chunk" in o["line"]:
        first = first or o["ts"]
        try:
            answer.append(json.loads(o["line"])["payload"]["content"]["text"])
        except Exception:
            pass
print("first chunk:", first, "answer:", "".join(answer)[:200])
