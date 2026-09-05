> Ticket T12, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

## T12 — The console: the client T11 was built for  ← DONE (2026-08-27)

> Scoped 2026-08-26. **The argument for doing this before anything else is the
> fork's own anti-goal.** T11 shipped an event log, a state snapshot, an SSE
> stream, a LAN listener, QR pairing and remote approve/deny. Every one of them
> is reachable only by `curl`. The failure this fork was started over — recorded
> in `.fork/archive/CONSOLIDATION.md` and restated in T11's framing — is *"features
> implemented but never wired, and docs written as though they were."* T11 is
> currently in exactly that state, and it is the one state this phase exists to
> detect. Five tickets deliver none of the ratified goal until something renders
> them.
>
> **The second reason is that rendering is a test.** Whether `/v1/state` carries
> the right fields is not answerable by reading it; it is answerable by trying to
> draw a screen from it and finding out what is missing. Expect T12 to send work
> back into T11's shape, and expect that to be the valuable part.

**Gate check, run 2026-08-26 before scoping.** `include_str!(*.html)`, `Html(`,
`ServeDir` and `text/html` across `app/src`, `crates/local_control` and
`crates/http_server` return exactly one hit — a MIME-extension table in
`app/src/ai/artifact_download.rs`. **Nothing serves a page anywhere in this
fork.** Unusually for this board, the answer is "not already built". `axum` is
already a workspace dependency of `app`, so the route itself costs nothing.

**And the web-surface question from `.fork/archive/CONSOLIDATION.md` §10 step 3 is hereby
settled, by reading both.** They are different needs and neither covers the
other: Warp's remote-development server (`wsl_transport.rs`) puts a *shell* on
another machine, while `tusk/engine/src/serve.rs` puts a *view of a running
session* on a phone. The fork now has the second one's entire backend and no
front end. Tusk's front end is also the precedent for the size: a 162-line
`serve_index.html` served as an embedded fallback, with an optional built Svelte
client behind `--web-dir`. **Take the 162-line half and not the Svelte half** —
a build step in this tree would be a new toolchain for one page.

- [x] **T12.1** One route, one embedded page, read-only. `GET /` on `warpctrl`'s
      listener returning a single self-contained HTML file: no build step, no
      npm, no framework, no external fetch. It primes from `/v1/state` and then
      follows `/v1/events`. **The hard part is not the page, it is that this is
      the first browser-reachable surface on the authenticated control plane** —
      so the design questions to answer *before* writing markup are: where the
      device token lives (fragment, never the query string, which lands in logs
      and referrers), what the CORS and origin policy is, and what escaping rule
      applies to agent-authored text, which is attacker-influenced by
      construction.
- [x] **T12.2** Approvals on the page. Render `agent.approvals`, and wire the
      two answers with the asymmetry T11.5 established: `deny` is pairable and
      always present, `approve` appears only when the instance advertises it.
      The page must learn that from the server rather than assuming it — a
      button that 403s is worse than a button that is absent.
- [x] **T12.3** Installable, and the QR points at it. A manifest and an icon so
      it is a home-screen app rather than a tab, and `control.pair`'s QR encodes
      the *page* URL rather than a bare token — pairing that ends at a page a
      person can use is the difference between a demo and a tool.

