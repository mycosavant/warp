> A decision on record, 2026-09-13. Binding until a later decision names it.

- **The TUI (`crates/warp_tui`) is wanted as a first-class surface, and it gets
  a test pass before that status.** The maintainer, 2026-09-13: *"TUI needs
  testing before 1st classing, but i would love that to be real."* What is
  already measured: `warpctrl` inside the TUI process (`f2e1558c6`) and
  park-and-approve end to end from another shell (`9b58f8eda`,
  `.fork/runs/tui-approve-2026-09-12/`). What must be measured before any
  answerer is built in it is I20's type-ahead hazard: whether a buffered Enter
  lands on a permission prompt, on which terminal, and whether SSH latency
  widens the window. That pass is `HANDOFF-BUILDS.md` task 3. First-class
  status is the maintainer's call after reading it.
