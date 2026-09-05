> A decision on record, split out of `.fork/TASKS.md` on 2026-09-05. Binding until a later decision names it.

- **The web surface goes on `warpctrl`, never on 9282** (2026-08-24, T10.2).
  Upstream's `crates/http_server` answers unauthenticated and is ungated by fork
  policy. `warpctrl` is the server with `auth.rs`, the credential broker and the
  peer-UID check.
