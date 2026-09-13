> A decision on record, 2026-09-13. Binding until a later decision names it.

- **A `warpctrl graph run` holds the machine awake for its duration whenever
  system sleep is enabled.** The maintainer, verbatim: *"graph run should keep
  the machine awake. since that entry, i killed the sleep for now, so that's
  not a thing currently, although the run should keep the machine awake if
  sleep is enabled"*. This answers `WB-SLEEP` (T15). The mechanism exists
  upstream (`crates/prevent_sleep`: Windows and macOS backends, a no-op
  elsewhere, READ) and is used only by `http_client`. How it reaches a run is
  `HANDOFF-BUILDS.md` task 2, shaped by one fact: `graph run` is often driven
  from the Linux `warpctrl` inside WSL, where the guard is a no-op, while the
  machine that sleeps is Windows.
