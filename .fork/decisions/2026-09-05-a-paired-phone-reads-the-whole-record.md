> A decision on record, 2026-09-05. Binding until a later decision names it.

- **A paired phone may read a conversation's whole record** (2026-09-05, board
  item 6 phase 3, at the maintainer's ask: *observing runs remotely*).
  `agent.trace` is on `PAIRABLE_ACTIONS`. It returns the agent's own session
  file joined with Warp's log: every prompt, every tool input in full, every
  result, the agent's thinking where its harness writes it. Phase 1's page
  kept this off the list because a QR code is a weak credential; what changed
  is not the credential but the accounting. `events.subscribe`, pairable since
  T11.2, already streams tool names, input previews and working directories
  for every agent in the instance, so the trace is the same material at full
  resolution and not a new kind of disclosure; it is a read, so a stolen
  device token learns and cannot act; and the residual that actually matters
  is the one the console's docs already name, plaintext HTTP on the LAN, which
  a Tailscale bind retires. `pairing.rs` carries the argument beside the
  entry, the pinned test names the widening, and the observability page's
  "deliberately not" paragraph now says when and why it changed.
