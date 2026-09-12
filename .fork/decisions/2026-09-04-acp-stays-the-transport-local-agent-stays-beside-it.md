> A decision on record, split out of `.fork/GOAL.md` on 2026-09-12 when that horizon retired. Binding until a later decision names it.

- **ACP stays the transport, and `local_agent` stays beside it** (2026-09-04).
  Asked whether ACP was the right call once the permission half is idle by
  default. It is, for the reason that survives the freeze: it is the only
  versioned interface a non-Claude agent offers, and the thesis names local
  models, which no Claude-only path reaches. `local_agent` is kept as the path
  with no `npx` in front of it. Neither is removed. What stops is measuring
  the permission half of ACP.
