"""Minimal ACP client: N prompts in ONE session, for configuration E.

  multiturn.py "<command>" <cwd> <turns>

Prints one JSON line per message received plus TURN markers. Refuses every
permission request, like `acp probe` without --approve.
"""
import json
import shlex
import subprocess
import sys

cmd, cwd, turns = sys.argv[1], sys.argv[2], int(sys.argv[3])
p = subprocess.Popen(shlex.split(cmd), stdin=subprocess.PIPE, stdout=subprocess.PIPE, cwd=cwd, text=True, bufsize=1)
nid = 0


def emit(o):
    print(json.dumps(o), flush=True)


def call(method, params):
    global nid
    nid += 1
    my = nid
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": my, "method": method, "params": params}) + "\n")
    p.stdin.flush()
    while True:
        line = p.stdout.readline()
        if not line:
            raise SystemExit(f"agent closed during {method}")
        m = json.loads(line)
        if m.get("id") == my and "method" not in m:
            emit({"response_to": method, "msg": m})
            return m
        if "method" in m and "id" in m:  # agent -> client request
            if m["method"] == "session/request_permission":
                opts = m["params"].get("options", [])
                rej = next((o["optionId"] for o in opts if o.get("kind", "").startswith("reject")), None)
                res = {"outcome": {"outcome": "selected", "optionId": rej}} if rej else {"outcome": {"outcome": "cancelled"}}
                p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": m["id"], "result": res}) + "\n")
            else:
                p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": m["id"], "error": {"code": -32601, "message": "not supported"}}) + "\n")
            p.stdin.flush()
        emit({"received": m})


call("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
s = call("session/new", {"cwd": cwd, "mcpServers": []})
sid = s["result"]["sessionId"]
prompts = [
    "CANARY-PROMPT-5d19b2. In one sentence, what is 2+2?",
    "In one sentence, what is 3+3?",
    "In one sentence, what is 4+4?",
]
for i in range(turns):
    emit({"marker": f"TURN{i+1}_START"})
    call("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": prompts[i % len(prompts)]}]})
    emit({"marker": f"TURN{i+1}_END"})
p.stdin.close()
p.wait(timeout=20)
