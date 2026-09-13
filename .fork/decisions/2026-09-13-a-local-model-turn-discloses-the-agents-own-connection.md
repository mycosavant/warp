> A decision on record, 2026-09-13. Binding until a later decision names it.
> Reverses half of the 2026-09-11 ruling in `docs/agent-transports.md`.

- **When the panel answers from a local model through an agent measured to
  contact its vendor anyway, Warp says so.** The maintainer, 2026-09-13: *"we
  want to be HONEST and ON-THESIS. i feel that at minimum it should be
  disclosed when running a local model."* The measured fact is unchanged:
  during a turn answered entirely by `llama-server`, `claude-agent-acp` opened
  TLS connections to `api.anthropic.com`, and the non-essential-traffic flags
  do not remove it (`.fork/runs/localmodel-panel-2026-09-09/`,
  `.fork/runs/pricefetch-2026-09-09/`). **Accepted** stands: Warp does not
  block or contain it. **Refused a disclosure** does not.

  **Why the 09-11 reason no longer holds.** It said a notice would be consent
  UI for something Warp neither causes nor can stop, and teaches nothing the
  person did not choose when naming the agent. But the fork's first sentence
  promises that a local model keeps the work on the machine, and a person who
  points the panel at a local model has chosen exactly that. The fact
  contradicts the promise at the moment they rely on it. A notice is not
  consent: it asks nothing and gates nothing, the same kind of thing as the
  mode disclosure.

  **Its shape, as the maintainer gave it:** a note in the panel with *Got it*,
  and an option for that acknowledgement to be the last time it shows. What
  triggers it, how "local" is detected, and whether the dismissal is keyed to
  the agent and version are `HANDOFF-BUILDS.md` task 4. The docs will
  recommend a different harness for local models to anyone for whom this
  matters, **but only a harness measured not to do the same thing.**
