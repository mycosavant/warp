> A decision on record, 2026-09-13. Binding until a later decision names it.

- **ACP agents are installed from pinned, verified artifacts and launched by
  absolute path, not fetched through `npx` or another package manager at
  launch.** Decided by the maintainer, 2026-09-13: the fork does not launch
  ACP agents through `npx`, and they distrust package managers in general.
  They asked for agents to be vendored, or forked, pinned and audited before
  each bump, and accept the maintenance that costs. Their reason is that the
  npm registry has a record of compromised packages and that attackers using
  AI models will make that worse.

  Open-source parts are forked by the maintainer, pinned by commit and audited
  on every bump. The Claude agent SDK inside `claude-agent-acp` is proprietary
  (READ, its `LICENSE.md`: "All rights reserved"), so it is pinned by sha256
  and fetched once, never forked or committed; its audit is behavioural, a
  re-run of the vendor-calls census on every bump, and the docs say so. Warp
  tells the user when a launch command goes through a package manager and does
  not refuse it. The work is `HANDOFF-SUPPLYCHAIN.md`.
