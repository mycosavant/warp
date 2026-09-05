> Idea I10, split out of `.fork/IDEAS.md` on 2026-09-05. History; read the section you came for.

# I10 — The browser

> *"Integrated browser (web | preview | monitor) + agent control surface."*

**I recommend not doing this**, and I want to give the reasons rather than just
the verdict, because the three words in your parentheses are three different
features with three different answers.

## The cost

There is **no web engine anywhere in this tree.** No `wry`, no `tao`, no CEF, no
servo, no webkit binding — I checked every `Cargo.toml`. Adding one means:

* A very large new dependency with its own release cadence, its own build
  requirements per platform, and its own crash surface.
* A **second network stack that the fork does not control.** The de-telemetry
  work (P1a, and the egress measurement closed 2026-08-20) rests on Warp making
  no requests you did not ask for. A browser engine makes requests constantly —
  favicons, safe-browsing, prefetch, telemetry of its own — and every one of
  those is outside `crates/http_client/src/egress.rs`. The headline claim of
  this fork would become conditional the day a webview lands, and re-measuring
  it would be much harder than it was.
* A new sandbox question: an agent that can drive a browser can navigate
  anywhere with your cookies.

That is a lot to take on for a fork whose value so far has come from *removing*
surface.

## The cheap 80%, which is three separate small things

* **monitor** — already exists. `app/src/pane_group/pane/network_log_pane.rs`
  is a pane that watches network activity. Whatever you want here is probably an
  improvement to that, not a browser.
* **preview** — if this means "see the local dev server I just started", the
  smallest honest version is a pane that renders a screenshot on an interval or
  on file change. Ugly, and it covers the actual need (did my change render?)
  without a web engine.
* **web** — this is the one that genuinely needs a browser. And it is also the
  one where the answer "use your browser, on the other monitor" is hardest to
  argue with.

## Answered, and it is not a browser

Asked 2026-08-21, and the answer moved this entry somewhere much better:

> *"dev server and, in another project specifically in Claude Desktop, Claude
> can instrument the build and monitor the user actions for agent-assisted smoke
> testing. Also great for previewing designs. You're also right about the other
> monitor bit."*

So **`web` is off the table** — browsing stays on the other monitor, agreed on
both sides. What is left is two things, and neither needs a web engine:

* **preview a design / a dev server** — an image on a refresh.
* **an agent that instruments a build and watches what you do** — a
  screenshot-and-input channel, not a document renderer. The thing being
  observed does not have to be a web page at all. It could be Warp itself.

That second one is the valuable half, and searching for it turned up something
that belongs in its own entry.

**See [I15](I15-computer-use.md).** The screenshot,
input, window-enumeration and recording stack for exactly this already exists
in `crates/computer_use`, behind a dogfood flag — the third time this fork has
found a finished feature gated off. And a *window-targeted screenshot works
today*, verified by taking one.

Which leaves I10 itself as: **a pane that renders an image and refreshes it.**
That is the whole feature. Point it at a screenshot the agent just took, or at
a file a build wrote, and re-render on change. Warp already renders images
(`kitty_images`), already has file panes, and already has a pane kind for
"watches something rather than hosts a shell" (`network_log_pane.rs`).

No webview, no second network stack, no re-opening the egress question.

---

