> Ticket T17, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

## T17 — Is LSP-for-agents already built and switched off?

**Asked 2026-09-02**, running this fork's own "look for the gate first" rule
literally before proposing that Warp give its agents go-to-definition and
find-references. **Answer: no, and it should not be built.**

### The gate looked exactly right, and was not

`FeatureFlag::LSPAsATool` — doc: *"When enabled, we expose LSP as a tool to the
agent"* — is off in **both** halves: in no channel list, and `lsp_as_a_tool` is
declared in `app/Cargo.toml` but absent from `default`. A reader stops there and
reports the feature as existing behind a switch.

All three call sites are `LspRepoWatcher::ensure`/`teardown`
(`crates/lsp/src/model.rs:305,335,409`), forwarding repo file changes as
`workspace/didChangeWatchedFiles`. Nothing about tools, nothing about agents.
And the tool is **absent from the protocol**, not gated off: `ToolType` comes
from the `warp-proto-apis` git dependency, 36 variants, none LSP-related, and the
proto has no mention of `lsp`, `definition`, `references`, `symbol` or
`diagnostic`. No `warpctrl` action, no built-in MCP server and no `remote_server`
message touches LSP either.

Recorded as the thirteenth entry in `CLAUDE.md`'s stale-doc table and the first
in an *upstream* file — the previous twelve were all fork-authored, which quietly
implied the defect was ours.

### What exists, and one live defect found alongside

Implemented in `crates/lsp`: `textDocument/definition`, `/hover`, `/references`,
`/formatting`, `publishDiagnostics`. Absent entirely: `documentSymbol`,
`workspace/symbol`, `codeAction`, `rename`, `implementation`, `typeDefinition`.
Real server lifecycle — GitHub-release install with SHA256 verification, five
servers, capability negotiation, per-path enablement in SQLite. Every consumer is
editor UI.

**And `default_client_capabilities()` advertises `did_change_watched_files`
unconditionally** (`config.rs:284-289`) while the only sender of those events is
behind `LSPAsATool`. So Warp tells every language server *"register your globs
with me"*, the server registers, and Warp never sends one. The other channel,
`did_change_document`, fires only for **open editor buffers** — so a file changed
on disk by an agent, or by `git checkout`, is invisible to the server. Not fixed;
the decision between forcing the flag on and not advertising the capability is
the maintainer's.

### Why not to build it

Measured rather than argued: the recommended pairing already has the whole
surface. `claude-agent-acp` 0.70.0 loads Claude Code's `LSP` tool over the
panel's own transport — definition, references, hover, documentSymbol,
workspaceSymbol, implementation, call hierarchy **and diagnostics** — with zero
permission requests.

`opencode` is the cautionary half: its `lsp` tool behind
`OPENCODE_EXPERIMENTAL_LSP_TOOL` opened, its server failed to attach, and the
model then invented five symbols. A tool that fabricates is worse than none.

Seams ranked: **do nothing** > a `warpctrl lsp` action advertised by a per-turn
pointer > that action alone > a built-in MCP server > the ACP protocol (nothing
there). The prompt-injection seam (`transcript::pointer`) is an *announcement
layer*, not a seam for query-shaped data, and its own decision — *"the file is
offered, not explained"* — argues against advertising a capability the agent has.

**Also settled:** Windows rust-analyzer on a WSL-hosted crate over UNC ran 4.26 s
against 4.19 s for a local copy — ~5%, not a barrier. That is a two-function
crate and does **not** exercise the repo-scale walk that made T16 expensive, so
it shows the boundary is crossable, not that it is cheap at size.

---

