> Idea I4, split out of `.fork/IDEAS.md` on 2026-09-05. History; read the section you came for.

# I4 — Per-pane zoom and font size

> *"Per-pane zoom/font size. A setting, optional but default. Can be toggled off
> so the current universal zoom behaviour is still available."*

Real, and more invasive than it looks. Font size and zoom are both single global
values — confirmed against the running release build, which reports exactly one
`appearance.text.font_size` (13.0) and one `appearance.window.zoom_level` (100)
for the whole app. They live in a settings singleton
(`app/src/settings/font.rs`) and are read from the render path in a lot of
places; `increase_notebook_font_size` and friends mutate that global and write
user defaults. Making it per-pane means
either threading an override through every read, or introducing a scoped
settings lookup — and the second one is the kind of infrastructure this fork has
been right to avoid.

**Before scoping this, count the reads.** If `font_size` is read in five places
behind one accessor, this is a day. If it is read in eighty, the honest answer
is "not worth it" and the feature becomes *per-pane zoom only for the pane that
has focus*, which is a much smaller thing and possibly all you actually wanted.

Not selected. The measurement is a 20-minute task whenever you want it.

---

