> A decision on record, split out of `.fork/TASKS.md` on 2026-09-05. Binding until a later decision names it.

- **Observability precedes the remote surface** (2026-08-24, T11). Two
  independent reasons, and the second is the one that decided it: read-only-first
  because the value is in the read path and the risk is in the write path; and
  events-first because the failure that cost a month of kode-rs work was
  *silent*, and an event taxonomy is the detector for that class. A stream with
  nothing structured on it is a pipe with no protocol.
