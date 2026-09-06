> A decision on record, 2026-09-06. Binding until a later decision names it.

Four answers to `.fork/mobile.html`, given by the maintainer the day it was
drafted, each the page's recommendation.

- **Trust on the wire is a fork-minted CA the phone installs once; reach is
  headscale, separately.** A LAN or tailnet address over plain HTTP is not a
  secure context, so browsers withhold notifications, service workers, push
  and Chrome's install prompt there, and a mesh alone changes none of that.
  Warp mints a CA key once, keeps it private beside the discovery record,
  signs a certificate for the bind address, and serves the CA's public half
  at one constant route; the phone installs it from a link the pairing page
  shows. The loopback listener stays plain: `warpctrl` is not a browser and
  localhost is already secure. Refused: a self-signed server certificate
  without a CA (a warning page and a tap-through), and a public certificate
  (a DNS name and an account for a console meant for one phone). headscale
  on a VPS is ops, not code, and does not wait on the CA or vice versa.
- **Being told is foreground-only.** Once the page is a secure context, the
  Notification API and vibration fire on a permission request and on a turn
  ending, from the events stream the page already holds. Web Push is not
  built; it is recorded as a disclosed opt-in behind its own variable, off,
  to be built only if a week of use says the foreground half is not enough,
  because push is a nudge through Google's or Apple's relay and that relay
  learns when this machine had something to say.
- **A control pairing has no clock.** It ends on Stop sharing, on the
  conversation being deleted, or on Warp closing. The 12-hour lifetime stays
  for the watch pairing, which a variable minted; a control pairing was minted
  by a person pointing at a conversation, and the credential purge on stop is
  what bounds a leak, not the clock.
- **The phone's actions are recorded, not marked.** `prompt_submit` and
  `permission_replied` in the event log say `via: paired_device`; the trace
  view stamps those rows; the conversation shows the prompt as the person's
  own words, because a marker there is a Warp aside in the agent's context,
  which the transcript work went to some trouble to avoid.
