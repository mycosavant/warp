> Idea I2, split out of `.fork/IDEAS.md` on 2026-09-05. History; read the section you came for.

# I2 — The composer

> *"An upgraded composer. Many apps are doing this, it's kind of become an
> industry standard (Cursor, Codex, t3code, Claude Desktop, et al)."*

## Where this stands

I am not going to scope this yet, and I want to be clear about why rather than
just deferring it.

Warp's composer is `app/src/terminal/input.rs` — **16,651 lines** — plus
`app/src/terminal/input/` which already contains: `message_bar/`, `models/`
(model picker), `plans/` (plan mode), `profiles/`, `skills/`, `prompts/`,
`repos/`, `rewind/`, `slash_commands/`, `inline_menu/`, `inline_history/`,
`suggestions_mode_menu.rs`, `handoff_compose.rs`. Attachments exist
(`app/src/context_chips/`, feature `image_as_context`).

That is not a thin composer. It is plausibly the *deepest* composer of the ones
you listed. So "upgrade it" without specifics risks rebuilding something that is
there, badly.

## What I want to do instead

The release build finishing right now is the tool for this. Sit in it for a few
days, and each time the composer is in your way, note the specific moment. Then
this entry becomes a list of concrete gaps rather than a category.

My guess at what you are actually reaching for — to be confirmed or thrown out
by that exercise:

* **Persistence of a draft across tab switches.** Losing a half-written prompt
  is the thing that makes a composer feel disposable.
* **Editing and resubmitting an earlier message** in place, rather than
  scrolling and retyping. (`rewind/` may already be this — worth a look.)
* **Seeing what is attached before you send**, as removable chips, including
  what the agent added implicitly.
* **Multi-line as the default posture**, with the send key an explicit choice
  rather than a fight with Enter.

Cheap, and worth doing regardless of the above: **draft persistence**. It is a
string per tab in the same place tab state already lives.

---

