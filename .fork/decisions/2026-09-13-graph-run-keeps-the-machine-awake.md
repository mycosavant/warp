> A decision on record, 2026-09-13. Binding until a later decision names it.

- **A `warpctrl graph run` holds the machine awake for its duration, whenever
  system sleep is enabled.** Taken by the maintainer on 2026-09-13, answering
  `WB-SLEEP` (T15), which had been filed as a decide since T11. The maintainer
  has disabled sleep on the desk for now, so the problem is not live today;
  the ruling is that a run should not depend on that setting. The mechanism
  exists upstream (`crates/prevent_sleep`: Windows and macOS backends, a no-op
  elsewhere) and is used only by `http_client`. How it reaches a run is open
  and is `HANDOFF-BUILDS.md` task 2, including the fact that shapes it: `graph
  run` is driven from a `warpctrl` process that is often the Linux one inside
  WSL, where the guard is a no-op, while the machine that sleeps is Windows.
