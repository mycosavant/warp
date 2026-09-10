# The record that outlives the process is a record nobody looks at (T15)

**2026-09-10, WSL, binary `v0.fork.c25c8fbc2`.** The handoff's most-mentioned
open housekeeping item, carried as *"a discovery record and broker socket
outlive three clean `window close` shutdowns; unbisected"* since T11.5, with a
fifth sighting in T13.1 and a hypothesis about agents.

It reproduced without launching anything, because a specimen was already on
disk.

---

## The specimen

`~/.warp/local-control/` held a record and its broker socket, written
**2026-08-28 20:15** — thirteen days:

```
srw------- inst_17c9b4e94f45432a8317be79858cc692.broker.sock
-rw------- inst_17c9b4e94f45432a8317be79858cc692.json     pid 91104, port 38609
```

`ps -p 91104` is empty. So the record outlived its process, exactly as T11.5
reported.

## The mechanism, and it is not a broken pruner

`warpctrl instance list` from this shell answers `{"instances": []}` and leaves
both files alone. Run once with the environment the writer had, it removes both:

```
$ env -u XDG_RUNTIME_DIR warp-oss --warpctrl instance list
{"instances": []}
$ ls ~/.warp/local-control/
(empty)
```

`discovery_dir()` (`crates/local_control/src/discovery.rs:307`) resolves to
`$XDG_RUNTIME_DIR/warp/local-control` when that variable is set and to
`$HOME/.warp/local-control` when it is not. **One user on one machine has two
registries**, and which one a process uses is decided by the environment it was
launched from. The GUI that wrote this had no `XDG_RUNTIME_DIR`; every
`warpctrl` since has run from a shell that has one.

So the pruner was never broken. `is_pid_alive` does exactly what `CLAUDE.md`
says it does, on every scan, in the directory it was pointed at — and nothing
had ever pointed it at this one.

## The real cost is not the litter

Two stale files are nothing. The same split says a **running** Warp registered
in one directory is invisible to `warpctrl` in the other, and what a person sees
then is:

```
no local Warp control instances were discovered
```

for a Warp that is alive on screen. That is `window close` answering
`no_instance` with no way to tell "nothing is running" from "something is
running somewhere you did not look" — and `CLAUDE.md` already records that
symptom twice, once attributing it to `WARP_FORK_POLICY=0` correctly and once
leaving the mechanism open.

## What shipped

The message now names the directory it searched and the variable that decides
it. `select_instance` takes the path as a parameter rather than reading the
environment, so it stays pure and its tests run anywhere — the same reason
`session::filesystem::classify` takes its three facts. Three call sites pass
`discovery_dir()`.

Calibrated by reverting the message to the old one: `calibration.txt`,
`the_empty_answer_names_the_directory_it_looked_in` reddens and nothing else
does.

## What this does NOT establish

**That this is the cause of T11.5's three shutdowns.** Those were described as
leaving records "in the scratch directory", which is a run with
`XDG_CONFIG_HOME`/`XDG_STATE_HOME` pointed elsewhere and may be a different
resolution again. This run found *a* leak with a complete mechanism, on this
machine, today; it did not re-run T11.5.

**T13.1's hypothesis is neither supported nor refuted.** It guessed the split
was whether an instance had run agents. Nothing here bears on that, and two
launches in one shell would share an environment — so if that split is real it
needs another explanation.

**And `discovery_dir()`'s two-registry behaviour is left alone.** Making it
deterministic would strand any instance already registered under the other
resolution, and the directory is `0700` and holds broker sockets, so it is not
a change to make on the way past. Recorded here as the maintainer's.
