> A decision on record, 2026-09-05. Binding until a later decision names it.

- **`/remote-control` hands one conversation to a phone, and that phone may
  prompt it** (2026-09-05, at the maintainer's ask: wire the chip up as the
  fork's mobile control surface, the way Claude Code's works). T19 left one
  question open, whether a QR-paired device may submit a prompt, and refused
  to answer it as plumbing. The answer is scoped: a device paired by
  `warpctrl pair show` still may not; a device paired by a code minted *for
  one conversation*, by a person at the machine pointing at it, may prompt
  that conversation and say yes to its requests, and can touch nothing else.
  The credential is the same weak QR. What changed is the gesture, which is a
  stronger consent than a variable, and the confinement, which is on every
  grant the device mints and checked before every handler (`confine.rs`).
  `WARP_FORK_REMOTE_APPROVE` is untouched: it is about answering for every
  agent, a different claim. The residual is the one the console has always
  had, plaintext HTTP on the LAN; bind a Tailscale address where that matters.
