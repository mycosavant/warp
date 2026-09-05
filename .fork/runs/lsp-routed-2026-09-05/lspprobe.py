"""Minimal stdio LSP client: initialize, didOpen, documentSymbol, definition, hover.

usage: lspprobe.py --root-uri URI --file-uri URI --content PATH [--cwd DIR] -- CMD [ARGS...]
Prints every response's URIs verbatim so the path question is answered from the wire.
"""
import argparse, json, os, subprocess, sys, time, threading, queue

p = argparse.ArgumentParser()
p.add_argument("--root-uri", required=True)
p.add_argument("--file-uri", required=True)
p.add_argument("--content", required=True, help="local path to read the file text from")
p.add_argument("--cwd")
p.add_argument("--line", type=int, default=11)
p.add_argument("--col", type=int, default=12)
p.add_argument("--timeout", type=float, default=90)
p.add_argument("cmd", nargs=argparse.REMAINDER)
a = p.parse_args()
cmd = a.cmd[1:] if a.cmd and a.cmd[0] == "--" else a.cmd

t0 = time.time()
def ts(): return f"{time.time()-t0:6.2f}s"
proc = subprocess.Popen(cmd, cwd=a.cwd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
print(f"[{ts()}] spawned pid={proc.pid} cmd={cmd} cwd={a.cwd}", flush=True)

q = queue.Queue()
def reader():
    f = proc.stdout
    while True:
        headers = {}
        line = f.readline()
        if not line: q.put(None); return
        while line.strip():
            k, _, v = line.decode().partition(":")
            headers[k.strip().lower()] = v.strip()
            line = f.readline()
        n = int(headers.get("content-length", "0"))
        body = f.read(n)
        q.put(json.loads(body))
def errreader():
    for line in proc.stderr:
        print(f"[{ts()}] stderr: {line.decode(errors='replace').rstrip()}", flush=True)
threading.Thread(target=reader, daemon=True).start()
threading.Thread(target=errreader, daemon=True).start()

seq = 0
def send(method, params, is_request=True):
    global seq
    msg = {"jsonrpc": "2.0", "method": method, "params": params}
    if is_request:
        seq += 1; msg["id"] = seq
    data = json.dumps(msg).encode()
    proc.stdin.write(f"Content-Length: {len(data)}\r\n\r\n".encode() + data); proc.stdin.flush()
    return seq if is_request else None

def wait_for(req_id, timeout):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try: m = q.get(timeout=0.2)
        except queue.Empty: continue
        if m is None: print(f"[{ts()}] server closed stdout"); return None
        if m.get("id") == req_id and "method" not in m: return m
        if m.get("method") == "$/progress":
            v = m["params"]["value"]
            if v.get("kind") in ("begin", "end"):
                print(f"[{ts()}] progress {m['params']['token']} {v['kind']} {v.get('title','')}", flush=True)
        elif "id" in m and "method" in m:  # server request: answer null
            resp = {"jsonrpc": "2.0", "id": m["id"], "result": None}
            data = json.dumps(resp).encode()
            proc.stdin.write(f"Content-Length: {len(data)}\r\n\r\n".encode() + data); proc.stdin.flush()
            print(f"[{ts()}] server request {m['method']} -> null", flush=True)
        elif m.get("method") not in ("window/logMessage", "textDocument/publishDiagnostics"):
            print(f"[{ts()}] notif {m.get('method')}", flush=True)
    print(f"[{ts()}] TIMEOUT waiting for id={req_id}"); return None

rid = send("initialize", {
    "processId": os.getpid(),
    "rootUri": a.root_uri,
    "workspaceFolders": [{"uri": a.root_uri, "name": "probe"}],
    "capabilities": {"window": {"workDoneProgress": True}, "textDocument": {"definition": {"linkSupport": True}}},
    "clientInfo": {"name": "lspprobe"},
})
r = wait_for(rid, a.timeout)
print(f"[{ts()}] initialize -> serverInfo={r and r.get('result',{}).get('serverInfo')}", flush=True)
send("initialized", {}, is_request=False)
text = open(a.content, encoding="utf-8").read()
send("textDocument/didOpen", {"textDocument": {"uri": a.file_uri, "languageId": "rust", "version": 1, "text": text}}, is_request=False)
print(f"[{ts()}] didOpen {a.file_uri} ({len(text)} bytes read from {a.content})", flush=True)

rid = send("textDocument/documentSymbol", {"textDocument": {"uri": a.file_uri}})
r = wait_for(rid, a.timeout)
print(f"[{ts()}] documentSymbol -> {json.dumps(r and r.get('result'))[:600]}", flush=True)

# Poll the definition until the workspace has loaded enough to answer it.
deadline = time.time() + a.timeout
res = None
while time.time() < deadline:
    rid = send("textDocument/definition", {"textDocument": {"uri": a.file_uri}, "position": {"line": a.line, "character": a.col}})
    r = wait_for(rid, a.timeout)
    res = r and r.get("result")
    if res: break
    time.sleep(1)
print(f"[{ts()}] definition({a.line},{a.col}) -> {json.dumps(res)[:600]}", flush=True)
rid = send("textDocument/hover", {"textDocument": {"uri": a.file_uri}, "position": {"line": a.line, "character": a.col}})
r = wait_for(rid, a.timeout)
print(f"[{ts()}] hover -> {json.dumps(r and r.get('result'))[:300]}", flush=True)
rid = send("shutdown", None); wait_for(rid, 10); send("exit", None, is_request=False)
try: proc.wait(10)
except Exception: proc.kill()
print(f"[{ts()}] exit code {proc.returncode}", flush=True)
