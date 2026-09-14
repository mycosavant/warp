"""mitmdump addon: one redacted JSON line per request, nothing else kept.

Written for the vendor-calls census (.fork/HANDOFF-VENDORCALLS.md). The proxy
sees the subscription token in plaintext; this file is what stands between that
and the record. It writes:

  method, host, redacted path, query KEYS, status, request/response sizes,
  header NAMES, a 12-hex sha256 prefix for credential-bearing headers (and
  whether that prefix equals the prefix of the accessToken/refreshToken in
  .credentials.json), the JSON key structure of both bodies (no values), and
  which canaries appear anywhere in the request (url, header values, body).

No flow file is written (run mitmdump without -w) and no value from a body,
query or credential header is ever written.

Env: VC_OUT (jsonl), VC_CANARIES (comma-separated), VC_CREDS (credentials path,
optional).
"""
import hashlib
import json
import os
import re
import time

from mitmproxy import http

OUT = os.environ["VC_OUT"]
CANARIES = [c for c in os.environ.get("VC_CANARIES", "").split(",") if c]
CREDS = os.environ.get("VC_CREDS", "")

SECRET_HEADERS = {"authorization", "proxy-authorization", "x-api-key", "cookie", "set-cookie"}


def prefix(s: str) -> str:
    return hashlib.sha256(s.encode()).hexdigest()[:12]


def load_token_prefixes():
    out = {}
    try:
        with open(CREDS) as f:
            d = json.load(f)
        o = d.get("claudeAiOauth", {})
        for k in ("accessToken", "refreshToken"):
            if o.get(k):
                out[k] = prefix(o[k])
    except Exception:
        pass
    return out


TOKEN_PREFIXES = load_token_prefixes()

UUID_OR_NUMBER = re.compile(r"^([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}|[0-9]{6,})$", re.I)


def is_identifier(seg: str) -> bool:
    # A uuid, a long number, or a long token-shaped segment that mixes letters
    # and digits. A plain word such as `claude_code_penguin_mode` is kept.
    if UUID_OR_NUMBER.match(seg):
        return True
    return len(seg) >= 16 and re.search(r"\d", seg) is not None and re.search(r"[A-Za-z]", seg) is not None


def redact_path(path: str) -> str:
    p = path.split("?", 1)[0]
    return "/".join("<id>" if is_identifier(seg) else seg for seg in p.split("/"))


def structure(v, depth=0):
    if depth > 6:
        return "…"
    if isinstance(v, dict):
        return {k: structure(x, depth + 1) for k, x in v.items()}
    if isinstance(v, list):
        return {"list_len": len(v), "item": structure(v[0], depth + 1) if v else None}
    return type(v).__name__


def body_facts(msg):
    try:
        raw = msg.get_content(strict=False) or b""
    except Exception:
        raw = msg.raw_content or b""
    facts = {"bytes": len(raw), "content_type": msg.headers.get("content-type", "")}
    if raw:
        try:
            facts["json_keys"] = structure(json.loads(raw))
        except Exception:
            facts["json_keys"] = None
    return facts, raw


def cred_facts(headers):
    out = {}
    for name, value in headers.items(multi=True):
        lname = name.lower()
        if lname not in SECRET_HEADERS:
            continue
        v = value
        scheme = None
        if lname in ("authorization", "proxy-authorization") and " " in v:
            scheme, v = v.split(" ", 1)
        p = prefix(v)
        out.setdefault(lname, []).append(
            {
                "scheme": scheme,
                "sha256_12": p,
                "equals_accessToken": p == TOKEN_PREFIXES.get("accessToken"),
                "equals_refreshToken": p == TOKEN_PREFIXES.get("refreshToken"),
                "value_len": len(v),
            }
        )
    return out


def write(rec):
    with open(OUT, "a") as f:
        f.write(json.dumps(rec, sort_keys=True) + "\n")


BLOCK = [h for h in os.environ.get("VC_BLOCK", "").split(",") if h]


class Redact:
    def request(self, flow: http.HTTPFlow):
        r = flow.request
        if any(r.pretty_host == h or r.pretty_host.endswith("." + h) for h in BLOCK):
            # Recorded, then refused before it leaves: a package fetch during a
            # census is exactly what the run must not let happen.
            flow.response = http.Response.make(403, b"blocked by vendorcalls census")
            flow.metadata["blocked"] = True
        facts, raw = body_facts(r)
        haystack = r.pretty_url.encode() + b"\n" + b"\n".join(
            v.encode(errors="replace") for _, v in r.headers.items(multi=True)
        ) + b"\n" + raw
        flow.metadata["vc"] = {
            "t": time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime(r.timestamp_start)) + f".{int(r.timestamp_start % 1 * 1000):03d}Z",
            "method": r.method,
            "host": r.pretty_host,
            "port": r.port,
            "path": redact_path(r.path),
            "query_keys": sorted(set(r.query.keys())),
            "header_names": [n for n, _ in r.headers.items(multi=True)],
            "credentials": cred_facts(r.headers),
            "user_agent_family": r.headers.get("user-agent", "").split("/", 1)[0],
            "req": facts,
            "canaries_in_request": [c for c in CANARIES if c.encode() in haystack],
            "client_peer": list(flow.client_conn.peername or [])[:2],
        }

    def response(self, flow: http.HTTPFlow):
        rec = flow.metadata.get("vc")
        if rec is None:
            return
        facts, _ = body_facts(flow.response)
        rec["status"] = flow.response.status_code
        rec["blocked_by_census"] = bool(flow.metadata.get("blocked"))
        rec["resp"] = facts
        rec["resp_header_names"] = [n for n, _ in flow.response.headers.items(multi=True)]
        write(rec)

    def error(self, flow: http.HTTPFlow):
        rec = flow.metadata.get("vc")
        if rec is None:
            return
        rec["status"] = None
        rec["error"] = type(flow.error).__name__ if flow.error else "error"
        write(rec)

    def tls_failed_client(self, data):
        # The client refused our certificate: the host is all we learn, and it
        # is the signal that the process does not trust the mitm CA.
        sni = getattr(data.context.client, "sni", None)
        write({"t": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "event": "tls_failed_client", "sni": sni,
               "client_peer": list(data.context.client.peername or [])[:2]})

    def http_connect(self, flow: http.HTTPFlow):
        write({"t": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "event": "connect",
               "host": flow.request.host, "port": flow.request.port,
               "client_peer": list(flow.client_conn.peername or [])[:2]})


addons = [Redact()]
