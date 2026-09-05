> Idea I14, split out of `.fork/IDEAS.md` on 2026-09-05. History; read the section you came for.

# I14 — Context masking

> *(from the same note as I9: "a question I wrote is 'context masking'")*

Recorded separately because it is not the same as pruning and should not be
folded into it silently.

Pruning **removes** content. Masking **hides** content while keeping it
recoverable — the model does not see it this turn, but it is still there and can
come back. Manus-style approaches use this for tool definitions: keep every tool
in the prefix (so the cache holds) and mask which ones are *selectable* this
turn, rather than editing the tool list and destroying the prefix.

That is a genuinely clever answer to the caching problem in I9, and it points at
something concrete here: if the expensive, cache-breaking thing turns out to be
the **tool list** rather than the transcript, then masking is the fix and
pruning is not.

Which is one more reason the first task in I9 is measurement.

---

