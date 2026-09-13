> A decision on record, 2026-09-13. Binding until a later decision names it.

- **Warp says when the configured local model endpoint stops answering.**
  Approved by the maintainer on 2026-09-13, closing the question T21 and
  friction a18 left as theirs. The reason is a18's: the maintainer stops
  `llama-server` routinely to reclaim ~10 GB, and the four small AI features
  then fail with nothing on screen, so the stop is an expected event and its
  silence is the defect. Scope and shape are `HANDOFF-BUILDS.md` task 1. The
  probe goes to the user's own endpoint only, and it must not attach Warp's
  identifying headers (`Client::get_without_warp_headers`, per `CLAUDE.md`).
