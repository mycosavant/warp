> A decision on record, 2026-09-13. Binding until a later decision names it.

- **A native Android app is a destination for the phone surface. Nothing is
  scoped.** Decided by the maintainer, 2026-09-13: an Android app (APK) is a
  destination. Everything past that is the maintainer's leaning, not a choice:

  - T3 Code's mobile app is their model for a minimal, on-thesis first effort:
    a remote-control GUI that pairs with the desk instance. They believe it is
    React Native and have not confirmed its architecture (TOLD).
  - They would rather build a native Android app around a Rust core, possibly
    with Jetpack Compose for the UI (ASSUMED: their wording named a composer,
    read here as Compose, not the fork's panel composer), or Kotlin
    Multiplatform if the Rust core can be bound to it.
  - The console's certificate authority is their largest single reason to
    want the app: with the authority installed, their browsers still mark the
    console as not secure (TOLD, 2026-09-13), so they have mostly moved to
    mosh and tmux, which they also prefer to use. The unexpiring control token
    (`2026-09-13-a-control-pairing-expires-after-twelve-hours.md`) is a
    further reason.

  T3 Code's app has not been read here. What the app would talk to already
  exists: `/remote-control`'s pairing and scoped actions
  (`docs/remote-control.md`), the console over TLS, and reach over the tailnet
  (`reach.html`). An app can pin the fork's authority inside itself rather than
  asking the phone to trust a root system-wide, which answers the browser
  problem and most of what a stolen authority key could do to the phone.
  Until a spec exists, `HANDOFF-SECURITY.md` task 5 hardens the authority the
  browser console uses.
