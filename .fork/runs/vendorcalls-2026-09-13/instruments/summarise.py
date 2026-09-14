"""Summarise one run directory: tree pids, their distinct remotes, controls, requests, TTFT."""
import json, re, sys
from datetime import datetime

d = sys.argv[1]
def ts(s): return datetime.strptime(s, "%Y-%m-%dT%H:%M:%S.%fZ")

timeline = {}
for l in open(f"{d}/timeline.txt"):
    k, t, *_ = l.split(" ")
    timeline[k] = t.strip()

tree = {}
for l in list(open(f"{d}/procs.tsv"))[1:]:
    f = l.rstrip("\n").split("\t")
    if f[3] in ("ss", "grep", "awk", "sleep", "date", "bash", "python3", "mitmdump", "curl", "reg.exe"):
        continue
    tree[int(f[1])] = (f[0], f[3], f[4])
print("process tree (agent side):")
for pid, (t, comm, cmd) in tree.items():
    print(f"  {t} pid={pid} {comm}  {cmd[:110]}")

seen = {}
for l in open(f"{d}/sockets.tsv.raw"):
    f = l.rstrip("\n").split("\t")
    m = re.search(r"pid=(\d+)", l)
    if not m: continue
    pid = int(m.group(1))
    cols = f[1].split() if len(f) < 5 else None
    parts = l.split()
    # ts, proto, state, recvq, sendq, local, remote, users..., sample
    state, local, remote = parts[2], parts[5], parts[6]
    if pid not in tree: continue
    if remote.endswith(":*") or remote == "0.0.0.0:*": continue
    key = (pid, state if state != "ESTAB" else "ESTAB", remote)
    seen.setdefault((pid, remote), (parts[0], state, local))
print("distinct remotes per tree pid (first seen):")
for (pid, remote), (t, state, local) in sorted(seen.items(), key=lambda x: x[1][0]):
    loop = remote.startswith("127.") or remote.startswith("[::1]") or remote.startswith("[::ffff:127.")
    print(f"  {t} pid={pid} {tree[pid][1]:<10} {state:<9} {local} -> {remote} {'loopback' if loop else 'NON-LOOPBACK'}")

ctl = [p for p, v in tree.items() if "example.com" in v[2]]
print("control 2 (node, no proxy) pids:", ctl, "fired:", any(p in ctl for p, _ in seen))

print("requests seen by proxy:")
for l in open(f"{d}/requests.jsonl"):
    r = json.loads(l)
    if r.get("event"):
        print(f"  {r['t']} {r['event']} {r.get('host') or r.get('sni')}:{r.get('port','')}")
        continue
    creds = {h: [(c['scheme'], 'SAME-AS-accessToken' if c['equals_accessToken'] else ('SAME-AS-refreshToken' if c['equals_refreshToken'] else 'other'), c['value_len']) for c in v] for h, v in r['credentials'].items()}
    print(f"  {r['t']} {r['method']} {r['host']}{r['path']} q={r['query_keys']} -> {r.get('status')} req={r['req']['bytes']}B resp={r.get('resp',{}).get('bytes')}B ua={r['user_agent_family']} creds={creds} canaries={r['canaries_in_request']}")

first = None; answer = []; stop = None
for l in open(f"{d}/probe.ndjson"):
    o = json.loads(l); line = o["line"]
    if "agent_message_chunk" in line:
        first = first or o["ts"]
        try:
            j = json.loads(line)
            p = j.get("payload") or j.get("received", {}).get("params", {}).get("update", {})
            answer.append((p.get("content") or {}).get("text", ""))
        except Exception:
            pass
    if '"stopReason"' in line: stop = o["ts"]
start = ts(timeline["DRIVER_START"])
print("first agent_message_chunk:", first, f"(+{(ts(first)-start).total_seconds():.2f}s)" if first else "")
print("answer:", "".join(answer)[:300])
print("driver:", timeline.get("DRIVER_START"), "->", timeline.get("DRIVER_END"))
