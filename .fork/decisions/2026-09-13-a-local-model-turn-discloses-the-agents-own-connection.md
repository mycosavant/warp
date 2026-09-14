> A decision on record, 2026-09-13. Binding until a later decision names it.

- **When the panel answers from a local model through an agent measured to
  contact its vendor anyway, Warp tells the user, and what it says comes from a
  measurement of what is sent.** Decided by the maintainer, 2026-09-13:
  honesty and the fork's thesis require, at minimum, a disclosure when the
  panel runs a local model. The shape they asked for is a notice with a "got
  it" acknowledgement and an option to make that one acknowledgement the last
  notice. They asked for an instrumented run first, to measure exactly what is
  sent and when, so the notice says no more and no less than that. Whether the
  docs should recommend a different harness for local models, for users who
  care, is the maintainer's leaning, not a choice.

  **What is measured** (RAN, `.fork/runs/localmodel-panel-2026-09-09/` and
  `.fork/runs/vendorcalls-2026-09-13/`): during a turn answered entirely by
  `llama-server`, `claude-agent-acp@0.73.0` calls `api.anthropic.com` at
  startup: a feature-flag evaluation, an account bootstrap, a `penguin_mode`
  check and the public MCP registry. No prompt, `CLAUDE.md` or file content was
  in any request. `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` removes all of
  them unless a settings file's `env` sets it back, which the 2026-09-09 run's
  cwd did; WebFetch still sends each domain to `api.anthropic.com` unless
  `skipWebFetchPreflight` is set. Warp itself made no non-loopback connection.
  Warp does not block the agent's connection. *Corrected 2026-09-13: this
  paragraph said the variable did not remove the connections.*

  **Why disclose.** The fork does not promise that nothing leaves the machine
  (`.fork/docs/manual.md:56`: the claim is "no telemetry"). But someone who
  points the panel at a local model reasonably expects the turn to stay local,
  and the requests are made as their own Claude account when one is signed in:
  the bootstrap and `penguin_mode` calls carry the subscription OAuth token,
  and the bootstrap response holds the account's email and organisation (RAN,
  2026-09-13). *Corrected the same day: this said account scoping was
  ASSUMED.* This reverses the disclosure half of the 2026-09-11 ruling (commit
  `a7b6803c8`), which had refused a panel notice; "accepted" still stands.

  **The order.** `HANDOFF-VENDORCALLS.md` runs first and decides the content;
  `HANDOFF-BUILDS.md` task 4 builds the note from it. If the run finds prompt
  or file content in a request body, a note is not enough and the maintainer
  decides between blocking and not recommending that agent for local models.
  Any stop the run finds goes in the launch command the user writes, and the
  docs recommend a harness for local models only after it is measured to reach
  loopback and nothing else.
