#!/usr/bin/env bash
# Prove the addon leaks nothing before a real token goes through it.
# Plants fake secrets in every place a request can carry one, then greps every
# file the rig produces for each of them. Exit 0 only if all are absent and
# the fake token's prefix is recognised as the "accessToken".
set -u
here=$(cd "$(dirname "$0")" && pwd)
t=$here/verify; rm -rf "$t"; mkdir -p "$t/mitm"
FAKE_ACCESS=sk-ant-oat01-FAKEACCESS-$(openssl rand -hex 16)
FAKE_REFRESH=sk-ant-ort01-FAKEREFRESH-$(openssl rand -hex 16)
SECRET_Q=QSECRET$(openssl rand -hex 6)
SECRET_V=VSECRET$(openssl rand -hex 6)
SECRET_C=CSECRET$(openssl rand -hex 6)
SECRET_K=KSECRET$(openssl rand -hex 6)
UUID=$(cat /proc/sys/kernel/random/uuid)
printf '{"claudeAiOauth":{"accessToken":"%s","refreshToken":"%s"}}' "$FAKE_ACCESS" "$FAKE_REFRESH" > "$t/creds.json"

VC_OUT=$t/out.jsonl VC_CANARIES=CANARYVERIFY VC_CREDS=$t/creds.json \
  mitmdump --listen-host 127.0.0.1 --listen-port 18081 --set confdir="$t/mitm" \
  --set flow_detail=0 -s "$here/redact_addon.py" > "$t/mitm.log" 2>&1 &
mp=$!
for i in $(seq 1 30); do [ -f "$t/mitm/mitmproxy-ca-cert.pem" ] && break; sleep 0.3; done
sleep 1
curl -s -o /dev/null -w '%{http_code}\n' --cacert "$t/mitm/mitmproxy-ca-cert.pem" -x http://127.0.0.1:18081 \
  -H "Authorization: Bearer $FAKE_ACCESS" -H "x-api-key: $SECRET_K" -H "Cookie: sid=$SECRET_C" \
  -H 'Content-Type: application/json' \
  --data "{\"prompt\":\"CANARYVERIFY $SECRET_V\",\"nested\":{\"refresh\":\"$FAKE_REFRESH\"}}" \
  "https://example.com/v1/org/$UUID/thing?code=$SECRET_Q&x=1"
sleep 1
kill $mp; wait $mp 2>/dev/null
rm -f "$t/creds.json"

fail=0
for s in "$FAKE_ACCESS" "$FAKE_REFRESH" "$SECRET_Q" "$SECRET_V" "$SECRET_C" "$SECRET_K" "$UUID"; do
  if grep -rqF -- "$s" "$t"; then echo "LEAK: ${s:0:14}… found in: $(grep -rlF -- "$s" "$t")"; fail=1; fi
done
python3 - "$t/out.jsonl" <<'EOF' || fail=1
import json,sys
recs=[json.loads(l) for l in open(sys.argv[1])]
reqs=[r for r in recs if r.get("method")]
assert len(reqs)==1, recs
r=reqs[0]
a=r["credentials"]["authorization"][0]
assert a["equals_accessToken"] and not a["equals_refreshToken"], a
assert r["canaries_in_request"]==["CANARYVERIFY"], r
assert r["query_keys"]==["code","x"], r
assert "<id>" in r["path"], r
assert any(x.get("event")=="connect" for x in recs), recs
print("addon record:", json.dumps(r)[:600])
EOF
echo "files produced: $(find "$t" -type f | sed "s|$t/||" | tr '\n' ' ')"
[ $fail = 0 ] && echo VERIFY PASS || echo VERIFY FAIL
exit $fail
