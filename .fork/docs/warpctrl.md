# `warpctrl`: instances, discovery, and stopping one

*Current as of 2026-09-11. The **index** is the rule list in `CLAUDE.md`'s
"Working rules" section. This page is the **account** — every wrong version
of "why won't it close" and what corrected it.*

*Split out of `CLAUDE.md` on 2026-09-12 with no sentence changed. This was
the largest coherent cluster left in "Working rules" after the build/memory
and console/SSH splits — instance lifecycle, the two discovery registries,
and what actually happens when `window close` is asked for.*

---

**Build with `--features gui,warp_control_cli`.** `warp_control_cli` is *not* in
`app/Cargo.toml`'s default list, and without it there is no `--warpctrl` — the
control plane this fork exists to open is simply absent from the binary.



**Stop a running Warp with `warpctrl window close`** (`CloseMainWindow` on
Windows), because ordinary shutdown cleans up the crash-recovery sibling and a
killed one leaves it holding the ports.



**The "stale discovery record" half of that sentence is wrong — and the first
attempt to correct it was wrong in a more instructive way, because it measured a
different quantity, in the commit that was correcting someone else for exactly
that.** Both halves measured 2026-09-03:

| what was killed | `instance list` afterwards |
|---|---|
| `taskkill /F /IM warp-oss.exe` — **every** process of that name, sibling included | **empty** |
| `taskkill /F /PID <the registered pid>` — *the* process, which is what the sentence describes | **one record**, a different pid |

The first form is what T20.3's pre-launch check was validated against, and on its
own it says records are pruned. That much is true: `discovery.rs` prunes dead-PID
records on every scan (`is_pid_alive`, two call sites), so the pid filter that
check nearly shipped really was dead code.

**The second form is the case the sentence was about, and what survives is not a
stale record — it is a live Warp.** The crash-recovery sibling is parked in
`WaitForSingleObject` on the parent handle; the parent dies, it continues into
normal startup, becomes a full instance, **publishes its own discovery record**,
and spawns a recovery sibling of its own. Measured: one pid killed, two processes
afterwards, one fresh record. So the original sentence's *observation* was right
and its *mechanism* was wrong — a record remains, and it is neither stale nor
prunable, because the process it names is genuinely running.

That is a better argument for `window close` than either version, and it is why
T20.3's check refuses on a **live** pid rather than filtering for a dead one.

**What accumulates instances is the same fact from the other side**, and this
file already records it two paragraphs down without connecting them: a CLI agent
in a pane blocks `window close`, the close is *refused*, and the instance stays
alive. Three piled up in one session that way. So `ambiguous_instance` always
comes from live Warps nobody could stop — whether they refused to close or were
resurrected by killing their parent — and never from records nobody cleaned.

**…and the last sentence needs one exception, because there is a second
registry.** Bisected 2026-09-10 (T15, `.fork/runs/discovery-2026-09-10/`).
`discovery_dir()` answers `$XDG_RUNTIME_DIR/warp/local-control` when that
variable is set and `$HOME/.warp/local-control` when it is not, so **one user on
one machine has two of them**, and the environment a process was launched from
decides which it writes to and reads from. A record and broker socket from
2026-08-28 were still in the second, naming a dead pid, thirteen days later; one
`env -u XDG_RUNTIME_DIR warpctrl instance list` removed both. The pruner does
exactly what this file says it does, on every scan, in the directory it was
given — nothing had ever given it that one.

**The litter is not what this costs you.** A *running* Warp registered in one
registry is invisible to `warpctrl` in the other, and answers `no_instance` — the
same word a genuinely absent Warp gets. That message now names the directory it
searched and the variable that decides it, so the two are separable at a glance
rather than by experiment. When `instance list` says nothing is running and
something is, check `$XDG_RUNTIME_DIR` first.

**…and until 2026-09-05 a closed instance's listeners outlived it on Windows,
so the next launch on the console's port failed with "only one usage of each
socket address".** Measured three times in one night: `netstat` showed the
wide listener `LISTENING` under the dead pid, held by `wsl.exe` children Warp
had spawned for its git chip and left orphaned. The `mio` this build locks
creates inheritable sockets, and every child Warp spawns with stdio inherits
them. Both control listeners are marked non-inheritable now
(`keep_from_children` in `app/src/local_control/mod.rs`); upstream's `9282`
still fails to bind the same way, which is the cause behind the egress run's
"honest note". The orphaned relays themselves are still there after a close
and are not the fork's fix yet. If a port is refused, `netstat -ano | findstr
<port>` names the dead pid, and the holders are the `wsl` processes created
at that instance's start.

**…and `ok: true` from `window close` never meant the window closed.** Read
2026-08-30: the handler sends the close with `TerminationMode::Cancellable` —
*"the termination can be interrupted"* — and returns the instant it has asked,
without observing the outcome. So `ok` meant *the request was dispatched*, and
nothing in the payload said so. That is the mistake `approvals.rs` explicitly
refuses one action over, reporting the keystroke it sent rather than
`approved: true` because *"a result claiming `approved: true` would assert an
effect this process cannot observe"*. The result now carries
`close: "requested"`, `cancellable: true` and a `verify` sentence naming
`instance list` as the check. **One mechanism for a refused close, measured
2026-09-06**: Warp's own *Quit Warp? You have 1 process running* dialog. With
a freshly split pane whose shell was still starting, `window close` returned
`ok: true`, the dialog stood over the window
(`.fork/runs/tls-2026-09-06/after-close-refused.png`), and `instance list`
kept its record until a second `window close`. That is one cause, not the
cause: `CloseSessionConfirmationDialog` covers pane and tab closes and
`OpenDialogSource` has no window arm, so the earlier version of this
sentence, which declined to name a mechanism, was right to; this one names
the one that has been seen.



**…and a CLI agent running in a pane blocks it too, with none of the wedge's
tells.** Measured 2026-08-30: with `claude` alive in a pane, `window close`
answered `ok: true` and the process stayed up — while `agent list` reported **no
conversations** and `agent approvals` reported **nothing waiting**, because a CLI
agent in a pane is neither. So T14.10's instruments, which exist precisely to
answer "why will it not close", are silent on this case. Three instances
accumulated this way in one session, and stale instances make every later
`warpctrl` call answer `ambiguous_instance` — which a check that greps only for
`"ok"` sails straight past. **End the agent in the pane first**, then close.



**…and cancel a wedged ACP turn first, or it will not close at all.** Measured
T14.10 against an agent built to stall: with a turn in flight that has stopped
answering, `window close` returns `ok: true` and Warp stays up — reproduced on
two separate instances, once after waiting 43 seconds. `agent cancel <id>` and
then `window close` exits in about five. So a wedge is not only a time cost; it
takes away the sanctioned way to stop, which is the one thing `kill` was already
ruled out for. `agent list` now reports `quiet_for_seconds` and `last_activity`
for a turn Warp is driving, which is how you tell there is one to cancel.



**A GUI Warp binds two loopback ports, and only one of them is this fork's.**
Measured 2026-08-24 with `ss -ltnp` against a running instance:

| port | owner |
|---|---|
| `127.0.0.1:9282` | **upstream's** `crates/http_server` — `PORT_BASE` 9277 plus the channel offset, and Oss is +5 |
| ephemeral (e.g. `:34969`) | `warpctrl`, which binds port **0** and publishes whatever it gets in the discovery record |

**This corrects a claim that stood here for two days: `warpctrl` never used
9282.** It also settles the open question below. Upstream's server is started by
`LaunchMode::should_start_local_http_server`, which is `!self.is_headless()` —
no feature flag, no channel gate, nothing fork policy touches. It serves the
routers listed at `app/src/lib.rs:2611` and answers **unauthenticated**: its CORS
layer restricts browsers to `warp.dev` origins and stops nothing else. Do not
put anything sensitive behind it; `warpctrl`'s server is the one with `auth.rs`,
the credential broker and the peer-UID check.

**…and `WARP_FORK_POLICY=0` is still a trap, because the same file tells you to
use that flag to A/B a regression.** Observed 2026-08-22: a policy-off instance
ran with a visible window and held a port, while the discovery directory stayed
**empty** — so `warpctrl window close` answered `no_instance` and there was no
sanctioned way to stop it. The port was upstream's 9282, ungated by policy; the
empty directory was `warpctrl` correctly staying off. Plan the shutdown before a
policy-off run.

**And the explanation this file gave for that stood wrong until 2026-08-31.** It
said *"the discovery record carries the credential, so no record means no client
can authenticate"*. It does not: `InstanceRecord` publishes routing metadata, the
loopback endpoint and **the filename of the credential-broker socket**, and
`discovery.rs`'s own module docs say in as many words that *"discovery records
never contain bearer tokens or reusable credentials"* — the secret is minted at
the broker, per action, and kept process-local. The observed behaviour was right
and the mechanism under it was invented: with no record a client cannot find
*where to ask*, which is a weaker and more interesting fact than not being able
to authenticate. Caught by an agent in Warp's own panel, from a prompt that
asserted the wrong version as its premise — it corrected the question instead of
answering it, which is the argument for stating your premise where the agent can
see it.

**…and often you do not need one.** `--warpctrl` runs `init_feature_flags`
before it dispatches, so `WARP_FORK_POLICY=0 warp-oss --warpctrl instance list`
resolves the whole flag set in a process that opens no window and binds no
port. That is enough to A/B any *flag*, which is most of what policy-off gets
used for. Save the GUI run for A/B-ing behaviour. (Put any probe **after**
`mark_initialized()` — `FeatureFlag::is_enabled` panics before it.)



**…and `WARP_FORK_POLICY=0` is still a trap, because the same file tells you to
use that flag to A/B a regression.** Observed 2026-08-22: a policy-off instance
ran with a visible window and held a port, while the discovery directory stayed
**empty** — so `warpctrl window close` answered `no_instance` and there was no
sanctioned way to stop it. The port was upstream's 9282, ungated by policy; the
empty directory was `warpctrl` correctly staying off. Plan the shutdown before a
policy-off run.



**And the explanation this file gave for that stood wrong until 2026-08-31.** It
said *"the discovery record carries the credential, so no record means no client
can authenticate"*. It does not: `InstanceRecord` publishes routing metadata, the
loopback endpoint and **the filename of the credential-broker socket**, and
`discovery.rs`'s own module docs say in as many words that *"discovery records
never contain bearer tokens or reusable credentials"* — the secret is minted at
the broker, per action, and kept process-local. The observed behaviour was right
and the mechanism under it was invented: with no record a client cannot find
*where to ask*, which is a weaker and more interesting fact than not being able
to authenticate. Caught by an agent in Warp's own panel, from a prompt that
asserted the wrong version as its premise — it corrected the question instead of
answering it, which is the argument for stating your premise where the agent can
see it.



**…and often you do not need one.** `--warpctrl` runs `init_feature_flags`
before it dispatches, so `WARP_FORK_POLICY=0 warp-oss --warpctrl instance list`
resolves the whole flag set in a process that opens no window and binds no
port. That is enough to A/B any *flag*, which is most of what policy-off gets
used for. Save the GUI run for A/B-ing behaviour. (Put any probe **after**
`mark_initialized()` — `FeatureFlag::is_enabled` panics before it.)



## Read back a mutation before trusting the next reading

**Read back a state-changing step before measuring what follows it.** The
merge-base trap has a second form and it bit on 2026-08-29: a driver script
answered an approval with the digest passed positionally instead of as
`--digest`, so nothing was delivered, the turn parked, and `quiet_for_seconds`
honestly reported 171 seconds of silence — which reads exactly like the wedge
that field exists to detect. As with `git diff A...B` against an assumed base,
the measurement was correct and the input to it was not. **After any mutation,
confirm the mutation before believing the next reading**: after `agent approve`,
check the request has left `agent approvals`. One extra call, and it turns a
three-minute misdiagnosis into a two-second one. Two dearer checks are worth it
before a *surprising* finding goes into a doc: calibrate a new instrument against
a known answer first (`wedged-agent.py` is the pattern — fire on the known
present, stay silent on the known absent), and confirm on a second instrument
when one exists, which is the general form of *take the screenshot before
believing `warpctrl agent read`*.

## No focused window is the ordinary case, not an error

**`ctx.windows().active_window()` is `None` whenever no Warp window holds OS
focus**, and for a control plane invoked from a shell that is most of the time.
On the winit backend it is literally
`windows.find(|w| w.has_focus() && w.is_visible())`
(`crates/warpui/src/windowing/winit/window.rs:193`), and the test platform
returns `None` unconditionally
(`crates/warpui_core/src/platform/test/delegate.rs:100`). Whoever runs
`warpctrl` is looking at a terminal, or at another monitor. Found 2026-09-12
when the maintainer, watching a run, clicked the desktop on a second monitor
and asked why that should matter to a CLI.

Two separate bugs came out of that in one day, and they are easy to confuse
because the symptom is a targeting error either way:

| fixed in | error | cause |
|---|---|---|
| `03bf98fd0` | `ambiguous_target` | `session_inspect` did not default its own target, leaving pane selection unscoped across every tab. Nothing to do with restore — `is_active` is one pane per `PaneGroup`, so N open tabs give N actives however they were opened. |
| `619c345a6`, `fc2a9bb46` | `missing_target` | the resolver demanded an OS-reported active window, and there is none whenever Warp is not frontmost. |

`active_or_single_window_id` (`app/src/local_control/resolver.rs`) answers the
second: the reported window when there is one, the single open window when
there is not, and `ambiguous_target` only on a genuine choice among 2+. Both
the read path (`metadata.rs::select_window_entries`) and the write path
(`metadata_config.rs::select_window_ids`) resolve through it — but only since
`fc2a9bb46`. `619c345a6` unified the read half alone, so for a day `tab.rename`
answered `missing_target` in the same instance, at the same moment, that
`session inspect` resolved fine. The two files each have their own private
`select_tab_entries`, which is what hid it: a grep on the function name
suggests one chain and there are two.

**How many actions the read half covers.** `619c345a6`'s body says *"all seven
read/inspect actions"* and names none; a review counted eight plus `agent.rs`.
Traced 2026-09-12 from `bridge.rs`'s dispatch, not run: **eight** actions route
through `select_window_entries` — `window`, `tab`, `pane` and `session`, each as
`list` and `inspect`. That the call is *routed* does not mean the fallback is
*reached*. On an argument-less call only the four `inspect`s reach
`active_or_single_window_id`, because each defaults its own target to `Active`;
the four `list`s reach it only when given an active or index selector.
`agent.rs::surface_locations` (nine callers across `agent.rs`, `approvals.rs`,
`trace.rs`) passes `TargetSelector::default()`, so it takes `None => Ok(entries)`
and is unaffected either way. No mutation run establishes the four-versus-eight
split; the `session_inspect` integration test covers one of the four.

**Driven live on Windows 2026-09-12**, pre-fix and post-fix debug builds under a
scratch profile, with another application frontmost: post-fix `tab reset-name`
answers `ok`, pre-fix answers `missing_target`, and `app active` names no window
in both (`.fork/runs/focus-live-2026-09-12/`). A launch usually takes
foreground, so check `app active` before believing a pre-fix pass.

**`app.active` is the deliberate exception**, and says so in place. Its question
is literally what holds focus, so an all-`None` chain is the true answer there,
and a single-window fallback would be a lie the caller cannot detect. A caller
that means "the window I would be working in" wants `session inspect`.

