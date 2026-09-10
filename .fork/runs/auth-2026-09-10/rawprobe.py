#!/usr/bin/env python3
"""Speak JSON-RPC to an ACP agent over stdio and print the raw error object.

The `warpctrl acp probe` prints an ACP `Error`'s Display, which is its
`message` alone. What arm the panel should take may depend on the `code`, and
Display hides it. So this reads the wire.
"""
import json, subprocess, sys, threading, time

def probe(name, argv):
    p = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         stderr=subprocess.DEVNULL, text=True, bufsize=1)
    def send(i, method, params):
        p.stdin.write(json.dumps({"jsonrpc":"2.0","id":i,"method":method,"params":params})+"\n")
        p.stdin.flush()
    def read_until(i, timeout=120):
        end = time.time()+timeout
        while time.time() < end:
            line = p.stdout.readline()
            if not line: return None
            try: o = json.loads(line)
            except Exception: continue
            if o.get("id") == i: return o
        return None
    send(1, "initialize", {"protocolVersion":1,
                           "clientCapabilities":{"fs":{"readTextFile":False,"writeTextFile":False}}})
    init = read_until(1)
    send(2, "session/new", {"cwd":"/home/effatha/git/warp","mcpServers":[]})
    new = read_until(2)
    p.stdin.close()
    try: p.wait(timeout=10)
    except Exception: p.kill()
    out = {"agent": name,
           "authMethods": (init or {}).get("result",{}).get("authMethods"),
           "session_new": new}
    print(json.dumps(out, indent=2))
    return out

if __name__ == "__main__":
    probe("codex",  ["npx","-y","@zed-industries/codex-acp@0.16.0"])
    probe("gemini", ["npx","-y","@google/gemini-cli@0.58.0","--acp"])
