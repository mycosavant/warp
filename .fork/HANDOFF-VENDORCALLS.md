# Handoff: what the agent sends to its vendor during a local-model turn

Written 2026-09-13 from the fable-advisor's design of the same evening, which
the maintainer adopted. **This run comes before `HANDOFF-BUILDS.md` task 4**
(the panel disclosure), because two likely outcomes would each rewrite what
that disclosure says. It needs no GUI and no merge; it can start any time.

Claims below are tagged RAN, READ, TOLD or ASSUMED, per `CLAUDE.md`.

## The question

During a panel turn answered entirely by a local `llama-server`,
`claude-agent-acp@0.73.0` opened TLS connections to `api.anthropic.com`
(RAN, `.fork/runs/localmodel-panel-2026-09-09/`), and
`CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` did not remove them (RAN,
`.fork/runs/pricefetch-2026-09-09/`). Nobody has seen what is in those
requests, whether they are made as the maintainer's account, when in a session
they happen, or whether anything stops them.

What is known around it, and its limits:

- **Anthropic's own docs name the hosts a sandboxed Claude Code needs**
  (READ, `code.claude.com/docs/en/sandbox-environments`, *Sandbox runtime*,
  2026-09-13): `api.anthropic.com` or the configured provider, and on a
  third-party provider `api.anthropic.com` as well, because the WebFetch domain
  safety check calls it unless `skipWebFetchPreflight: true`; plus `claude.ai`
  and `platform.claude.com` for OAuth sign-in and token refresh, droppable when
  authenticated with an API key. The 2026-09-09 turn used no WebFetch, so the
  preflight should not explain it (ASSUMED). Token refresh is a candidate; it
  would appear on `claude.ai` or `platform.claude.com`, not only
  `api.anthropic.com`, so **the census must record every host**.
- **The byte counts are not per turn.** The run's `socket-bytes.txt` totals
  (9,286 bytes up, 171,451 down, four connections) belong to pid 337595,
  sampled 17:45:41-46, a later `acp probe`, not a panel turn: the panel-turn
  agents were pids 322226 and 326427 and turn 2 ended at 17:41:25 (READ, the
  run's README and `timeline.txt`). Per-turn volume, and whether anything is
  sent after startup, is unmeasured.
- **The subscription token is on disk.** `~/.claude/.credentials.json` exists
  in the WSL home (RAN, `stat`), so an agent launched with
  `ANTHROPIC_API_KEY=local` still has it. Whether it uses it when an API key is
  set is ASSUMED either way.
- **One such call was decrypted before.** `.fork/tickets/open-questions.md`,
  *Method 2*, shows `GET /v1/mcp_servers?limit=1000` from the `claude` child
  (READ). That it is the claude.ai connectors list and account-scoped is
  ASSUMED; that run was an ordinary subscription turn with no local model.
- **A switch may exist.** The 0.73.0 binary contains the strings
  `ENABLE_CLAUDEAI_MCP_SERVERS` and `disableClaudeAiConnectors` (RAN, a string
  search). What they do is ASSUMED until configuration C runs.

## Instruments

Use `warpctrl acp probe` inside the distribution, which reproduces the
connection with no Warp GUI (RAN 2026-09-09). Run the agent binary from its
existing cache path and record its sha256; do not let `npx` fetch anything
during the run (`decisions/2026-09-13-acp-agents-are-not-launched-through-a-package-manager.md`).
Record each probe's pid and time window, so bytes are attributed to the turn
that made them.

1. **A decrypting proxy.** `mitmdump` on loopback, `HTTPS_PROXY=http://127.0.0.1:8081`,
   `NO_PROXY=127.0.0.1,localhost`, and the mitm CA trusted in the agent's
   environment only (`NODE_EXTRA_CA_CERTS`). The `claude` binary is
   Bun-compiled; whether it honours that variable is ASSUMED, so control 1b
   must fire.
2. **A socket census by pid**, beside the proxy, using the poller from the
   2026-09-09 **panel** run, whose positive control fired. Not the pricefetch
   poller, whose control never fired (friction a13). A non-loopback socket that
   is not to the proxy means the agent bypassed it.

### Controls, each run before believing a result

- 1a. `curl` to example.com through the proxy: the proxy logs it.
- 1b. One WebFetch turn: the preflight is a documented Anthropic call, so it
  calibrates whether the proxy can see the agent's requests at all.
- 2. `node` to example.com with no proxy: the census logs it.

### The canary

A unique string in the prompt and a different one in a file in the session
cwd. Grep every captured request body, to every host, for both.

## Configurations, in this order (not a grid)

| # | configuration | what it answers |
|---|---|---|
| A | baseline: credentials present, the non-essential flag set | what is sent today, to which hosts |
| B | A with `CLAUDE_CONFIG_DIR` pointed at an empty directory | does it depend on the account |
| C | A with `ENABLE_CLAUDEAI_MCP_SERVERS=false` | does the switch stop the connectors call |
| D | fail-closed: `HTTPS_PROXY=http://127.0.0.1:9` (a closed port), `NO_PROXY` loopback | does a turn survive with no route out (advisory: only if the agent honours the proxy) |
| E | turn 1 against turn 3 in one session | startup or every turn |
| F | a WebFetch turn, and one with `skipWebFetchPreflight: true` | the documented preflight and its documented off switch |
| G | the winner of A-D, repeated with the agent on the **Windows** side | a different credential store and network path |
| H | **optional**: the agent under Anthropic's sandbox runtime with no network domains allowed | an **enforced** version of D. On Linux the runtime removes the network namespace and routes everything through its own proxy (READ, its README), so nothing can bypass it. The README also says loopback is not directly reachable on Linux, so whether the agent can still reach `llama-server` is the question. Install it from a pinned source build, not `npx` |

For every run record: does the turn still answer, and time to first token
against A. Then the same census, configuration A only, for `opencode acp` and
`codex-acp` pointed at the same `llama-server`, so the docs can recommend a
launch line measured to reach loopback and nothing else.

## Credentials in the record

**Never commit a raw flow file.** Write a mitmdump addon that records, per
request: method, host, path, query keys, status, sizes, header **names**, the
first 12 hex digits of a sha256 of the `Authorization` value compared with the
same prefix of the token in `.credentials.json` (so "same account" is shown
without publishing the token), the JSON body's key structure, and whether
either canary appears. Delete the raw flows at the end and say so in the
README.

## Falsifiers, stated before the run

- **A canary in any body sent off the machine.** Then a notice is the wrong
  answer: this becomes "block it or stop recommending this agent for local
  models", and the maintainer decides which before anything is built.
- **"Nothing stops it"** is falsified by any configuration reaching zero
  non-loopback sockets, with control 2 firing, while the turn still answers.
- **"It uses your account"** is falsified if B shows the same requests with no
  `Authorization` header.

## What to do with the result

- Record in `.fork/runs/vendorcalls-<date>/`.
- **If a stop works**, it goes in the launch command the user writes (the
  manual's local-model recipe), not injected by Warp: environment set on
  `wsl.exe` does not reach the Linux process without `WSLENV`
  (READ, `wsl_command_executor.rs:83-86`), so an injected variable would fail
  silently for a WSL agent. The disclosure then names the stop and stays hidden
  when the command already carries it.
- **Firewall rules are out** as a product mechanism: they need admin rights,
  the agent's binary path changes with each version, a host block on
  `api.anthropic.com` also kills the maintainer's own Claude Code, and
  Hyper-V firewall rules cover the whole WSL VM (the last is TOLD, not
  verified). If H works, the sandbox runtime is the better containment, and it
  also answers `HANDOFF-SECURITY.md`'s excluded threat (anything running as the
  user) for agents.
- Update `docs/agent-transports.md`, then write `HANDOFF-BUILDS.md` task 4's
  wording from the table: agent and version measured, hosts, whether as the
  user's account, whether bodies carried content, the stop if any.
