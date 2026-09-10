#!/usr/bin/env python3
"""Control case: an agent that IS logged in, refusing session/new for a
non-auth reason. If it advertises authMethods anyway, then "advertised
authMethods + refused" is not a safe test for an auth refusal."""
import json, subprocess, time, sys

def probe(name, argv, cwd):
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
    send(2, "session/new", {"cwd":cwd,"mcpServers":[]})
    new = read_until(2)
    p.stdin.close()
    try: p.wait(timeout=10)
    except Exception: p.kill()
    print(json.dumps({"agent":name,"cwd":cwd,
                      "authMethods":(init or {}).get("result",{}).get("authMethods"),
                      "agentInfo":(init or {}).get("result",{}).get("agentInfo"),
                      "session_new":new}, indent=2))

probe("claude-agent-acp (missing cwd)",
      ["npx","-y","@agentclientprotocol/claude-agent-acp@0.73.0"],
      "/home/effatha/definitely-not-a-real-directory-xyz")
