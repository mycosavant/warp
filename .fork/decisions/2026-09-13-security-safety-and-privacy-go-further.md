> A decision on record, 2026-09-13. Binding until a later decision names it.

- **The fork holds itself to a higher bar on security, safety and privacy than
  it has, judged for users who are not the maintainer, and the fable-advisor's
  2026-09-13 assessment is the record's posture.** The maintainer, verbatim:
  *"let the record reflect the posture that Fable has taken here, and i
  personally advise that we step our game up on security, safety and privacy
  when/where reasonably possible. with harneses like claude code and codex
  using 3rs party models, there's only so much we can do and i understadn that
  fully, but we should go further than we are."* Earlier the same evening:
  *"Eventually there will be users other than me, so we def need more robust
  security practices and hygeine."*

  The assessment's findings, ranked for a stranger on defaults, are
  `HANDOFF-SECURITY.md`'s tasks: an agent session in which Warp is never
  asked; history, logs and state readable by other accounts on Linux; keys at
  `644` under a public constant key; a phone control token with no clock; an
  unconstrained ten-year console authority delivered over plain HTTP; HTTP
  clients that bypass the egress backstop. What it found solid is kept:
  single-use pairing codes from `OsRng`, constant-time token comparison, the
  peer-UID check on the local socket, telemetry off at the source, and the
  refusal to fall through to warp.dev.

  **How to apply it:** a default is judged by what it does for someone who
  never reads `.fork/`. Where the fork cannot control a third-party harness, it
  measures and says so rather than claiming more. Records carry the final call
  with the maintainer's words quoted (see `CLAUDE.md`, *How to write in this
  file*).
