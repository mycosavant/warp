#!/usr/bin/env python3
"""A loopback OTLP/HTTP trace receiver, just complete enough to see what arrives.

Warp exports Protocol::HttpBinary to <endpoint>/v1/traces. This decodes the
OTLP protobuf by hand against the known field numbers rather than pulling in
opentelemetry-proto, so it installs nothing and talks to nobody.
"""
import gzip
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

OUT = sys.argv[1] if len(sys.argv) > 1 else "spans.jsonl"


def read_varint(buf, i):
    result = 0
    shift = 0
    while True:
        b = buf[i]
        i += 1
        result |= (b & 0x7F) << shift
        if not b & 0x80:
            return result, i
        shift += 7


def fields(buf):
    """Yield (field_number, wire_type, value) for one protobuf message."""
    i = 0
    n = len(buf)
    while i < n:
        key, i = read_varint(buf, i)
        fnum, wtype = key >> 3, key & 7
        if wtype == 0:
            val, i = read_varint(buf, i)
        elif wtype == 1:
            val, i = buf[i:i + 8], i + 8
        elif wtype == 2:
            ln, i = read_varint(buf, i)
            val, i = buf[i:i + ln], i + ln
        elif wtype == 5:
            val, i = buf[i:i + 4], i + 4
        else:
            raise ValueError(f"unsupported wire type {wtype}")
        yield fnum, wtype, val


def get(buf, want):
    return [v for f, _, v in fields(buf) if f == want]


def anyvalue(buf):
    for f, wt, v in fields(buf):
        if f == 1:
            return v.decode("utf-8", "replace")
        if f == 2:
            return bool(v)
        if f == 3:
            # int_value is a signed varint on the wire.
            return v - (1 << 64) if v >= (1 << 63) else v
        if f == 4:
            import struct
            return struct.unpack("<d", v)[0]
        if f == 5:
            return [anyvalue(x) for x in get(v, 1)]
        if f == 6:
            return keyvalues(v)
        if f == 7:
            return v.hex()
    return None


def keyvalues(buf):
    out = {}
    for kv in get(buf, 1):
        key = None
        val = None
        for f, _, v in fields(kv):
            if f == 1:
                key = v.decode("utf-8", "replace")
            elif f == 2:
                val = anyvalue(v)
        if key is not None:
            out[key] = val
    return out


def kv_list(buf, field):
    """attributes are `repeated KeyValue` directly, not wrapped."""
    out = {}
    for kv in get(buf, field):
        key = None
        val = None
        for f, _, v in fields(kv):
            if f == 1:
                key = v.decode("utf-8", "replace")
            elif f == 2:
                val = anyvalue(v)
        if key is not None:
            out[key] = val
    return out


def u64le(b):
    return int.from_bytes(b, "little") if b else 0


def decode_span(buf):
    span = {"attributes": {}, "events": []}
    for f, wt, v in fields(buf):
        if f == 1:
            span["trace_id"] = v.hex()
        elif f == 2:
            span["span_id"] = v.hex()
        elif f == 4:
            span["parent_span_id"] = v.hex()
        elif f == 5:
            span["name"] = v.decode("utf-8", "replace")
        elif f == 7:
            span["start_ns"] = u64le(v)
        elif f == 8:
            span["end_ns"] = u64le(v)
        elif f == 9:
            kv = {}
            key = val = None
            for ff, _, vv in fields(v):
                if ff == 1:
                    key = vv.decode("utf-8", "replace")
                elif ff == 2:
                    val = anyvalue(vv)
            if key is not None:
                span["attributes"][key] = val
        elif f == 11:
            ev = {"attributes": {}}
            for ff, _, vv in fields(v):
                if ff == 1:
                    ev["time_ns"] = u64le(vv)
                elif ff == 2:
                    ev["name"] = vv.decode("utf-8", "replace")
                elif ff == 3:
                    key = val = None
                    for gg, _, gv in fields(vv):
                        if gg == 1:
                            key = gv.decode("utf-8", "replace")
                        elif gg == 2:
                            val = anyvalue(gv)
                    if key is not None:
                        ev["attributes"][key] = val
            span["events"].append(ev)
        elif f == 15:
            for ff, _, vv in fields(v):
                if ff == 2:
                    span["status_message"] = vv.decode("utf-8", "replace")
                elif ff == 3:
                    span["status_code"] = vv
    if "start_ns" in span and "end_ns" in span:
        span["duration_ms"] = round((span["end_ns"] - span["start_ns"]) / 1e6, 3)
    return span


def decode_request(body):
    spans = []
    for rs in get(body, 1):                      # ResourceSpans
        resource = {}
        for res in get(rs, 1):
            resource = kv_list(res, 1)
        for ss in get(rs, 2):                    # ScopeSpans
            scope = ""
            for sc in get(ss, 1):
                names = get(sc, 1)
                scope = names[0].decode("utf-8", "replace") if names else ""
            for sp in get(ss, 2):                # Span
                span = decode_span(sp)
                span["scope"] = scope
                span["resource"] = resource
                spans.append(span)
    return spans


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(length)
        if self.headers.get("Content-Encoding") == "gzip":
            body = gzip.decompress(body)
        try:
            spans = decode_request(body)
        except Exception as err:                 # noqa: BLE001
            spans = []
            print(f"decode failed: {err}", flush=True)
        with open(OUT, "a") as handle:
            for span in spans:
                handle.write(json.dumps(span) + "\n")
        print(f"{self.path}: {len(spans)} spans ({length} bytes)", flush=True)
        self.send_response(200)
        self.send_header("Content-Type", "application/x-protobuf")
        self.send_header("Content-Length", "0")
        self.end_headers()

    def log_message(self, *args):
        pass


if __name__ == "__main__":
    HTTPServer(("127.0.0.1", 4318), Handler).serve_forever()
