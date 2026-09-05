> The board's own preamble and the pre-board checklist, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

# Fork task board

Tracks the full scope agreed 2026-08-17. Ordered by value-per-line-of-code, not
by conceptual grandeur — see `.fork/archive/SPEC.md` for the reasoning behind each.

Status key: `[ ]` not started · `[~]` in progress · `[x]` done · `[!]` blocked
· `[-]` dropped (with reason)

Phases 0–4 in `.fork/archive/SPEC.md` are the original de-telemetry/de-account track. This
board supersedes it from Phase 5 on, and renumbers nothing.

> **Most of this file is history, and that is the point.** T1–T7 are done.
> Read a section when you are about to touch that area; do not read it front to
> back.
>
> Three kinds of content live here, and they age differently:
>
> * **Checklists** (`- [x] T1.4 …`) — a record of what shipped. Historic.
> * **"as built" sections** — what was actually found when the thing was run,
>   including where the plan was wrong. **These stay live**, because they are
>   the only place several findings are written down.
> * **"Decisions on record"** and **"Open questions"**, near the bottom — live,
>   and the first place to look before re-opening something.
>
> **T12 is the current phase.** `.fork/tickets/` is the queue in front of it.
> `CLAUDE.md` is the cold-start summary of the method and the invariants.

---

## Done (carried over from SPEC Phases 0–2)

- [x] **P0** Repo hygiene, branch topology, git-lfs, CRLF/symlink corruption
- [x] **P1a** Telemetry egress deny-list (`crates/http_client/src/egress.rs`)
- [x] **P1b** Telemetry collection shutdown (`settings/privacy.rs` accessors)
- [x] **P1c** Feature-flag kill switch (`app/src/fork.rs`)
- [x] **P1d** Account gates — master AI switch, BYO key, custom inference
- [x] **P1e** Account gate — settings UI banner (`is_anonymous_for_ui`)
- [x] **P4a** Local OpenTelemetry export, loopback-only auth bypass
- [x] **P2a** Local harnesses forced available when logged out
- [x] Native Windows build verified end-to-end (`C:\dev\warp`)

---

