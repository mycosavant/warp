"""Record every descendant of a root pid, first-seen, until a stop file exists.

  proctree.py <root_pid> <out.tsv> <stopfile>

This is what attributes census sockets to the agent rather than to the driving
claude session, which shares the process name.
"""
import os, sys, time

root, out, stop = int(sys.argv[1]), sys.argv[2], sys.argv[3]
seen = {}


def ppid_comm(pid):
    try:
        with open(f"/proc/{pid}/stat") as f:
            s = f.read()
        comm = s[s.index("(") + 1 : s.rindex(")")]
        ppid = int(s[s.rindex(")") + 2 :].split()[1])
        with open(f"/proc/{pid}/cmdline", "rb") as f:
            cmd = f.read().replace(b"\0", b" ").decode(errors="replace")[:160]
        return ppid, comm, cmd
    except Exception:
        return None


with open(out, "w") as f:
    f.write("first_seen\tpid\tppid\tcomm\tcmdline\n")
    while not os.path.exists(stop):
        tree = {root}
        procs = {}
        for d in os.listdir("/proc"):
            if d.isdigit():
                r = ppid_comm(int(d))
                if r:
                    procs[int(d)] = r
        changed = True
        while changed:
            changed = False
            for pid, (ppid, _, _) in procs.items():
                if ppid in tree and pid not in tree:
                    tree.add(pid)
                    changed = True
        t = time.time()
        stamp = time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime(t)) + f".{int(t % 1 * 1000):03d}Z"
        for pid in sorted(tree):
            if pid not in seen and pid in procs:
                seen[pid] = 1
                ppid, comm, cmd = procs[pid]
                f.write(f"{stamp}\t{pid}\t{ppid}\t{comm}\t{cmd}\n")
                f.flush()
        time.sleep(0.2)
