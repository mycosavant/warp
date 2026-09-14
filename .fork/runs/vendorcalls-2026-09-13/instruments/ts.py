import sys, time, json
for line in sys.stdin:
    t = time.time()
    stamp = time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime(t)) + f".{int(t % 1 * 1000):03d}Z"
    sys.stdout.write(json.dumps({"ts": stamp, "line": line.rstrip("\n")}) + "\n")
    sys.stdout.flush()
