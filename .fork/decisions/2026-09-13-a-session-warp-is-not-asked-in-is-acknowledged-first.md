> A decision on record, 2026-09-13. Binding until a later decision names it.

- **Before the first turn of an agent session in which Warp will not be asked
  for permission, the panel asks for a one-time acknowledgement, and the turn
  waits for it.** The maintainer chose option (b), verbatim: *"on default agent
  perm: (b)"*, of three offered the same evening: (a) disclose more loudly,
  (b) require a one-time acknowledgement before the first turn runs without
  Warp in the loop, (c) refuse to start a known agent until a mode is named.
  Warp still never picks a mode for the user (`fork::acp_mode()` has no default,
  and that stands). The reason is the fable-advisor's top-ranked risk: a
  stranger who names `claude-agent-acp` without `WARP_FORK_ACP_MODE` runs in
  the agent's `auto` mode, where its own classifier approves, and a prompt
  injection in a cloned repository can then drive a shell with the event log
  showing zero requests. The work is `HANDOFF-SECURITY.md` task 1.
