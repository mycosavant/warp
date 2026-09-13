> A decision on record, 2026-09-13. Binding until a later decision names it.

- **ACP agents are installed from pinned, verified artifacts and launched by
  absolute path, not fetched through `npx` or another package manager at
  launch.** The maintainer, verbatim: *"i don't like npx at all, and really
  don't even like package managers in the first place. i do strongly feel that
  we should not npx acp agents. Can we vendor them? or have forks that we pin
  and audit before bumping? i don't mind the workload there at all. NPM is
  pretty much notorious for vulns now, and it's only going to get worse as
  threat actors w/ ai-models get increasingly more sophisticated."*

  Open-source parts are forked by the maintainer, pinned by commit and audited
  on every bump. The Claude agent SDK inside `claude-agent-acp` is proprietary
  (READ, its `LICENSE.md`: "All rights reserved"), so it is pinned by sha256
  and fetched once, never forked or committed; its audit is behavioural, a
  re-run of the vendor-calls census on every bump, and the docs say so. Warp
  tells the user when a launch command goes through a package manager and does
  not refuse it. The work is `HANDOFF-SUPPLYCHAIN.md`.
