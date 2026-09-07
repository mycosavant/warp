# The other agents on the wire, 2026-09-07 (T21.3a)

`warpctrl acp probe` against the two agents T21's survey had only read,
from WSL, binary `v0.fork.c56fc22de`. Each file is the probe's output as
printed, one JSON object per line; the `.err` beside it is what the probe
said on stderr.

| agent | `initialize` | `session/new` |
|---|---|---|
| `@zed-industries/codex-acp@0.16.0` | answered: `codex-acp` 0.16.0, protocol 1, `loadSession`, session list/resume/close, `auth.logout` | **refused, `Authentication required`** |
| `@google/gemini-cli@0.58.0 --acp` | answered: `gemini-cli` 0.58.0, protocol 1, `loadSession`, audio prompts, MCP over http and sse | **refused, `Gemini API key is missing or not configured`** |

So neither row of the survey table can be filled from the wire on this
machine: the model list, if either agent sends one, is on the `session/new`
reply, and both agents want a credential before they will open a session.
Codex offers `chatgpt` (a paid ChatGPT login), `codex-api-key` and
`openai-api-key`; Gemini offers `oauth-personal`, `gemini-api-key`,
`vertex-ai` and `gateway`. None is present here, by name (`env` was
grepped for the variable names only, and `~/.gemini` does not exist).

Two things the probes did settle without a credential:

- **Both advertise `authMethods` on `initialize`, and the fork never sends
  `authenticate`.** `grep authenticate app/src/ai/acp_agent/` is empty. With
  `claude-agent-acp` this never mattered: Claude Code reads its own login
  from disk and the panel has no auth step. With Codex or Gemini the panel
  as built would fail every `session/new` the way the probe did, and the
  person would have to log in through the agent's own CLI first, out of
  band, and hope the ACP process finds the same file. Whether it does is
  the first thing to measure once a credential exists. Sending
  `authenticate` with a chosen method id is a small addition to
  `acp_agent`; choosing the method is not Warp's, for the reason
  `WARP_FORK_ACP_MODE` has no default.
- **`opencode` has an OpenRouter credential on this machine and nothing
  else does** (`~/.local/share/opencode/auth.json` names one provider). So
  T21.3b, the picker against opencode's OpenRouter list, is the survey item
  that can run tonight, and the one that answers I22.

What is still only read: whether Codex sends `configOptions` at all, and
whether Gemini's `models` draft is what its `session/new` carries at 0.58.0.
The bundle said so; the wire has not.
