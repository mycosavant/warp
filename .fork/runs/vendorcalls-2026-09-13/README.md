# What the agent sends its vendor during a local-model turn, 2026-09-13

Run from `.fork/HANDOFF-VENDORCALLS.md`. **No fork code changed.** One tracked
tool changed: `.fork/tools/egress-poll-wsl.sh` (two instrument defects, below).
Claims are tagged RAN, READ, TOLD or ASSUMED.

---

## The answer

1. **No prompt, CLAUDE.md or file content went anywhere but `llama-server`.**
   105 requests from the agents were decrypted across 38 runs, to every host;
   none carried any of the three canaries in its URL, headers or body (RAN).
2. **`CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` works, and the 2026-09-09
   runs that said it did not were overridden by this checkout.** Both ran with
   the session cwd at `/home/effatha/git/warp`, whose gitignored
   `.claude/settings.local.json` set that variable to `""` (READ on the run
   date; the file's mtime was 2026-09-03, so it most likely held the key on
   09-09). The maintainer removed the line after this run: it was left over
   from a remote-control test (TOLD). A
   settings `env` entry beats the process environment: copying that one key
   into a scratch directory under `$HOME` brought every call back (RAN, bisect
   below). With the flag
   in force and no settings file undoing it, the agent made **zero** requests
   off the machine (RAN, WSL and Windows).
3. **When the calls happen, they are made as your account.** Two of them send
   the subscription OAuth token, and one returns the account's email, uuid and
   organisation. They do not depend on it: with no credentials the same
   requests go out and those two get 401 (RAN).
4. **Whatever is in `ANTHROPIC_API_KEY` is sent to `api.anthropic.com`**, even
   with `ANTHROPIC_BASE_URL` pointing at loopback. The `x-api-key` on the
   feature-flag call hashes to exactly `sha256("local")` (RAN). That a real
   third-party key would go the same way is ASSUMED from this.
5. **A stop exists, and a repository can undo it.** The flag reaches zero, and
   a closed proxy port (`HTTPS_PROXY=http://127.0.0.1:9`) reaches zero while
   the turn still answers. A project settings file that blanks the flag and
   `HTTPS_PROXY` undoes both: the agent opened a direct socket to
   `160.79.104.10:443` (RAN), the address 2026-09-09 resolved for
   `api.anthropic.com` (READ). Only enforcement outside the agent (configuration
   H) would survive that, and H was not run.
6. **The WebFetch preflight survives the flag** and sends the fetched domain to
   `api.anthropic.com/api/web/domain_info`, with no credential.
   `skipWebFetchPreflight: true` removes it (RAN).
7. **`opencode` and `codex-acp` reach loopback only**, opencode after
   `OPENCODE_DISABLE_MODELS_FETCH=1`; without it opencode downloads a 4.6 MB
   model catalogue from `models.opencode.ai` when its cache is cold (RAN).

**After the line was removed, same night** (RAN 2026-09-14 01:08Z): a probe
from `/home/effatha/git/warp` itself, flag in the launch command, proxy and
census as above. Zero requests besides the curl control; `claude` on
`127.0.0.1:8080` only; control 2 fired; the turn answered. Its files were not
kept, since it repeats `T-nosettings-flagset` from the checkout.

The handoff's falsifiers: the canary did not appear, so a notice is not ruled
out. *"Nothing stops it"* is falsified by the flag and by D. *"It uses your
account"* is falsified as a dependency (B) and holds as a fact about what is
sent when an account is present.

---

## What ran

| | |
|---|---|
| agent | `claude-agent-acp` 0.73.0, `dist/index.js` sha256 `9d73d1f0…9422a`, run by path from `~/.npm/_npx/6130df88bef2cca3` with `node` v24.5.0 (`e0e46d3a…62a4`). No `npx` invocation in any run (RAN) |
| its CLI | `@anthropic-ai/claude-agent-sdk-linux-x64` 0.3.257, `claude` sha256 `9a64bda9…7f05`, reports `2.1.257 (Claude Code)`; named with `CLAUDE_CODE_EXECUTABLE` |
| Windows (G) | same `index.js` hash from the Windows npx cache; `claude.exe` `190a5fe1…d400`; `node.exe` v26.7.0 `e921fe53…05b9`; `C:\dev\warp` at `ac5f6b25f` for the probe. Hashes in `G-*/sha256.txt` |
| others | `opencode` 1.18.25 `d91e0d33…eedb`; `codex-acp` built from `~/git/codex-acp` at `296069e`, clean tree, `24f16304…2b33` (that the binary was built from that commit is ASSUMED from its mtime). `other-agents-sha256.txt` |
| model | `llama-server` b10844, Gemma 4 12B, `127.0.0.1:8080` on the Windows side, ctx 98304. Started for this run and stopped after it |
| driver | `warpctrl acp probe` (WSL `target/release/warp-oss`), or `instruments/multiturn.py` for E, which sends three prompts in one ACP session |
| prompt canary | `CANARY-PROMPT-5d19b2` in every prompt |
| context canary | `CANARY-CTX-7f3a91` in the session cwd's `CLAUDE.md`, which the agent loads into its prompt |
| file canary | `CANARY-FILE-c24e08` in `notes.txt` in the cwd, never asked for |

### Instruments

- **A decrypting proxy**, `mitmdump` 8.1.1 on `127.0.0.1:8081`, with
  `instruments/redact_addon.py` as the only output: method, host, path with
  identifier segments replaced by `<id>`, query **keys**, status, sizes, header
  **names**, a 12-hex sha256 prefix of each credential header compared with the
  same prefix of `accessToken` and `refreshToken`, the JSON key structure of
  both bodies with no values, and canary hits. No flow file was written (no
  `-w`). Before any real traffic, `instruments/verify_redaction.sh` planted a
  fake access token, refresh token, API key, cookie, query value, body value
  and path uuid in one request and grepped every file the rig produced for each
  of them: none present, and the fake token matched as `accessToken` (RAN,
  twice, after each addon edit). The proxy saw the agents because they honour
  `HTTPS_PROXY` and trust `NODE_EXTRA_CA_CERTS` (the Bun-compiled `claude`
  included), shown by decrypted requests in A-repo on WSL and G-webfetch on
  Windows.
- **A socket census by pid**: `.fork/tools/egress-poll-wsl.sh` at 200 ms, plus
  `instruments/proctree.py`, which records every descendant of the run's own
  shell so the agent's sockets are separated from this session's `claude`
  (pid 457218). Windows: `.fork/tools/egress-poll.ps1` via `instruments/g.ps1`.
- **Controls**, in every run: 1a, `curl` through the proxy (appears in
  `requests.jsonl`, except in `G-D` and `G-D-webfetch`, where the proxy
  variable pointed at the closed port); 2, `node` (WSL) or `curl.exe` (Windows) to example.com
  with no proxy (appears in the census). Control 1b is F and G-webfetch.

`instruments/run.sh` is one configuration end to end; `summarise.py` and
`summarise_win.py` produced every `summary.txt`.

---

## The requests, when they happen

With credentials present (A, C, E, A-repo; WSL). All within 1.4 s of the agent
starting.

| request | credential sent | size up / down | what it is |
|---|---|---|---|
| `POST /api/eval/sdk-zAZezfDKGoZuXXKe` | `x-api-key` = the `ANTHROPIC_API_KEY` value | 482 B / 101 KB | feature-flag evaluation. Body keys: `attributes{apiBaseUrlHost, appVersion, deviceID, entrypoint, hasRemoteEnvironment, id, organizationRole, platform, sessionId, subscriptionCreatedAt, userType}`, `forcedFeatures`, `forcedVariations`, `url`. The response is a `features` map (`tengu-off-switch`, `tengu-fable-off-switch`, …). That this is GrowthBook is ASSUMED from the shape |
| `GET /api/claude_cli/bootstrap?entrypoint&model` | OAuth bearer = `accessToken` | 0 / 762 B | response keys include `oauth_account{account_email, account_uuid, organization_name, organization_type, organization_uuid, …}` and `additional_model_options` |
| `GET /api/claude_code_penguin_mode` | OAuth bearer = `accessToken` | 0 / 39 B | `{enabled, disabled_reason}` |
| `GET /mcp-registry/v0/servers?limit&version&visibility`, 4 pages | none | 0 / ~720 KB | the public MCP server registry |

Seen only in some runs:

| request | when | credential |
|---|---|---|
| `POST /api/event_logging/v2/batch`, 14,141 B up | only in T-nosettings-flagunset: no settings files at all and the flag absent from the process environment. Not in any run where the flag was set in the environment and blanked by a settings file. Why is not established | none |
| `GET /api/web/domain_info?domain` | every WebFetch, flag or no flag, unless `skipWebFetchPreflight` | none |
| `GET downloads.claude.ai/claude-code-releases/plugins/claude-plugins-official/latest`, then three CONNECTs to `github.com` with no decrypted request, made by `git remote-https` children fetching plugin repositories (`getsentry/plugin-claude`, `greptileai/claude-plugin`, …; `F-webfetch/procs.tsv`) | once, 8 s into the 63 s F-webfetch turn | none |
| `GET /v1/mcp_servers` (the claude.ai connectors call from `open-questions.md`) | **never**, in any run | |

The account-derived eval attributes (`organizationRole`,
`subscriptionCreatedAt`, `hasRemoteEnvironment`) are present when the config
directory holds an account and absent from B, whose config is empty (RAN).

---

## Configurations

TTFT is from driver start to the first `agent_message_chunk`. Cold and warm
prefills differ by 10-40 s, so only the warm pairs compare a configuration's
cost; the others show only that the turn answered.

| # | run | cwd | result | answered | TTFT |
|---|---|---|---|---|---|
| A | `A` | `target/vc-ws` (inside the repo) | 7 requests, table above, no canary | yes | 20.4 s cold |
| A | `A-warm1`, `A-warm2` | same | 6 requests each; in `A-warm2` one registry page has no status because the process exited mid-request | yes | **1.39 s, 1.25 s** |
| A, flag unset | `A-flag-unset` | same | the same set: the repo file had already blanked it | yes | 3.5 s |
| B | `B-empty` | same | `CLAUDE_CONFIG_DIR` empty: same requests; bootstrap and penguin_mode 401 with `x-api-key: local`, no `Authorization` | yes | 26.9 s |
| B | `B-trusted-nocreds` | same | config holding only a trust entry for the cwd: identical to B-empty | yes | 26.7 s |
| C | `C` | same | `ENABLE_CLAUDEAI_MCP_SERVERS=false`: no change. The call it names never occurred | yes | 1.9 s |
| D | `D` | same | proxy on a closed port: SYN to `127.0.0.1:9`, **zero non-loopback sockets**, no requests | yes | 6.4 s |
| D | `D-warm1`, `D-warm2` | same | same | yes | **8.19 s, 5.83 s**, so D costs 4.5-7 s per turn, spent waiting on the refused requests |
| D, defeated | `D-project-override` | `$HOME` copy with `settings.local.json` `env` blanking the flag and both proxies | **direct socket to `160.79.104.10:443`** (api.anthropic.com), bypassing the proxy | yes | 15.1 s |
| E | `E` | `target/vc-ws` | all 7 requests during turn 1's first 1.2 s; **none in turns 2 and 3** of the same session | yes, 3 turns | 1.7 s |
| F | `F-webfetch` | same | `domain_info` preflight, then the fetch to example.com (UA `Claude-User`), plus the marketplace fetch above | yes, correct title | 62.8 s |
| F | `F-webfetch-skip` | same + `.claude/settings.json` `skipWebFetchPreflight: true` | no `domain_info`; the fetch itself still goes | yes | 36.9 s |
| F, flag in force | `F-flagset-webfetch` | `$HOME` copy | only `domain_info` and the fetch | yes | 28.7 s |
| G | `G-A` | `C:\Users\onemind`, Windows user settings set the flag to `1` (READ) | zero requests; `claude.exe` on loopback only; control 2 fired | yes | |
| G | `G-D` | same, proxy on port 9 | no requests; control 2 fired, but the 1.6 s turn's `claude.exe` was **not captured** by the census | yes | |
| G | `G-D-webfetch` | same, proxy on port 9 | `claude.exe` on loopback only; control 2 fired. WebFetch refused: *"blocked by security policies"* | yes | |
| G | `G-webfetch` | same, proxy up | `domain_info` and the fetch, decrypted: control 1b for Windows | yes | |
| H | not run | | see below | | |
| opencode | `O-opencode-coldcache` | `$HOME` copy, empty `XDG_CACHE_HOME` | loopback (its own server on `:4096`, llama-server) and `GET models.opencode.ai/api.json`, 4.6 MB, no credential | yes | 2.1 s |
| opencode | `O-opencode-coldcache-nomodels` | same + `OPENCODE_DISABLE_MODELS_FETCH=1` | **loopback only**, zero requests | yes | 2.2 s |
| codex | `X-codex` | `$HOME` copy, `CODEX_HOME` with `instruments/codex-config.toml` (analytics and otel off) | **loopback only**, zero requests | yes, prefixed with *"Model metadata for `gemma-4-12b` not found"* | n/a: the first chunk is codex's own warning |

`O-opencode` and `O-opencode-nomodels` ran against a warm cache and made no
request either way, so they do not test the switch; the cold-cache pair does.

---

## Why 2026-09-09 saw connections: the bisect

The first runs in a scratch directory under `/tmp` made no vendor request at
all, which contradicted both earlier records. (`A0-noproxy` is left out of the
table below: it ran with no proxy and before the poller fix, so neither
instrument could see it.) Each run's cwd and settings file are in its
`cwd.txt`, written from the driver commands after review, because the rig did
not capture them. Every run inside the checkout
did. Moving one thing at a time:

| run | cwd | vendor requests |
|---|---|---|
| `T-untrusted-git` (a `git init` copy) | `/tmp` scratch | 0 |
| `T-home-untrusted` | `$HOME` copy, no settings file | 0 |
| `A-repo` | `/home/effatha/git/warp` | 7 |
| `T-home-mcpjson` | `$HOME` copy + a dummy `.mcp.json` | 0 |
| `T-home-settingslocal` | `$HOME` copy + the repo's `.claude/settings.local.json` | 7 |
| `T-home-only-hooks`, `-permissions`, `-disabledMcpjsonServers` | that file cut to one key | 0 each |
| `T-home-only-env` | that file cut to `env` only: `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC: ""` | 7 |
| `T-home-dummyenv` | an `env` block naming an unrelated variable | 0 |
| `T-home-flagunset` | no project settings, flag absent from the process env | 0, because `~/.claude/settings.json` sets it to `"1"` (READ) |
| `T-nosettings-flagunset` | no settings anywhere (`CLAUDE_CONFIG_DIR` empty), flag absent | **8**, the only run with `event_logging` |
| `T-nosettings-flagset` | no settings anywhere, flag `1` | **0** |

Trust was the first suspect and is not it: `B-empty` had no trust record and
made the calls. Git is not it either. The `env` key is the whole difference.
That a settings file in a parent directory applies to `target/vc-ws` is RAN;
whether Claude Code walks up to the git root or to each parent is not
established.

The 2026-09-09 panel run's pane cwd was the checkout (READ, its README), and
the pricefetch probe passed `--cwd /home/effatha/git/warp` (READ,
`optout-probe.sh`). Neither record could have seen this: they had sockets, not
requests, and the variable was set in both launch commands.

---

## H: not run

`srt` is on this machine, and it is `@anthropic-ai/sandbox-runtime` 1.0.0
installed globally through npm into the nvm prefix (READ, `readlink`), not a
pinned source build, which the handoff and
`decisions/2026-09-13-acp-agents-are-not-launched-through-a-package-manager.md`
require. H is the only configuration that would survive `D-project-override`,
so it is the one worth running next, from a source build.

---

## Instrument defects found on the way

- **The WSL poller never saw a Node 24 process.** Node v24.5.0 reports its
  process name to `ss` as `MainThread`, and the pattern matched `node`. Control
  2 therefore did not fire in the first four runs; `claude-agent-acp`'s own
  node process was invisible too, though it made no non-loopback connection
  once visible. Fixed in `egress-poll-wsl.sh`; runs before the fix (`A0`,
  `A-repo`, and the first `A`, since rerun) have a census that covers `claude`
  but not node.
- **The same pattern hides `opencode` and `codex-acp`.** The first pair of
  their runs showed the control and nothing of the agent. The poller now takes
  `EGRESS_POLL_PATTERN=.`, and every opencode and codex run in this record used
  it.
- **mitmproxy dials upstream before an addon can refuse the request.** Blocking
  `registry.npmjs.org` (so no agent could fetch a package mid-census) still
  opened a TLS connection to it until both `connection_strategy=lazy` and
  `upstream_cert=false` were set. Tested with the registry: 0 upstream
  connections after the fix. No run in this record requested the registry.
- **The Windows census misses a process that lives under a second.** An
  unthrottled `curl.exe` finished before the poller's once-a-second name
  refresh, and so did a warm 1.6 s turn's `claude.exe` in the second `G-D`.
  The control is now rate-limited; `G-D-webfetch` has both the agent's socket
  and the control in one run.

---

## What this does not establish

- **The panel path was not rerun.** Every run is `acp probe` or the multi-turn
  client. That the probe reproduces the panel's connections is READ from the
  2026-09-09 records.
- **Periodic or delayed calls.** Turns lasted 1-63 s. The marketplace fetch
  appeared once, 8 s into the longest turn and in no shorter one, so something
  on a timer longer than a minute would not be here.
- **The file canary was never read by the agent**, so it tests unsolicited
  upload only. A tool call that reads a file and then something going to the
  vendor is untested.
- **Windows with the flag absent** was not measured: the Windows user settings
  set it, and G ran only the winners.
- **codex-acp was measured only with analytics and otel off** in its config,
  the posture from `.fork/runs/codex-wire-2026-09-11/`.
- **One model, short prompts.** Gemma 4 12B, one-sentence answers, one
  WebFetch.

---

## Credentials and raw captures

- The record holds, per credential header, a 12-hex sha256 prefix and whether
  it equals the prefix of `accessToken` or `refreshToken`. No token, no body
  value, no query value.
- No flow file was ever written. Deleted at the end of the run: the proxy's
  logs, its CA key and certificate, the raw per-sample socket files
  (`*.tsv.raw`, which also held this session's own sockets), the scratch config
  directories, the canary workspaces, and the probe sessions' own transcripts
  (twelve directories under `~/.claude/projects` named for the scratch cwds,
  one file in `-home-effatha-git-warp`, six in the Windows
  `C--Users-onemind` project, each identified by the prompt canary and a
  modification time after the run began).
- Scanned after copying, before this README and the `cwd.txt` files existed: 313 files in this directory, both accounts' access
  and refresh tokens (WSL and Windows), whole and a 30-character slice: 0 hits
  (RAN).
- The eval path segment `sdk-zAZezfDKGoZuXXKe` is kept. It is the client key
  compiled into Claude Code, identical in every run and every account state.
  That it is not account-specific is RAN only for the one account and the empty
  config.

## Files

| file | what |
|---|---|
| `<run>/cwd.txt` | the session cwd and any settings file in it, from the driver commands |
| `<run>/summary.txt` | process tree, distinct remotes per agent pid, controls, requests, answer, TTFT. For `G-A` and `G-D` the answer line is empty because `summarise_win.py` failed to parse that output; `probe.ndjson` holds the answer |
| `<run>/requests.jsonl` | the redacted per-request record. Key structures over 4 KB were cut to their top-level keys when copied |
| `<run>/sockets-tree.tsv` | census rows for this run's own process tree only (WSL) |
| `<run>/win-sockets.tsv` | the Windows poller's output (G) |
| `<run>/procs.tsv`, `timeline.txt`, `env.txt`, `sha256.txt`, `probe.ndjson` | the process tree, phase timestamps, the agent's environment, hashes, the driver's output |
| `instruments/` | every script and config above, as run |
