> Idea I12, split out of `.fork/IDEAS.md` on 2026-09-05. History; read the section you came for.

# I12 — Tooltips

> *"Tooltips: delay, opacity, content options."*

Small, and it reads like an irritation rather than a feature — which usually
means it is worth fixing. Tooltips are used widely
(`app/src/tab.rs`, `app/src/menu.rs`, most of `settings_view/`), so the
question is whether there is one tooltip component with hard-coded timing, or
many. If one: this is a settings struct and three fields, an afternoon. If many:
consolidating them is the actual task and it is worth doing anyway.

Not selected, but it is the best candidate for "something small to do while
waiting for a long build".

---

