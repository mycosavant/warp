> A decision on record, 2026-09-13. Binding until a later decision names it.

- **Warp says when the configured local model endpoint stops answering.** The
  maintainer, verbatim: *"watcher approved."* The reason is friction a18: the
  maintainer stops `llama-server` routinely to reclaim about 10 GB, and the four
  small AI features then fail with nothing on screen (TOLD in a18; to be
  re-measured per feature before building). The work is `HANDOFF-BUILDS.md`
  task 1. The probe goes to the user's own endpoint only and must not attach
  Warp's identifying headers (`Client::get_without_warp_headers`).
