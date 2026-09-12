# codex-acp on the wire: the OpenRouter key fits, and the telemetry was never in this binary

Run 2026-09-11, closing item 3 of the decide run. The maintainer's direction was
*"sync codex, flip the flags, build it, and watch the first run on the wire"*,
with the standing note from the recon that `[analytics] enabled = false` is
*"necessary and not demonstrated sufficient"*.

**Both halves of that note turn out to be wrong, in opposite directions.** The
flag is not sufficient -- there is a second default-on path it does not touch --
and it is not necessary either, because neither path is reachable in the binary
that actually answers Warp. What follows is the evidence for both.

## The tree the recon read is not the tree that runs

Three different codex trees are in play, and the question was asked of the wrong
one twice.

| tree | what it is | dated |
|---|---|---|
| `~/git/codex` @ `404c126fc3` | the maintainer's year-stale checkout | 2025-09-15 |
| `upstream/main` @ `53ff712a48` | what the recon fetched and read | 2026-09-12 |
| **`f221438`, tag `rust-v0.137.0`** | **what codex-acp 0.16.0 actually vendors** | **2026-06-03** |

The third is the one that matters, and the binary says so itself -- its build
paths carry `/home/runner/.cargo/git/checkouts/codex-9eee5d47a939c68c/f221438/`.
`git rev-parse rust-v0.137.0` matches that sha exactly.

**`upstream/main` is also not what it looks like.** The fetch reported a forced
update, and the topology explains why reading it is misleading: `main` holds
**1** commit the tag does not, while the tag holds **7103** commits `main` does
not. openai/codex force-pushes a squashed `main` onto an old base. Reading HEAD
there is reading a tree with almost none of the history in it.

## What codex-acp is, and what it is not

`@zed-industries/codex-acp` is Zed's wrapper, and **codex itself has no ACP code
at all** -- zero filename matches for `acp`, and no `agent-client-protocol`
dependency in any `codex-rs` Cargo.toml. The wrapper is the entire ACP surface.

It is also already on this machine and has been since 2026-09-07: a prebuilt
223 MB binary in the npx cache at `577e9cd102bc41f1`, which is what wrote the
`~/.codex/installation_id` the recon flagged as its tell. That file is written by
`codex-core/src/installation_id.rs:57` during ordinary startup. **It is an
identity file, not a transmission** -- which resolves the tell without
contradicting anything below.

## The telemetry, in the tree that is compiled in

`codex-core` depends on `codex-analytics` and `codex-otel` **unconditionally** --
no cargo feature gates either. So both are compiled into codex-acp: 287
`codex_analytics` symbols and 106 `opentelemetry` symbols are in the shipped
binary. Reading the dependency graph, the recon's worry looks confirmed.

**Reading the call graph reverses it.**

- **Analytics.** `AnalyticsEventsClient::new` has exactly one non-test caller in
  the whole workspace: `codex-app-server/src/analytics_utils.rs:11`. codex-acp
  does not depend on `codex-app-server`, and its source imports no
  `codex_analytics` at all. The client is never constructed.
- **OTEL.** `codex_core::otel_init::build_provider` is called only from codex's
  *own binaries* -- `app-server`, `exec`, `mcp-server`, `tui`. `codex-core` never
  calls it. codex-acp is its own binary with its own `main` and never calls it.
- **Sentry.** The `feedback` crate has a real hardcoded DSN
  (`o33249.ingest.us.sentry.io`), and it uploads only on an explicit user
  feedback action.

The binary agrees with the call graph rather than the dependency graph:

| string searched | occurrences in the shipped binary |
|---|---|
| `ab.chatgpt.com` (the Statsig endpoint) | **0** |
| `statsig-api-key` | **0** |
| the Sentry DSN | **0** |
| `/codex/analytics-events/events` | 1 (the path const, no client to use it) |

Linked but unreachable, so LTO dropped the consts. **A missing string is weak
evidence on its own; here it has a mechanism, and the mechanism was found
first.**

## The default-on path the flag does not cover, which is real for codex proper

`codex-rs/core/src/config/otel.rs` at the pinned commit:

```rust
let exporter         = config.exporter.unwrap_or(OtelExporterKind::None);          // :17
let trace_exporter   = config.trace_exporter.unwrap_or(OtelExporterKind::None);    // :20
let metrics_exporter = config.metrics_exporter.unwrap_or(OtelExporterKind::Statsig); // :21
```

Metrics default to **Statsig**, which `otel/src/config.rs` resolves to
`https://ab.chatgpt.com/otlp/v1/metrics` with a hardcoded
`statsig-api-key: client-MkRule...` header -- and with
`if cfg!(debug_assertions) { return OtelExporter::None; }`, so it is **off in
debug builds and on in release**. `[analytics] enabled = false` reads
`cfg.analytics.enabled` and never touches this.

So the recon's suspicion was correct *about codex*. Anyone running the codex CLI
or TUI from a release build is exporting metrics to Statsig by default, and the
analytics flag does not stop it. It simply does not reach codex-acp.

## Watched on the wire

`warpctrl acp probe` driving the prebuilt binary, with a polling `ss` census over
every socket owned by the process -- the method from
`.fork/runs/localmodel-panel-2026-09-09/`, which needs no root.

**The instrument was calibrated before it was trusted**: a `curl` to
openrouter.ai during a census run produced rows, so a connection that is present
does register. A census that has never fired proves nothing.

| run | config | peers observed |
|---|---|---|
| one-word turn | opt-outs set | `104.18.3.115:443` only |
| three-tool turn (list, read, write) | opt-outs set | `104.18.2.115:443`, `104.18.3.115:443` |
| **negative control** | **opt-outs removed entirely** | `104.18.2.115:443` only |

Both addresses are openrouter.ai: its AAAA records `2606:4700::6812:273` and
`::6812:373` decode to `104.18.2.115` and `104.18.3.115`. **`warp-oss` itself
made zero outbound connections** across all three runs.

**The negative control is the finding.** Strip `[analytics]` and `[otel]` out of
the config completely and the wire does not change. The flags were never what
kept codex-acp quiet; the missing call sites were. Writing them is belt and
braces over code that is not reached, which is worth doing -- the pin can move --
but it should not be mistaken for the thing that is working.

### What this does not establish

The census polls at 0.1-0.2 s. A connection opened and closed inside one interval
would be missed, and a socket in `TIME_WAIT` has lost its process attribution and
would not match the filter. So this is *no observed egress*, not a proof of
absence. It is worth exactly as much as it is because the static read predicted
it first and the negative control agrees with it; one instrument alone would not
carry the claim.

Not looked at at all: what the `@zed-industries/codex-acp` npm package's install
step does before the binary ever runs.

## The OpenRouter key fits, and the config block the recon wrote would be refused

At `rust-v0.137.0`, `WireApi` has exactly one variant:

```rust
pub enum WireApi {
    /// The Responses API exposed by OpenAI at `/v1/responses`.
    #[default]
    Responses,
}
```

Chat completions are gone from codex. The recon's block said `wire_api = "chat"`,
read from HEAD, and the binary refuses it at config load:

```
`wire_api = "chat"` is no longer supported.
How to fix: set `wire_api = "responses"`
```

That looked fatal for the pairing, because OpenRouter is a chat-completions
service. It is not: **OpenRouter serves `/v1/responses`**, answering HTTP 200
with a properly shaped Responses payload. The working block is
`config-watched.toml`, and the only change from the recon's is that one value.

Measured end to end with the maintainer's existing OpenRouter credential: a
one-word turn answered `PONG`, and a three-tool turn listed a directory, read
`main.rs` and wrote `NOTES.md` with correct content.

## Two things about codex-acp as a fork agent, neither of them asked for

**It starts in `read-only`, not `auto`.** Its three modes are `read-only`,
`auto` ("Default") and `full-access`, and the session opens in the first. In the
three-tool turn Warp was asked about **1 of 3 calls** -- the `execute` that wrote
the file -- while the list and the read went unasked. That is a stricter default
than `claude-agent-acp`, which opens in `auto` and lets its own classifier answer
so Warp is never in the loop. No `WARP_FORK_ACP_MODE` is needed to make codex-acp
ask.

**It advertises a model list.** `configOptions` carries a `model` select, so the
panel's model chip (T14.14) has something to populate from. With a non-OpenAI
model id it also emits a warning as its first message: *"Model metadata for
`openai/gpt-4o-mini` not found. Defaulting to fallback metadata; this can degrade
performance and cause issues."*

Its `authMethods` are `chatgpt`, `codex-api-key` and `openai-api-key` -- named in
full even though a provider key is configured and no authentication is required,
which is the behaviour T21 measured and built `auth.rs` around.

## Built from source, and it agrees

`cargo build --release` on `zed-industries/codex-acp` @ `296069e`, 849 crates,
**6m57s**, 0 errors, 248 MB. codex-acp declares no custom release profile, so
this build has **no LTO**, where the npm binary does.

That difference is useful, because it tests the explanation rather than the
result. The dead Statsig const is absent from **both** builds:

| | npm 0.16.0 (LTO) | built here (no LTO) |
|---|---|---|
| `ab.chatgpt.com` | 0 | 0 |
| `statsig-api-key` | 0 | 0 |
| Sentry DSN | 0 | 0 |
| `codex_otel` symbols | present | **208** |
| `codex_analytics` symbols | 287 | **490** |

So it was never LTO doing it. The crates are linked in both -- 208 `codex_otel`
symbols in a binary with no Statsig endpoint in it -- and rustc passes
`--gc-sections` for executables, so an unreferenced call chain takes its
`.rodata` with it. **The explanation survived a build that could have falsified
it**, which is worth more than the string count on its own.

On the wire the source build is indistinguishable from the npm one: the same
three-tool turn, the same two openrouter.ai addresses, nothing else.

### The TIME_WAIT gap, closed

The caveat above -- that a closed socket loses its process attribution and would
slip past the filter -- was checked rather than left standing. Immediately after
the run, every non-loopback socket in **any** state:

```
2 ESTAB      160.79.104.10:443     users:(("claude",pid=39294))
1 ESTAB      35.190.46.17:443      users:(("claude",pid=39294))
1 TIME-WAIT  104.18.2.115:443      <- openrouter, ours
```

The only lingering socket from the run is openrouter's. The other three belong to
`claude` -- the Claude Code session that drove this work, reaching
`api.anthropic.com` and a googleusercontent host. **They are worth printing
precisely because they are not ours**: the instrument sees other processes'
traffic on this machine, so its silence about codex-acp is not blindness.

(The `claude` egress is item 6 of the decide run, accepted and documented on
2026-09-11 with an explicit instruction to stop re-measuring it. Noted here only
to attribute the rows, not to reopen it.)

## Using it

The binary is at `~/git/codex-acp/target/release/codex-acp`, and the npm one
works identically. The config lives in a scratch `CODEX_HOME` --
`~/.cache/codex-acp-wirewatch/config.toml` -- because **the maintainer's own
`~/.codex` was deliberately left untouched**; it has no `config.toml` at all
today, so adopting this means copying that file there, which is theirs to do.

```
CODEX_HOME=~/.cache/codex-acp-wirewatch \
OPENROUTER_API_KEY=<the key opencode already holds> \
WARP_FORK_ACP_COMMAND=~/git/codex-acp/target/release/codex-acp
```

`WARP_FORK_ACP_MODE` is **not** needed: codex-acp opens in `read-only` and asks.

One trap worth carrying: codex refuses to create its helper binaries when
`CODEX_HOME` is under `/tmp`, and says so in a warning it then proceeds past.
Put the scratch home somewhere else.

## Files

| file | what it is |
|---|---|
| `config-watched.toml` | the working config, telemetry opt-outs set |
| `config-control.toml` | the negative control, opt-outs removed |
| `probe-prebuilt.ndjson` | the one-word turn, full ACP transcript |
| `probe-rich.ndjson` | the three-tool turn |
| `wire-prebuilt.tsv`, `wire-prebuilt-rich.tsv` | socket census, opt-outs set |
| `wire-analytics-on.tsv` | socket census, negative control |
| `probe-mybuild.ndjson` | the three-tool turn against the source build |
| `wire-mybuild.tsv` | socket census, source build |
